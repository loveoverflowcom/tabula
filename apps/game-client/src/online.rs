//! Projection-only opt-in direct browser session (ADR-0041).
//! No State, seed, timers, rules creation/apply or local replay lives here.
use std::marker::PhantomData;
use tabula_core::{canonical_decode, canonical_encode, GameId, GameVersion, MatchId};
use tabula_game_api::GameRules;
use tabula_net_client::direct::{DirectClient, DirectState};
use tabula_presentation::{AudioCues, FrameCtx, GamePresentation, InputEvent, RenderList};
use tabula_protocol::{ClientEnvelope, ErrorCode, ServerEnvelope, ServerMessage};

/// Authoritative server projection plus strictly local presentation.
pub struct OnlineMatch<R, P>
where
    R: GameRules,
    P: GamePresentation<Rules = R>,
    R::View: serde::de::DeserializeOwned,
    R::ViewEvent: serde::de::DeserializeOwned,
{
    view: Option<R::View>,
    local: P::Local,
    network: DirectClient,
    rejection: Option<ErrorCode>,
    presentation: PhantomData<P>,
}
impl<R, P> core::fmt::Debug for OnlineMatch<R, P>
where
    R: GameRules,
    P: GamePresentation<Rules = R>,
    R::View: serde::de::DeserializeOwned,
    R::ViewEvent: serde::de::DeserializeOwned,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OnlineMatch")
            .field("network", &self.network)
            .field("rejection", &self.rejection)
            .finish_non_exhaustive()
    }
}
impl<R, P> OnlineMatch<R, P>
where
    R: GameRules,
    P: GamePresentation<Rules = R>,
    R::View: serde::de::DeserializeOwned,
    R::ViewEvent: serde::de::DeserializeOwned,
{
    /// Only a validated initial authorized attachment snapshot can construct it.
    pub fn new(
        match_id: MatchId,
        game: GameId,
        game_version: GameVersion,
        next_seq: u64,
        frames: &[ServerEnvelope],
    ) -> Result<Self, &'static str> {
        let mut network = DirectClient::new(match_id, game, game_version, next_seq)
            .map_err(|_| "Invalid online sequence")?;
        network
            .receive(frames)
            .map_err(|_| "Invalid online stream")?;
        if frames.len() != 1 {
            return Err("The initial attachment must contain exactly one snapshot");
        }
        let view = frames
            .iter()
            .find_map(|frame| match frame.body() {
                ServerMessage::MatchUpdate { view, events, .. } if events.is_empty() => Some(view),
                _ => None,
            })
            .ok_or("The attachment has no projection")?;
        let view = canonical_decode(view).map_err(|_| "The server projection is incompatible")?;
        Ok(Self {
            view: Some(view),
            local: P::Local::default(),
            network,
            rejection: None,
            presentation: PhantomData,
        })
    }
    /// Bind a fresh server-derived operation-scope hint, never an authorization token.
    pub fn bind_scope(&mut self, scope: &str) -> Result<(), &'static str> {
        self.network
            .bind_scope(scope)
            .map_err(|_| "Invalid online operation scope")
    }
    /// Restore an opaque original intent only after fresh attachment validation.
    pub fn restore_pending(
        &mut self,
        scope: &str,
        command: ClientEnvelope,
    ) -> Result<Option<ClientEnvelope>, &'static str> {
        self.network
            .restore_pending(scope, command)
            .map_err(|_| "Invalid pending online operation")
    }
    /// Withdraw all private presentation while retaining the original uncertain intent.
    pub fn recover(&mut self) {
        self.network.recover();
        self.view = None;
        self.local = P::Local::default();
        self.rejection = None;
    }
    /// Replace the whole projection, clearing animations and checking original retry scope.
    pub fn resync(
        &mut self,
        scope: &str,
        next_seq: u64,
        frames: &[ServerEnvelope],
    ) -> Result<Option<ClientEnvelope>, &'static str> {
        if frames.len() != 1 {
            self.disconnect();
            return Err("Invalid resync snapshot");
        }
        let ServerMessage::MatchUpdate {
            view,
            events,
            revision: 0,
        } = frames[0].body()
        else {
            self.disconnect();
            return Err("Invalid resync snapshot");
        };
        if !events.is_empty() {
            self.disconnect();
            return Err("Invalid resync events");
        }
        let Ok(view) = canonical_decode(view) else {
            self.disconnect();
            return Err("The server projection is incompatible");
        };
        let Ok(retry) = self.network.resync(scope, next_seq, frames) else {
            self.disconnect();
            return Err("Invalid online resync");
        };
        self.view = Some(view);
        self.local = P::Local::default();
        self.rejection = None;
        Ok(retry)
    }
    /// Keep a fresh authorized board read-only when an old intent cannot be resolved.
    pub fn mark_unknown(&mut self) {
        self.network.mark_unknown();
    }
    /// Whether a definitive receipt is still missing for this original operation.
    pub const fn has_pending(&self) -> bool {
        self.network.has_pending()
    }
    /// Local controls never travel upstream.
    pub fn local_mut(&mut self) -> &mut P::Local {
        &mut self.local
    }
    /// Current transport input gate.
    pub const fn state(&self) -> DirectState {
        self.network.state()
    }
    /// Per-attachment visible revision.
    pub const fn revision(&self) -> Option<u64> {
        self.network.revision()
    }
    /// Status/result wording belongs to the actual game presenter.
    pub fn description(&self) -> Option<String> {
        self.view
            .as_ref()
            .map(|view| P::a11y(view, &self.local).status)
    }
    /// Most recent public-safe receipt rejection.
    pub const fn rejection(&self) -> Option<ErrorCode> {
        self.rejection
    }
    /// Draw the existing game presenter exclusively from server projections.
    pub fn present(&self, frame: &FrameCtx) -> Option<RenderList> {
        self.view
            .as_ref()
            .map(|view| P::present(view, &self.local, frame))
    }
    /// Build/encode typed intent without any local rules evaluation.
    pub fn on_input(&mut self, input: &InputEvent) -> Result<Option<ClientEnvelope>, &'static str> {
        let Some(view) = self.view.as_ref() else {
            return Ok(None);
        };
        if self.network.state() != DirectState::Ready {
            // Release/focus cleanup must not leave keys or drag held after a receipt.
            if matches!(
                input,
                InputEvent::Key { pressed: false, .. }
                    | InputEvent::Focus(_)
                    | InputEvent::Pointer {
                        phase: tabula_presentation::PointerPhase::Cancel,
                        ..
                    }
            ) {
                let _ = P::on_input(input, view, &mut self.local);
            }
            return Ok(None);
        }
        let Some(intent) = P::on_input(input, view, &mut self.local) else {
            return Ok(None);
        };
        let payload = canonical_encode(intent.command())
            .map_err(|_| "The online command could not be encoded")?;
        let command = self
            .network
            .command(payload)
            .map_err(|_| "The online board is not ready to send")?;
        self.rejection = None;
        Ok(Some(command))
    }
    /// Decode/validate the entire batch before replacing a projected value.
    pub fn receive(
        &mut self,
        frames: &[ServerEnvelope],
        frame: &FrameCtx,
    ) -> Result<AudioCues, &'static str> {
        let result = self.receive_checked(frames, frame);
        if result.is_err() {
            self.disconnect();
        }
        result
    }
    fn receive_checked(
        &mut self,
        frames: &[ServerEnvelope],
        frame: &FrameCtx,
    ) -> Result<AudioCues, &'static str> {
        let mut updates = Vec::new();
        for envelope in frames {
            if let ServerMessage::MatchUpdate { view, events, .. } = envelope.body() {
                let view = canonical_decode::<R::View>(view)
                    .map_err(|_| "The server projection is incompatible")?;
                let events = events
                    .iter()
                    .map(|event| canonical_decode::<R::ViewEvent>(event))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| "The server event is incompatible")?;
                updates.push((view, events));
            }
        }
        self.network
            .receive(frames)
            .map_err(|_| "The online stream was interrupted")?;
        let mut cues = AudioCues::new();
        for (view, events) in updates {
            self.view = Some(view);
            for event in events {
                cues.extend(P::on_view_event(&event, &mut self.local, frame));
            }
        }
        for envelope in frames {
            if let ServerMessage::Reject { error, .. } = envelope.body() {
                self.rejection = Some(*error);
            }
        }
        Ok(cues)
    }
    /// Lose authority and discard the projection and all local presentation data.
    pub fn disconnect(&mut self) {
        self.network.disconnect();
        self.view = None;
        self.local = P::Local::default();
        self.rejection = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{RulesVersion, SeatRoster, Viewer};
    use tabula_game_api::{A11yDescription, Ctx, Init, InitError, Input, Outcome};
    use tabula_presentation::{AssetPackRef, Camera2D, Intent, Key, RenderListBuilder};
    struct NeverLocalAuthority;
    impl GameRules for NeverLocalAuthority {
        type State = u16;
        type Command = u8;
        type Event = u16;
        type View = u8;
        type ViewEvent = u8;
        type Config = ();
        const RULES_VERSION: RulesVersion = RulesVersion(1);
        fn create((): &(), _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            panic!("online must not create local authority")
        }
        fn apply(
            _: &mut u16,
            _: Input<u8>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, tabula_core::RuleError> {
            panic!("online must not apply rules")
        }
        fn project(_: &u16, _: Viewer) -> u8 {
            panic!("online must not project canonical state")
        }
        fn view_event(_: &u16, _: &u16, _: Viewer) -> Option<u8> {
            panic!("online must not redact canonical events")
        }
    }
    #[derive(Default)]
    struct Local {
        held: bool,
        events: u8,
        forbid_presentation: bool,
    }
    struct Presenter;
    impl GamePresentation for Presenter {
        type Rules = NeverLocalAuthority;
        type Local = Local;
        fn asset_pack() -> AssetPackRef {
            panic!("this fixture needs no resources")
        }
        fn present(_: &u8, local: &Local, _: &FrameCtx) -> RenderList {
            assert!(
                !local.forbid_presentation,
                "lost projection must never be presented"
            );
            RenderListBuilder::new(Camera2D::default())
                .finish()
                .unwrap()
        }
        fn on_view_event(event: &u8, local: &mut Local, _: &FrameCtx) -> AudioCues {
            local.events += *event;
            AudioCues::new()
        }
        fn on_input(input: &InputEvent, _: &u8, local: &mut Local) -> Option<Intent<u8>> {
            match input {
                InputEvent::Key {
                    key: Key::Enter,
                    pressed: true,
                } => {
                    local.held = true;
                    Some(Intent::new(9))
                }
                InputEvent::Key { pressed: false, .. } | InputEvent::Focus(false) => {
                    local.held = false;
                    None
                }
                _ => None,
            }
        }
        fn a11y(view: &u8, local: &Local) -> A11yDescription {
            assert!(
                !local.forbid_presentation,
                "lost projection must never be described"
            );
            let mut value = A11yDescription::unsupported();
            value.status = view.to_string();
            value
        }
    }
    fn frame() -> FrameCtx {
        FrameCtx::new(
            tabula_presentation::Viewport::new(glam::Vec2::new(800.0, 600.0)).unwrap(),
            tabula_presentation::Dpi::new(1.0).unwrap(),
            0,
            tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light),
        )
    }
    fn update(number: u64, revision: u64, view: u8, events: Vec<Vec<u8>>) -> ServerEnvelope {
        ServerEnvelope::new(
            None,
            number,
            ServerMessage::MatchUpdate {
                revision,
                view: canonical_encode(&view).unwrap(),
                events,
            },
        )
        .unwrap()
    }
    fn session() -> OnlineMatch<NeverLocalAuthority, Presenter> {
        OnlineMatch::new(
            MatchId(7),
            GameId::new("org.example.game").unwrap(),
            GameVersion::new("1.0.0").unwrap(),
            1,
            &[update(1, 0, 1, Vec::new())],
        )
        .unwrap()
    }
    const SCOPE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[test]
    fn recovery_drops_private_view_and_local_input_before_fresh_same_scope_retry() {
        let mut online = session();
        online.bind_scope(SCOPE).unwrap();
        let original = online
            .on_input(&InputEvent::Key {
                key: Key::Enter,
                pressed: true,
            })
            .unwrap()
            .unwrap();
        online.local_mut().events = 9;
        online.recover();
        assert_eq!(online.state(), DirectState::UnknownResult);
        assert_eq!(online.description(), None);
        assert!(online.present(&frame()).is_none());
        assert!(!online.local.held);
        assert_eq!(online.local.events, 0);
        assert_eq!(
            online
                .resync(SCOPE, 2, &[update(1, 0, 2, Vec::new())])
                .unwrap(),
            Some(original)
        );
        assert_eq!(online.description().as_deref(), Some("2"));
        assert_eq!(online.local.events, 0);
        assert_eq!(online.state(), DirectState::Sending);
    }
    #[test]
    fn fresh_projection_with_changed_scope_is_read_only_and_never_runs_rules() {
        let mut online = session();
        online.bind_scope(SCOPE).unwrap();
        online
            .on_input(&InputEvent::Key {
                key: Key::Enter,
                pressed: true,
            })
            .unwrap();
        online.recover();
        assert!(online
            .resync(&"b".repeat(64), 1, &[update(1, 0, 2, Vec::new())])
            .unwrap()
            .is_none());
        assert_eq!(online.description().as_deref(), Some("2"));
        assert_eq!(online.state(), DirectState::ReadOnly);
        assert!(online
            .on_input(&InputEvent::Key {
                key: Key::Enter,
                pressed: true
            })
            .unwrap()
            .is_none());
        assert!(!online.local.held);
    }
    #[test]
    fn malformed_resync_withdraws_private_output_even_if_called_without_prior_recover() {
        let mut online = session();
        online.bind_scope(SCOPE).unwrap();
        let malformed = ServerEnvelope::new(
            None,
            1,
            ServerMessage::MatchUpdate {
                revision: 0,
                view: vec![255, 255],
                events: Vec::new(),
            },
        )
        .unwrap();
        assert!(online.resync(SCOPE, 1, &[malformed]).is_err());
        assert!(online.present(&frame()).is_none());
        assert_eq!(online.description(), None);
    }
    #[test]
    fn intents_never_run_rules_or_change_the_authoritative_projection() {
        let mut online = session();
        let command = online
            .on_input(&InputEvent::Key {
                key: Key::Enter,
                pressed: true,
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            canonical_decode::<u8>(command.command().payload()).unwrap(),
            9
        );
        assert_eq!(online.description().as_deref(), Some("1"));
        assert!(online
            .on_input(&InputEvent::Key {
                key: Key::Enter,
                pressed: true
            })
            .unwrap()
            .is_none());
        online
            .receive(
                &[
                    ServerEnvelope::new(None, 2, ServerMessage::Ack { seq: 1 }).unwrap(),
                    update(3, 1, 2, vec![canonical_encode(&3u8).unwrap()]),
                ],
                &frame(),
            )
            .unwrap();
        assert_eq!(online.description().as_deref(), Some("2"));
        assert_eq!(online.local.events, 3);
    }
    #[test]
    fn malformed_later_event_is_atomic_and_disconnects_before_any_view_is_replaced() {
        let mut online = session();
        assert!(online
            .receive(
                &[
                    update(2, 1, 2, Vec::new()),
                    update(3, 2, 3, vec![vec![0xff]])
                ],
                &frame()
            )
            .is_err());
        assert_eq!(online.description(), None);
        assert_eq!(online.local.events, 0);
        assert_eq!(online.state(), DirectState::Disconnected);
        assert!(online
            .on_input(&InputEvent::Key {
                key: Key::Enter,
                pressed: true
            })
            .unwrap()
            .is_none());
    }
    #[test]
    fn pending_or_disconnected_input_keeps_release_cleanup_without_sending() {
        let mut online = session();
        online
            .on_input(&InputEvent::Key {
                key: Key::Enter,
                pressed: true,
            })
            .unwrap();
        assert!(online.local.held);
        assert!(online
            .on_input(&InputEvent::Key {
                key: Key::Enter,
                pressed: false
            })
            .unwrap()
            .is_none());
        assert!(!online.local.held);
        online.disconnect();
        assert_eq!(online.description(), None);
    }
    #[test]
    fn authority_loss_drops_projection_and_never_calls_game_presentation_again() {
        let mut online = session();
        assert!(online.present(&frame()).is_some());
        assert_eq!(online.description().as_deref(), Some("1"));
        online.local_mut().held = true;
        online.disconnect();
        assert!(online.view.is_none());
        assert!(!online.local.held);
        online.local_mut().forbid_presentation = true;
        assert!(online.present(&frame()).is_none());
        assert_eq!(online.description(), None);
        // A delayed success cannot restore a retired attachment.
        assert!(online
            .receive(&[update(2, 1, 2, Vec::new())], &frame())
            .is_err());
        online.local_mut().forbid_presentation = true;
        assert!(online.present(&frame()).is_none());
        assert_eq!(online.description(), None);
    }
    #[test]
    fn initial_attachment_requires_one_snapshot_instead_of_choosing_stale_view() {
        assert!(OnlineMatch::<NeverLocalAuthority, Presenter>::new(
            MatchId(7),
            GameId::new("org.example.game").unwrap(),
            GameVersion::new("1.0.0").unwrap(),
            1,
            &[update(1, 0, 1, Vec::new()), update(2, 1, 2, Vec::new())]
        )
        .is_err());
    }
}
