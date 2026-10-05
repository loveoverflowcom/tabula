//! Typed game authority behind the generic match boundary (doc 02 §8).
//!
//! ADR-0039 opens the isolated canonical offline authority bridge; ADR-0040
//! adds its bounded server-only persistence/recovery operations. Canonical state
//! stays private; client reads pass through `project` and `view_event` (I-5/I-6).

use core::fmt;

use serde::{de::DeserializeOwned, Serialize};
use tabula_core::{
    canonical_decode, canonical_encode, CanonicalError, DetRng, GameId, GameVersion, InputIndex,
    LogicalTime, MatchSeed, RuleError, RulesVersion, SeatChange, SeatId, SeatRoster, SpectatorTier,
    StateHash, TimerId, Viewer,
};
use tabula_game_api::{
    AdminInput, ConfigError, Ctx, Effect, GameModule, GameRules, InitError, Input,
};

/// Maximum canonical config or player-command bytes admitted by this bridge.
/// The wire and actor apply the same command bound (doc 05 §9.1).
pub const MAX_RUNTIME_PAYLOAD_BYTES: usize = tabula_protocol::MAX_GAME_PAYLOAD_BYTES;
/// Maximum canonical or projected event bytes accepted at this boundary.
pub const MAX_RUNTIME_EVENT_BYTES: usize = tabula_protocol::MAX_EVENT_BYTES;
/// Maximum events from one accepted input or match creation.
pub const MAX_RUNTIME_EVENTS: usize = tabula_protocol::MAX_EVENTS;
/// Maximum bytes in a client projection.
pub const MAX_RUNTIME_VIEW_BYTES: usize = tabula_protocol::MAX_VIEW_BYTES;
/// Maximum server-only canonical state in the isolated durable bridge (ADR-0040).
/// Larger states require a separately authorized snapshot-storage policy
/// (doc 03 §9.2); this does not change any client payload limit.
pub const MAX_RUNTIME_STATE_BYTES: usize = 1024 * 1024;
/// Maximum canonical recorded input, allowing bounded stream framing around
/// the existing player-command limit (doc 05 §8.1).
pub const MAX_RUNTIME_INPUT_BYTES: usize = MAX_RUNTIME_PAYLOAD_BYTES + 16;

/// Immutable rules/package identity bound when a match is created (I-16).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuntimeIdentity {
    pub game: GameId,
    pub game_version: GameVersion,
    pub rules_version: RulesVersion,
    pub rules_hash: [u8; 32],
}

/// A client-authorized live projection identity, with no audit conversion.
/// Authorization of seat ownership belongs to the actor; roster membership is
/// also checked by the bridge. Delayed spectators are outside ADR-0039.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientViewer {
    Seat(SeatId),
    Spectator,
}

/// The single ordered stream, keeping player commands opaque (ADR-003/008).
#[derive(Clone, Debug)]
pub enum ErasedInput {
    Player { seat: SeatId, payload: Vec<u8> },
    Timer { timer: TimerId },
    Seat { seat: SeatId, change: SeatChange },
    Admin(AdminInput),
}

/// A successfully created authority and its server-only initial records.
/// This is not a client-output type: events and effects remain actor-owned.
/// Authority and seed cannot be changed before the actor records initialization.
///
/// ```compile_fail
/// fn alter_initial_authority(created: &mut tabula_registry::CreatedMatch) {
///     created.runtime_mut();
/// }
/// ```
///
/// ```compile_fail
/// fn swap_seed(created: &mut tabula_registry::CreatedMatch, seed: tabula_core::MatchSeed) {
///     created.seed = seed;
/// }
/// ```
pub struct CreatedMatch {
    runtime: Box<dyn ErasedMatch>,
    seed: MatchSeed,
    events: Vec<Vec<u8>>,
    effects: Vec<Effect>,
}

/// Actor-owned creation records, including the seed used by initialization.
/// Never exposed as a client-output type (I-5/I-16).
pub type CreatedMatchParts = (Box<dyn ErasedMatch>, MatchSeed, Vec<Vec<u8>>, Vec<Effect>);

impl CreatedMatch {
    /// The created authority, for server-side inspection only.
    pub fn runtime(&self) -> &dyn ErasedMatch {
        self.runtime.as_ref()
    }

    /// Canonical server-only initial events, in order.
    pub fn events(&self) -> &[Vec<u8>] {
        &self.events
    }

    /// Initial platform requests, executed only after initialization is committed.
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    /// Transfer authority, its bound RNG seed, initial events and initial effects
    /// to the sole owning actor. Private fields prevent mismatched seed assembly.
    pub fn into_parts(self) -> CreatedMatchParts {
        (self.runtime, self.seed, self.events, self.effects)
    }
}

impl fmt::Debug for CreatedMatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreatedMatch")
            .field("identity", self.runtime.identity())
            .field("events", &self.events.len())
            .field("effects", &self.effects.len())
            .finish_non_exhaustive()
    }
}

/// A successful transition's canonical server-only records (doc 03 §7).
/// Client output is constructed separately using the redaction methods below.
#[derive(Debug)]
pub struct ErasedTransition {
    pub canonical_input: Vec<u8>,
    pub events: Vec<Vec<u8>>,
    pub effects: Vec<Effect>,
}

/// Bridge failures keep malformed payloads separate from game-rule rejections.
/// Error details are server-side; the actor emits only public-safe error codes.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("malformed canonical payload: {0}")]
    Malformed(#[source] CanonicalError),
    #[error("invalid configuration: {0}")]
    Config(#[source] ConfigError),
    #[error("match initialization failed: {0}")]
    Init(#[source] InitError),
    #[error("rules rejected input: {0}")]
    Rule(#[source] RuleError),
    #[error("seat {0:?} is outside the match roster")]
    UnknownSeat(SeatId),
    #[error("runtime payload, input, state, event, or projection exceeded its bound")]
    LimitExceeded,
    #[error("canonical serialization failed: {0}")]
    Serialization(#[source] CanonicalError),
    #[error("module metadata and rules implementation have different rules versions")]
    IdentityMismatch,
    #[error("stored match identity differs from the linked game package and rules")]
    RestoreIdentityMismatch,
}

/// One actor's exclusively owned game authority (I-14).
/// Canonical snapshots and replay are server-only persistence operations.
/// The client-facing methods accept only [`ClientViewer`], never `Viewer::Audit`.
///
/// ```compile_fail
/// fn request_audit(runtime: &dyn tabula_registry::ErasedMatch) {
///     runtime.project(tabula_core::Viewer::Audit);
/// }
/// ```
pub trait ErasedMatch: Send {
    fn identity(&self) -> &RuntimeIdentity;
    fn roster(&self) -> &SeatRoster;
    /// Original validated canonical creation config, retained immutably for
    /// deterministic reconstruction (I-16; doc 05 §8.1).
    ///
    /// ```compile_fail
    /// fn change_config(runtime: &mut dyn tabula_registry::ErasedMatch) {
    ///     runtime.creation_config()[0] = 2;
    /// }
    /// ```
    fn creation_config(&self) -> &[u8];
    /// Encode canonical state for server-owned persistence only (I-5; ADR-0040).
    /// Never use this value in a client-output DTO.
    fn snapshot(&self) -> Result<Vec<u8>, RuntimeError>;
    fn apply(
        &mut self,
        input: ErasedInput,
        now: LogicalTime,
        index: InputIndex,
        rng: &mut DetRng,
    ) -> Result<ErasedTransition, RuntimeError>;
    /// Replay one accepted canonical input at its exact recorded logical time,
    /// input index, and per-input RNG root (doc 05 §8.1; ADR-0040). The actor verifies
    /// resulting events and state hashes against the durable records.
    fn replay(
        &mut self,
        canonical_input: &[u8],
        now: LogicalTime,
        index: InputIndex,
        rng: &mut DetRng,
    ) -> Result<ErasedTransition, RuntimeError>;
    fn project(&self, viewer: ClientViewer) -> Result<Vec<u8>, RuntimeError>;
    fn view_events(
        &self,
        events: &[Vec<u8>],
        viewer: ClientViewer,
    ) -> Result<Vec<Vec<u8>>, RuntimeError>;
    fn state_hash(&self) -> StateHash;
}

/// The one generic typed implementation, usable by catalog adapters and fixtures.
/// Its state is private so there is no canonical-state client output (I-5).
pub struct TypedMatch<M: GameModule> {
    identity: RuntimeIdentity,
    roster: SeatRoster,
    creation_config: Vec<u8>,
    state: <M::Rules as GameRules>::State,
}

impl<M: GameModule> fmt::Debug for TypedMatch<M> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypedMatch")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl<M: GameModule> TypedMatch<M> {
    /// Validate the canonical config through the module before rules creation.
    /// Initialization uses logical time/index zero; accepted actor inputs start
    /// at index one, so creation and later RNG streams remain separate (I-4).
    pub fn create(
        config: &[u8],
        roster: &SeatRoster,
        seed: MatchSeed,
    ) -> Result<CreatedMatch, RuntimeError> {
        let typed_config = decode_payload::<<M::Rules as GameRules>::Config>(config)?;
        M::validate_config(&typed_config, roster).map_err(RuntimeError::Config)?;
        let identity = Self::linked_identity()?;
        let mut rng = DetRng::for_input(&seed, InputIndex(0));
        let init = M::Rules::create(
            &typed_config,
            roster,
            &mut Ctx {
                now: LogicalTime(0),
                index: InputIndex(0),
                rng: &mut rng,
                budget: M::capabilities().apply_budget(),
            },
        )
        .map_err(RuntimeError::Init)?;
        // Fail before returning a live authority if its state/events cannot be
        // encoded. State bytes remain private and are never a client payload.
        encode_state(&init.state)?;
        let events = encode_events(init.events.iter())?;
        Ok(CreatedMatch {
            runtime: Box::new(Self {
                identity,
                roster: roster.clone(),
                creation_config: config.to_vec(),
                state: init.state,
            }),
            seed,
            events,
            effects: init.effects.into_vec(),
        })
    }

    /// Restore a server-owned canonical snapshot under its exact immutable
    /// package/rules identity (I-16; doc 05 §§7–8; ADR-0040). No migration or version
    /// substitution occurs. The actor must verify the returned state hash and
    /// reconcile this snapshot with deterministic replay before using it.
    pub fn restore(
        identity: &RuntimeIdentity,
        config: &[u8],
        roster: &SeatRoster,
        snapshot: &[u8],
    ) -> Result<Box<dyn ErasedMatch>, RuntimeError> {
        let linked_identity = Self::linked_identity()?;
        if identity != &linked_identity {
            return Err(RuntimeError::RestoreIdentityMismatch);
        }
        let typed_config = decode_payload::<<M::Rules as GameRules>::Config>(config)?;
        M::validate_config(&typed_config, roster).map_err(RuntimeError::Config)?;
        let state = decode_bounded(snapshot, MAX_RUNTIME_STATE_BYTES)?;
        Ok(Box::new(Self {
            identity: linked_identity,
            roster: roster.clone(),
            creation_config: config.to_vec(),
            state,
        }))
    }

    fn linked_identity() -> Result<RuntimeIdentity, RuntimeError> {
        let metadata = M::metadata();
        if metadata.rules_version() != M::Rules::RULES_VERSION {
            return Err(RuntimeError::IdentityMismatch);
        }
        Ok(RuntimeIdentity {
            game: metadata.id().clone(),
            game_version: metadata.version().clone(),
            rules_version: metadata.rules_version(),
            rules_hash: M::rules_hash(),
        })
    }

    fn require_seat(&self, seat: SeatId) -> Result<(), RuntimeError> {
        self.roster
            .get(seat)
            .map(|_| ())
            .ok_or(RuntimeError::UnknownSeat(seat))
    }

    fn viewer(&self, viewer: ClientViewer) -> Result<Viewer, RuntimeError> {
        match viewer {
            ClientViewer::Seat(seat) => {
                self.require_seat(seat)?;
                Ok(Viewer::Seat(seat))
            }
            ClientViewer::Spectator => Ok(Viewer::Spectator(SpectatorTier::Live)),
        }
    }

    fn decode_input(
        &self,
        input: ErasedInput,
    ) -> Result<Input<<M::Rules as GameRules>::Command>, RuntimeError> {
        Ok(match input {
            ErasedInput::Player { seat, payload } => {
                self.require_seat(seat)?;
                Input::Player {
                    seat,
                    command: decode_payload(&payload)?,
                }
            }
            ErasedInput::Timer { timer } => Input::Timer { timer },
            ErasedInput::Seat { seat, change } => {
                self.require_seat(seat)?;
                Input::Seat { seat, change }
            }
            ErasedInput::Admin(input) => Input::Admin(input),
        })
    }

    fn apply_typed(
        &mut self,
        input: Input<<M::Rules as GameRules>::Command>,
        now: LogicalTime,
        index: InputIndex,
        rng: &mut DetRng,
    ) -> Result<ErasedTransition, RuntimeError> {
        match &input {
            Input::Player { seat, command } => {
                self.require_seat(*seat)?;
                let encoded = canonical_encode(command).map_err(RuntimeError::Serialization)?;
                require_size(&encoded, MAX_RUNTIME_PAYLOAD_BYTES)?;
            }
            Input::Seat { seat, .. } => self.require_seat(*seat)?,
            Input::Timer { .. } | Input::Admin(_) => {}
        }
        let canonical_input = canonical_encode(&input).map_err(RuntimeError::Serialization)?;
        require_size(&canonical_input, MAX_RUNTIME_INPUT_BYTES)?;
        // Defensive R2/R8 containment for a module that mutates or draws before
        // rejecting. A panic also unwinds these candidates without committing;
        // catching and terminating the match belongs to its actor (doc 03 §6.4).
        let mut state = self.state.clone();
        let mut candidate_rng = rng.clone();
        let outcome = M::Rules::apply(
            &mut state,
            input,
            &mut Ctx {
                now,
                index,
                rng: &mut candidate_rng,
                budget: M::capabilities().apply_budget(),
            },
        )
        .map_err(RuntimeError::Rule)?;
        let events = encode_events(outcome.events.iter())?;
        encode_state(&state)?;
        self.state = state;
        *rng = candidate_rng;
        Ok(ErasedTransition {
            canonical_input,
            events,
            effects: outcome.effects.into_vec(),
        })
    }
}

impl<M: GameModule> ErasedMatch for TypedMatch<M> {
    fn identity(&self) -> &RuntimeIdentity {
        &self.identity
    }

    fn roster(&self) -> &SeatRoster {
        &self.roster
    }

    fn creation_config(&self) -> &[u8] {
        &self.creation_config
    }

    fn snapshot(&self) -> Result<Vec<u8>, RuntimeError> {
        encode_state(&self.state)
    }

    fn apply(
        &mut self,
        input: ErasedInput,
        now: LogicalTime,
        index: InputIndex,
        rng: &mut DetRng,
    ) -> Result<ErasedTransition, RuntimeError> {
        let input = self.decode_input(input)?;
        self.apply_typed(input, now, index, rng)
    }

    fn replay(
        &mut self,
        canonical_input: &[u8],
        now: LogicalTime,
        index: InputIndex,
        rng: &mut DetRng,
    ) -> Result<ErasedTransition, RuntimeError> {
        let input = decode_bounded(canonical_input, MAX_RUNTIME_INPUT_BYTES)?;
        self.apply_typed(input, now, index, rng)
    }

    fn project(&self, viewer: ClientViewer) -> Result<Vec<u8>, RuntimeError> {
        let view = M::Rules::project(&self.state, self.viewer(viewer)?);
        let encoded = canonical_encode(&view).map_err(RuntimeError::Serialization)?;
        require_size(&encoded, MAX_RUNTIME_VIEW_BYTES)?;
        Ok(encoded)
    }

    fn view_events(
        &self,
        events: &[Vec<u8>],
        viewer: ClientViewer,
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        let viewer = self.viewer(viewer)?;
        if events.len() > MAX_RUNTIME_EVENTS {
            return Err(RuntimeError::LimitExceeded);
        }
        let mut projected = Vec::with_capacity(events.len());
        for event in events {
            let event = decode_payload::<<M::Rules as GameRules>::Event>(event)?;
            if let Some(view) = M::Rules::view_event(&self.state, &event, viewer) {
                let encoded = canonical_encode(&view).map_err(RuntimeError::Serialization)?;
                require_size(&encoded, MAX_RUNTIME_EVENT_BYTES)?;
                projected.push(encoded);
            }
        }
        Ok(projected)
    }

    fn state_hash(&self) -> StateHash {
        M::Rules::state_hash(&self.state)
    }
}

fn require_size(bytes: &[u8], maximum: usize) -> Result<(), RuntimeError> {
    if bytes.len() > maximum {
        return Err(RuntimeError::LimitExceeded);
    }
    Ok(())
}

fn decode_payload<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, RuntimeError> {
    decode_bounded(bytes, MAX_RUNTIME_PAYLOAD_BYTES)
}

fn decode_bounded<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, RuntimeError> {
    require_size(bytes, maximum)?;
    let value = canonical_decode(bytes).map_err(RuntimeError::Malformed)?;
    let encoded = canonical_encode(&value).map_err(RuntimeError::Serialization)?;
    if encoded != bytes {
        return Err(RuntimeError::Malformed(CanonicalError::Decode(
            "noncanonical payload",
        )));
    }
    Ok(value)
}

fn encode_state<T: Serialize>(state: &T) -> Result<Vec<u8>, RuntimeError> {
    let encoded = canonical_encode(state).map_err(RuntimeError::Serialization)?;
    require_size(&encoded, MAX_RUNTIME_STATE_BYTES)?;
    Ok(encoded)
}

fn encode_events<'a, T: Serialize + 'a>(
    events: impl ExactSizeIterator<Item = &'a T>,
) -> Result<Vec<Vec<u8>>, RuntimeError> {
    if events.len() > MAX_RUNTIME_EVENTS {
        return Err(RuntimeError::LimitExceeded);
    }
    events
        .map(|event| {
            let encoded = canonical_encode(event).map_err(RuntimeError::Serialization)?;
            require_size(&encoded, MAX_RUNTIME_EVENT_BYTES)?;
            Ok(encoded)
        })
        .collect()
}

/// Approved typed-game fixtures for platform integration tests. This explicit
/// opt-in keeps game names/command shapes inside the registry (I-9).
#[cfg(all(feature = "test-support", feature = "game-chess"))]
pub mod test_support {
    use std::sync::Arc;

    use tabula_core::{
        canonical_encode, MatchSeed, Millis, Occupant, SeatEntry, SeatId, SeatRoster, UserId,
    };
    use tabula_game_api::GameModule;
    use tabula_game_chess::{ChessModule, ClockConfig, ClockControl, Command, Config};

    use crate::ErasedGame;

    /// Registry-owned real-game transcript expressed as opaque platform inputs.
    pub struct RuntimeFixture {
        pub game: Arc<dyn ErasedGame>,
        pub config: Vec<u8>,
        pub roster: SeatRoster,
        pub seed: MatchSeed,
        pub commands: Vec<(SeatId, Vec<u8>)>,
        pub illegal_command: Vec<u8>,
    }

    impl core::fmt::Debug for RuntimeFixture {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter
                .debug_struct("RuntimeFixture")
                .field("game", self.game.metadata().id())
                .field("commands", &self.commands.len())
                .finish_non_exhaustive()
        }
    }

    /// A real clocked match starting with one second per seat and no increment.
    /// It shares the approved finite transcript, roster and seed, so actor
    /// time-anchor tests need no game names or command schemas (I-9).
    #[must_use]
    pub fn approved_clocked_fixture() -> RuntimeFixture {
        let mut fixture = approved_checkmate_fixture();
        fixture.config = canonical_encode(&Config {
            clock: Some(ClockConfig {
                initial: Millis(1000),
                control: ClockControl::Fischer {
                    increment: Millis(0),
                },
            }),
        })
        .expect("derived clocked fixture config is canonical");
        fixture
    }

    /// Twenty-four accepted nonterminal inputs for snapshot-cadence and
    /// partial-history recovery checks. The unclocked Ruy Lopez opening stays
    /// game-owned and opaque to platform tests (I-9).
    #[must_use]
    pub fn approved_long_fixture() -> RuntimeFixture {
        let mut fixture = approved_checkmate_fixture();
        fixture.commands = [
            (12, 28), // e4
            (52, 36), // e5
            (6, 21),  // Nf3
            (57, 42), // Nc6
            (5, 33),  // Bb5
            (48, 40), // a6
            (33, 24), // Ba4
            (62, 45), // Nf6
            (4, 6),   // O-O
            (61, 52), // Be7
            (5, 4),   // Re1
            (49, 33), // b5
            (24, 17), // Bb3
            (51, 43), // d6
            (10, 18), // c3
            (60, 62), // O-O
            (15, 23), // h3
            (42, 57), // Nb8
            (11, 27), // d4
            (57, 51), // Nbd7
            (1, 11),  // Nbd2
            (50, 34), // c5
            (27, 35), // d5
            (34, 26), // c4
        ]
        .into_iter()
        .zip([SeatId(0), SeatId(1)].into_iter().cycle())
        .map(|((from, to), seat)| {
            (
                seat,
                canonical_encode(&Command::Move {
                    from,
                    to,
                    promotion: None,
                })
                .expect("derived long fixture command is canonical"),
            )
        })
        .collect();
        fixture
    }

    /// A finite approved match ending after four valid player commands, plus
    /// a syntactically valid illegal command for rejection checks.
    #[must_use]
    pub fn approved_checkmate_fixture() -> RuntimeFixture {
        let game = crate::registered_games()
            .into_iter()
            .find(|game| game.metadata().id() == ChessModule::metadata().id())
            .expect("the feature-linked fixture module is registered");
        let commands = [(0, 13, 21), (1, 52, 36), (0, 14, 30), (1, 59, 31)]
            .into_iter()
            .map(|(seat, from, to)| {
                (
                    SeatId(seat),
                    canonical_encode(&Command::Move {
                        from,
                        to,
                        promotion: None,
                    })
                    .expect("derived fixture command is canonical"),
                )
            })
            .collect();
        RuntimeFixture {
            game,
            config: canonical_encode(&Config::default())
                .expect("derived fixture config is canonical"),
            roster: SeatRoster::new(
                (0u8..2)
                    .map(|seat| SeatEntry {
                        seat: SeatId(seat),
                        occupant: Occupant::Human(UserId(u128::from(seat) + 1)),
                        team: None,
                    })
                    .collect(),
            )
            .expect("fixture has distinct seats"),
            seed: MatchSeed::from_bytes([51; 32]),
            commands,
            illegal_command: canonical_encode(&Command::Move {
                from: 255,
                to: 254,
                promotion: None,
            })
            .expect("derived hostile fixture command is canonical"),
        }
    }
}
