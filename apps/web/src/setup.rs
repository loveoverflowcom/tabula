//! The setup draft and its state machine.
//!
//! This is the shell's half of screen 03: it owns fields, focus, and the
//! lifecycle, and it owns nothing about what the values *mean*. Every
//! transition out of `Editing` is a result the registry produced.
//!
//! The machine is written against the shared contract in
//! `docs/ui/screens/discovery.md`: a validation result belongs to exactly one
//! draft revision, editing invalidates readiness immediately, a late result for
//! an older revision is discarded, and `Pending` admits neither a field edit
//! nor a second submission.

use tabula_registry::{
    BotLevel, ConfigDraft, ConfigRejection, ErasedGame, LaunchHandoff, LaunchMode,
    NormalizedConfig, SetupRequest, UnavailableReason,
};

/// Where the draft is in its lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// No mode this build can start exists for this game, or an accepted
    /// configuration could not be handed off. The configuration is kept so the
    /// player can review exactly what was refused.
    Unavailable {
        reason: UnavailableReason,
        config: Option<NormalizedConfig>,
    },
    /// Values are being entered; nothing has been validated.
    Editing,
    /// A revision was submitted for validation.
    Validating,
    /// This exact revision validated and may be started.
    Ready(NormalizedConfig),
    /// The game rejected this revision. The draft is kept as entered.
    Rejected(ConfigRejection),
    /// A start was requested for an accepted configuration.
    Pending(NormalizedConfig),
    /// A gameplay document was resolved; the shell navigates to it.
    Handoff(LaunchHandoff),
}

impl Phase {
    /// Label key for the visible state line.
    #[must_use]
    pub const fn label_key(&self) -> &'static str {
        match self {
            Self::Unavailable { .. } => "setup.state.unavailable",
            Self::Editing => "setup.state.editing",
            Self::Validating | Self::Pending(_) | Self::Handoff(_) => "setup.state.validating",
            Self::Ready(_) => "setup.state.ready",
            Self::Rejected(_) => "setup.state.rejected",
        }
    }

    #[must_use]
    pub const fn locks_fields(&self) -> bool {
        matches!(
            self,
            Self::Pending(_) | Self::Handoff(_) | Self::Unavailable { .. }
        )
    }
}

/// One game's setup draft.
#[derive(Clone, Debug)]
pub struct SetupState {
    revision: u64,
    mode: Option<LaunchMode>,
    seats: u8,
    bot_level: Option<BotLevel>,
    draft: ConfigDraft,
    phase: Phase,
}

impl SetupState {
    /// Start a draft for a game, with the game's own defaults.
    ///
    /// `preselected` is screen 02's explicit choice; it is kept only if it is
    /// still a supported mode, and otherwise the first supported mode is used.
    /// A game with no supported mode starts `Unavailable`.
    #[must_use]
    pub fn new(game: &dyn ErasedGame, preselected: Option<LaunchMode>) -> Self {
        let supported: Vec<LaunchMode> = game
            .modes()
            .iter()
            .filter(|support| support.is_available())
            .map(|support| support.mode)
            .collect();
        let mode = preselected
            .filter(|mode| supported.contains(mode))
            .or_else(|| supported.first().copied());
        let seats = game.capabilities().seats().allowed().min();
        let bot_level = game.bot_levels().first().copied();
        let phase = if mode.is_some() {
            Phase::Editing
        } else {
            Phase::Unavailable {
                reason: UnavailableReason::NoNetworkService,
                config: None,
            }
        };
        Self {
            revision: 0,
            mode,
            seats,
            bot_level,
            draft: ConfigDraft::with_defaults(game.form()),
            phase,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn phase(&self) -> &Phase {
        &self.phase
    }

    #[must_use]
    pub const fn mode(&self) -> Option<LaunchMode> {
        self.mode
    }

    #[must_use]
    pub const fn seats(&self) -> u8 {
        self.seats
    }

    #[must_use]
    pub const fn bot_level(&self) -> Option<BotLevel> {
        self.bot_level
    }

    #[must_use]
    pub const fn draft(&self) -> &ConfigDraft {
        &self.draft
    }

    /// The field error currently attached to `key`, if any.
    #[must_use]
    pub fn field_error(&self, key: &str) -> Option<&ConfigRejection> {
        match &self.phase {
            Phase::Rejected(rejection) if rejection.field.as_deref() == Some(key) => {
                Some(rejection)
            }
            _ => None,
        }
    }

    /// The rejection that is not attached to any single field.
    #[must_use]
    pub fn summary_error(&self) -> Option<&ConfigRejection> {
        match &self.phase {
            Phase::Rejected(rejection) if rejection.field.is_none() => Some(rejection),
            Phase::Rejected(rejection) => Some(rejection),
            _ => None,
        }
    }

    pub fn set_field(&mut self, key: &str, value: impl Into<String>) {
        if self.phase.locks_fields() {
            return;
        }
        self.draft.set(key, value);
        self.invalidate();
    }

    pub fn set_mode(&mut self, mode: LaunchMode) {
        if self.phase.locks_fields() {
            return;
        }
        self.mode = Some(mode);
        self.invalidate();
    }

    pub fn set_seats(&mut self, seats: u8) {
        if self.phase.locks_fields() {
            return;
        }
        self.seats = seats;
        self.invalidate();
    }

    pub fn set_bot_level(&mut self, level: BotLevel) {
        if self.phase.locks_fields() {
            return;
        }
        self.bot_level = Some(level);
        self.invalidate();
    }

    /// Any edit retires the current revision: readiness never outlives the
    /// exact draft it was granted for.
    fn invalidate(&mut self) {
        self.revision = self.revision.saturating_add(1);
        self.phase = Phase::Editing;
    }

    /// Begin validating the current revision.
    ///
    /// Returns the request to hand to the registry, together with the revision
    /// the answer must carry. `None` when there is nothing to validate.
    pub fn begin_validation(&mut self) -> Option<(u64, SetupRequest)> {
        if self.phase.locks_fields() {
            return None;
        }
        let mode = self.mode?;
        self.phase = Phase::Validating;
        Some((
            self.revision,
            SetupRequest {
                mode,
                seats: self.seats,
                bot_level: mode.fills_with_bots().then_some(self.bot_level).flatten(),
                draft: self.draft.clone(),
            },
        ))
    }

    /// Accept a validation result, discarding one for a retired revision.
    pub fn validated(
        &mut self,
        revision: u64,
        result: Result<NormalizedConfig, ConfigRejection>,
    ) -> bool {
        if revision != self.revision || !matches!(self.phase, Phase::Validating) {
            return false;
        }
        self.phase = match result {
            Ok(config) => Phase::Ready(config),
            Err(rejection) => Phase::Rejected(rejection),
        };
        true
    }

    /// Submit the accepted configuration exactly once.
    pub fn submit(&mut self) -> Option<NormalizedConfig> {
        let Phase::Ready(config) = &self.phase else {
            return None;
        };
        let config = config.clone();
        self.phase = Phase::Pending(config.clone());
        Some(config)
    }

    /// A gameplay document was resolved for the pending configuration.
    pub fn handed_off(&mut self, handoff: LaunchHandoff) {
        if matches!(self.phase, Phase::Pending(_)) {
            self.phase = Phase::Handoff(handoff);
        }
    }

    /// The pending start could not be resolved. The accepted configuration is
    /// kept and the reason stays visible; nothing claims a started match.
    pub fn start_unavailable(&mut self, reason: UnavailableReason) {
        if let Phase::Pending(config) = &self.phase {
            self.phase = Phase::Unavailable {
                reason,
                config: Some(config.clone()),
            };
        }
    }

    /// Return from an unavailable start to the draft that produced it.
    pub fn resume_editing(&mut self) {
        if matches!(self.phase, Phase::Unavailable { .. }) && self.mode.is_some() {
            self.invalidate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Phase, SetupState};
    use tabula_registry::{
        BotLevel, Catalog, ConfigRejection, GameId, I18nKey, LaunchMode, Localizer,
        RejectionReason, RuntimeBinding, UnavailableReason,
    };

    struct NoMessages;

    impl Localizer for NoMessages {
        fn text(&self, _key: &I18nKey) -> Option<&str> {
            None
        }
    }

    fn catalog() -> Catalog {
        tabula_registry::catalog(&NoMessages)
    }

    fn first_game(catalog: &Catalog) -> &dyn tabula_registry::ErasedGame {
        catalog.entries().first().expect("a linked game").game()
    }

    #[test]
    fn a_fresh_draft_selects_a_supported_mode_and_the_games_own_defaults() {
        let catalog = catalog();
        let game = first_game(&catalog);
        let state = SetupState::new(game, None);
        assert_eq!(state.phase(), &Phase::Editing);
        assert_eq!(state.mode(), Some(LaunchMode::LocalHotSeat));
        assert_eq!(state.seats(), game.capabilities().seats().allowed().min());
    }

    #[test]
    fn an_unsupported_preselection_is_replaced_rather_than_carried() {
        let catalog = catalog();
        let state = SetupState::new(first_game(&catalog), Some(LaunchMode::Network));
        assert_eq!(
            state.mode(),
            Some(LaunchMode::LocalHotSeat),
            "an unsupported mode never becomes the selection"
        );
    }

    #[test]
    fn a_supported_preselection_is_kept() {
        let catalog = catalog();
        let state = SetupState::new(first_game(&catalog), Some(LaunchMode::LocalBots));
        assert_eq!(state.mode(), Some(LaunchMode::LocalBots));
    }

    #[test]
    fn validation_is_scoped_to_one_revision() {
        let catalog = catalog();
        let game = first_game(&catalog);
        let mut state = SetupState::new(game, None);
        let (revision, request) = state.begin_validation().expect("a mode is selected");
        let result = game.normalize(&request);

        // The player keeps typing while validation is in flight.
        state.set_field("clock_does_not_exist", "x");
        assert_eq!(state.phase(), &Phase::Editing);

        assert!(
            !state.validated(revision, result),
            "a result for a retired revision is discarded"
        );
        assert_eq!(state.phase(), &Phase::Editing);
    }

    #[test]
    fn a_result_for_the_current_revision_is_accepted() {
        let catalog = catalog();
        let game = first_game(&catalog);
        let mut state = SetupState::new(game, None);
        let (revision, request) = state.begin_validation().expect("a mode is selected");
        assert_eq!(state.phase(), &Phase::Validating);
        assert!(state.validated(revision, game.normalize(&request)));
        assert!(matches!(state.phase(), Phase::Ready(_)));
    }

    #[test]
    fn editing_immediately_retires_readiness() {
        let catalog = catalog();
        let game = first_game(&catalog);
        let mut state = SetupState::new(game, None);
        let (revision, request) = state.begin_validation().expect("a mode is selected");
        state.validated(revision, game.normalize(&request));
        state.set_seats(99);
        assert_eq!(state.phase(), &Phase::Editing);
        assert_eq!(state.submit(), None, "a retired draft cannot be started");
    }

    #[test]
    fn a_rejection_keeps_the_values_that_were_rejected() {
        let catalog = catalog();
        let game = first_game(&catalog);
        let mut state = SetupState::new(game, None);
        state.set_seats(99);
        let (revision, request) = state.begin_validation().expect("a mode is selected");
        assert!(state.validated(revision, game.normalize(&request)));
        assert!(matches!(
            state.phase(),
            Phase::Rejected(ConfigRejection {
                reason: RejectionReason::SeatCount,
                ..
            })
        ));
        assert_eq!(state.seats(), 99, "the rejected input is preserved");
        assert!(state.summary_error().is_some());
    }

    #[test]
    fn a_pending_start_admits_neither_an_edit_nor_a_second_submission() {
        let catalog = catalog();
        let game = first_game(&catalog);
        let mut state = SetupState::new(game, None);
        let (revision, request) = state.begin_validation().expect("a mode is selected");
        state.validated(revision, game.normalize(&request));
        let first = state.submit().expect("a ready draft submits once");
        assert_eq!(state.submit(), None, "a second submission is refused");

        let seats_before = state.seats();
        state.set_seats(seats_before.wrapping_add(1));
        assert_eq!(
            state.seats(),
            seats_before,
            "fields are locked while pending"
        );
        assert!(matches!(state.phase(), Phase::Pending(_)));
        assert!(!first.launch_args.is_empty());
    }

    #[test]
    fn an_unresolvable_start_reports_its_reason_instead_of_a_started_match() {
        let catalog = catalog();
        let game = first_game(&catalog);
        let mut state = SetupState::new(game, None);
        let (revision, request) = state.begin_validation().expect("a mode is selected");
        state.validated(revision, game.normalize(&request));
        let config = state.submit().expect("ready");

        match tabula_registry::resolve_launch(RuntimeBinding::unbound(), &config) {
            Ok(handoff) => state.handed_off(handoff),
            Err(reason) => state.start_unavailable(reason),
        }
        assert!(matches!(
            state.phase(),
            Phase::Unavailable {
                reason: UnavailableReason::NoGameplayRuntime,
                config: Some(_),
            }
        ));

        state.resume_editing();
        assert_eq!(state.phase(), &Phase::Editing, "the draft is recoverable");
    }

    #[test]
    fn a_bot_level_is_only_sent_for_a_mode_that_uses_one() {
        let catalog = catalog();
        let game = first_game(&catalog);
        let mut state = SetupState::new(game, Some(LaunchMode::LocalHotSeat));
        let (_, request) = state.begin_validation().expect("a mode is selected");
        assert_eq!(request.bot_level, None);

        let mut state = SetupState::new(game, Some(LaunchMode::LocalBots));
        state.set_bot_level(BotLevel::Easy);
        let (_, request) = state.begin_validation().expect("a mode is selected");
        assert_eq!(request.bot_level, Some(BotLevel::Easy));
    }

    #[test]
    fn a_game_id_that_is_not_in_the_catalog_has_no_draft() {
        let catalog = catalog();
        let unknown = GameId::new("com.tabula.unlisted").expect("literal id");
        assert!(catalog.get(&unknown).is_none());
    }
}
