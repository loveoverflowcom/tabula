//! # `tabula-registry` — the catalog and the only bridge to games
//!
//! > ## PHASE 4 — DO NOT IMPLEMENT BEFORE PHASE 3 EXITS
//!
//! This is the **only** crate that knows the set of games exists. That
//! containment is what makes I-9 mechanically checkable: every platform crate
//! depends on this crate's *interfaces*, never on a game.
//!
//! `xtask check-no-game-ids` greps `crates/` and `services/` for game id
//! literals and `games::` imports, with this crate as the sole exemption.
//!
//! ## Type erasure (doc 02 §8)
//!
//! ```text
//! typed world              the bridge                  game-agnostic platform
//! ChessRules: GameRules → GameAdapter<ChessModule> → MatchActor holds
//!   State/Command/...       blanket impl of              Box<dyn ErasedMatch>
//!                           ErasedGame + codec
//! ```
//!
//! ```rust,ignore
//! /// Object-safe façade over a GameModule. ONE implementation, generic over M.
//! pub trait ErasedGame: Send + Sync {
//!     fn metadata(&self) -> &'static GameMetadata;
//!     fn capabilities(&self) -> &'static GameCapabilities;
//!     fn validate_config(&self, cfg: &RawValue, codec: Codec) -> Result<(), ConfigError>;
//!     fn create_match(&self, cfg: &RawValue, codec: Codec, roster: &SeatRoster,
//!                     seed: &MatchSeed) -> Result<(Box<dyn ErasedMatch>, ErasedInit), CreateError>;
//!     fn restore_match(&self, snapshot: &[u8], from: RulesVersion)
//!         -> Result<Box<dyn ErasedMatch>, RestoreError>;
//!     fn bot(&self, level: BotLevel) -> Option<Box<dyn ErasedBot>>;
//! }
//!
//! /// One live match's state, type-erased. Owned EXCLUSIVELY by one actor (I-14).
//! pub trait ErasedMatch: Send {
//!     /// Malformed is a PROTOCOL error, not a rule error — the platform must be
//!     /// able to tell "garbage" from "illegal" for abuse counting.
//!     fn decode_command(&self, payload: &[u8], codec: Codec) -> Result<ErasedCommand, DecodeError>;
//!     fn apply(&mut self, input: ErasedInput, ctx: &mut Ctx<'_>) -> Result<ErasedOutcome, RuleError>;
//!     /// Per-viewer redactions of the events from the LAST successful apply.
//!     fn view_events(&self, viewer: Viewer, codec: Codec) -> Result<Vec<Bytes>, CodecError>;
//!     fn project(&self, viewer: Viewer, codec: Codec) -> Result<Bytes, CodecError>;
//!     fn snapshot(&self) -> Result<Bytes, CodecError>;
//!     fn state_hash(&self) -> StateHash;
//!     fn rules_version(&self) -> RulesVersion;
//!     fn describe(&self, viewer: Viewer) -> A11yDescription;
//!     fn legal_commands(&self, seat: SeatId, codec: Codec) -> Option<Bytes>;
//! }
//!
//! pub struct ErasedOutcome {
//!     pub canonical_events: Vec<Bytes>,   // for the append-only log, in order
//!     pub effects: SmallVec<[Effect; 2]>,
//!     pub state_version: StateVersion,
//!     pub state_hash: Option<StateHash>,  // Some at checkpoint intervals
//! }
//! ```
//!
//! ### Four design notes worth keeping
//!
//! - **Why not one big `GameState` enum?** It would make every game a
//!   compile-time dependency of every other game and of the platform,
//!   re-version everything on any change, and create a permanent merge hotspot.
//!   Erasure costs one vtable call per input — irrelevant at board-game rates.
//! - **Why does `ErasedMatch` own the state?** So the actor never names the state
//!   type, and so the single-owner invariant (I-14) is expressed by Rust
//!   ownership rather than by convention.
//! - **Why keep the last events inside?** Redaction needs typed events.
//!   Re-decoding canonical bytes per viewer would be wasteful and would risk
//!   decode/encode asymmetry.
//! - **`Codec` is a parameter, not a global.** The same match may serve a
//!   Postcard production client and a JSON debugging client simultaneously.
//!
//! ## The static registry (doc 02 §8.1)
//!
//! ```rust,ignore
//! tabula_registry::register! {
//!     tabula_game_chess::ChessModule,
//!     tabula_game_cards::CardsModule,
//!     tabula_game_werewolf::WerewolfModule,
//!     tabula_game_tiles::TilesModule,
//!     tabula_game_tictactoe::TicTacToeModule,
//! }
//! ```
//!
//! Generates:
//!
//! ```rust,ignore
//! pub fn all() -> &'static [&'static dyn ErasedGame];
//! pub fn get(id: &GameId, version: Option<&GameVersion>) -> Option<&'static dyn ErasedGame>;
//! pub fn catalog(audience: Audience) -> Vec<&'static GameMetadata>;   // rollout-filtered
//! ```
//!
//! plus a **compile-time uniqueness check on `GameId`**, and a client-side twin
//! registry for `GamePresentation` behind the `presentation` feature.
//!
//! `register!` is also where **build-time exclusion** happens: a cargo feature
//! per game lets a small mobile bundle or a dedicated tournament server link a
//! subset.
//!
//! ## Multiple rules versions, simultaneously
//!
//! When a rules change lands while matches are live, the server links **both**
//! versions: `ChessModuleV1` and `ChessModuleV2`, registered under the same
//! `GameId` with different `rules_version`. The registry resolves by the match's
//! recorded version. Old versions drop once there are no live matches and the
//! replay-support window (default 180 days) has passed. (doc 02 §9.2)
//!
//! **Upgrading a running match's rules is not supported. Ever.**
//!
//! ## Rollout is data, not deployment
//!
//! `rollout.enabled` and `rollout.audience` come from the manifest as defaults
//! and are **overridden by a DB table**. Disabling a game, restricting it to
//! beta/staff/`percentage:10`, or hiding it from the catalog therefore needs
//! **no deploy** — live matches continue to completion either way.
//! (doc 02 §9.1)
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/erased.rs    ErasedGame, ErasedMatch, ErasedOutcome, ErasedBot
//! src/adapter.rs   GameAdapter<M> — the one blanket impl
//! src/codec.rs     bytes <-> typed bridging, the two-arm match on Codec
//! src/manifest.rs  game.toml parsing + validation against compiled metadata
//! src/resolve.rs   game_id@version resolution, multi-version linking
//! src/rollout.rs   enable/disable, audience filtering
//! src/macros.rs    register!
//! ```

#![forbid(unsafe_code)]
