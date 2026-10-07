//! What a client can actually *do* with a game, as opposed to what the game
//! declares.
//!
//! `GameCapabilities` says the rules support bots, ranked play, or async turns.
//! None of that establishes that this build links a bot factory, that a service
//! exists, or that a gameplay runtime is reachable. Availability is therefore a
//! separate, evidence-carrying fact, and every unavailable choice keeps a
//! readable reason and a recovery (docs/ui/screens/discovery.md, "Shared empty,
//! loading, and failure matrix").

/// A way a match can be started from the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LaunchMode {
    /// People taking turns on one device.
    LocalHotSeat,
    /// One person against the game's own bot seats.
    LocalBots,
    /// Participants over the network.
    Network,
}

impl LaunchMode {
    pub const ALL: [Self; 3] = [Self::LocalHotSeat, Self::LocalBots, Self::Network];

    /// Stable query/draft value. Not a product URL on its own.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalHotSeat => "local",
            Self::LocalBots => "bots",
            Self::Network => "network",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == value)
    }

    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::LocalHotSeat => "mode.local.label",
            Self::LocalBots => "mode.bots.label",
            Self::Network => "mode.network.label",
        }
    }

    /// What choosing this mode will do, stated as a consequence.
    #[must_use]
    pub const fn consequence_key(self) -> &'static str {
        match self {
            Self::LocalHotSeat => "mode.local.consequence",
            Self::LocalBots => "mode.bots.consequence",
            Self::Network => "mode.network.consequence",
        }
    }

    /// Whether this mode fills non-first seats with the game's own bots.
    #[must_use]
    pub const fn fills_with_bots(self) -> bool {
        matches!(self, Self::LocalBots)
    }
}

/// Why a mode or action cannot be used, with the recovery the player has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnavailableReason {
    /// The game links no bot factory in this build.
    NoBotFactory,
    /// The package declares bot policies, but no gameplay host runs that mode.
    NoBotRuntime,
    /// No authoritative online service exists at this phase (doc 07 Phase 4).
    NoNetworkService,
    /// The gameplay document this shell would hand off to is not deployed in
    /// this build, so no local session can be started from the browser.
    NoGameplayRuntime,
    /// The bound document does not implement this game's selected local mode.
    NoModeRuntime,
    /// The browser refused the requested separate-document navigation.
    NavigationFailed,
}

impl UnavailableReason {
    #[must_use]
    pub const fn reason_key(self) -> &'static str {
        match self {
            Self::NoBotFactory => "unavailable.no_bot_factory.reason",
            Self::NoBotRuntime => "unavailable.no_bot_runtime.reason",
            Self::NoNetworkService => "unavailable.no_network_service.reason",
            Self::NoGameplayRuntime => "unavailable.no_gameplay_runtime.reason",
            Self::NoModeRuntime => "unavailable.no_mode_runtime.reason",
            Self::NavigationFailed => "unavailable.navigation_failed.reason",
        }
    }

    #[must_use]
    pub const fn recovery_key(self) -> &'static str {
        match self {
            Self::NoBotFactory => "unavailable.no_bot_factory.recovery",
            Self::NoBotRuntime => "unavailable.no_bot_runtime.recovery",
            Self::NoNetworkService => "unavailable.no_network_service.recovery",
            Self::NoGameplayRuntime => "unavailable.no_gameplay_runtime.recovery",
            Self::NoModeRuntime => "unavailable.no_mode_runtime.recovery",
            Self::NavigationFailed => "unavailable.navigation_failed.recovery",
        }
    }
}

/// A mode and the evidence-backed state it is in for this build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeSupport {
    pub mode: LaunchMode,
    pub state: ModeState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeState {
    /// The adapter confirmed the construction path for this mode.
    Available,
    Unavailable(UnavailableReason),
}

impl ModeSupport {
    #[must_use]
    pub const fn available(mode: LaunchMode) -> Self {
        Self {
            mode,
            state: ModeState::Available,
        }
    }

    #[must_use]
    pub const fn unavailable(mode: LaunchMode, reason: UnavailableReason) -> Self {
        Self {
            mode,
            state: ModeState::Unavailable(reason),
        }
    }

    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self.state, ModeState::Available)
    }

    #[must_use]
    pub const fn unavailable_reason(&self) -> Option<UnavailableReason> {
        match self.state {
            ModeState::Available => None,
            ModeState::Unavailable(reason) => Some(reason),
        }
    }
}
