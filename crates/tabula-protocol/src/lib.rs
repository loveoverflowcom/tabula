//! # `tabula-protocol` — the wire
//!
//! > ## PHASE 4 — DO NOT IMPLEMENT BEFORE PHASE 3 EXITS
//! >
//! > Gate: four games pass conformance, projection scans are green, and
//! > **no change was required to `tabula-core`/`tabula-game-api` in the final
//! > two weeks of Phase 3** — i.e. the contract has stopped moving.
//! > (doc 07 Phase 3 exit criteria)
//! >
//! > Doc 09 §7 is explicit about why: *"the networking gets built against a
//! > contract that has not yet been validated by real games, and then the
//! > contract cannot move."* The wire protocol is the hardest thing in this
//! > repository to change after launch — every client in the wild depends on it.
//!
//! Shared verbatim by the server, the native client, the WASM client, the load
//! generator, and any future non-Rust client. **It must compile on wasm32 with
//! no runtime.**
//!
//! ## Envelopes (doc 05 §2)
//!
//! ```rust,ignore
//! pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
//!
//! pub struct ClientEnvelope {
//!     pub v: ProtocolVersion,     // rejected if major differs from negotiated
//!     pub seq: u32,               // client-assigned, monotonic per (session, match)
//!     pub corr: CorrelationId,    // echoed in every caused response; also the trace id
//!     pub body: ClientMessage,
//! }
//!
//! pub enum ClientMessage {
//!     Hello { protocol, client: ClientIdent, auth: AuthCredential, codec: Codec },
//!     Platform(PlatformCommand),
//!     Game(GameCommandFrame),     // the ONLY message carrying game-specific bytes
//!     Ping { nonce: u32 },
//!     Bye { reason: ByeReason },
//! }
//!
//! pub struct GameCommandFrame {
//!     pub match_id: MatchId,
//!     pub game: GameId, pub game_version: GameVersion,  // both, so a mismatch is
//!                                                        // a clean error not a mis-decode
//!     pub at: StateVersion,       // advisory: lets the server detect a stale client.
//!                                 // NOT optimistic concurrency.
//!     #[serde(with = "serde_bytes")]
//!     pub payload: Vec<u8>,       // OPAQUE. Decoded by ErasedMatch inside the actor.
//! }
//!
//! pub struct ServerEnvelope {
//!     pub v: ProtocolVersion,
//!     pub corr: Option<CorrelationId>,
//!     pub frame: u32,             // server monotonic per connection; detects dropped frames
//!     pub body: ServerMessage,
//! }
//!
//! pub enum ServerMessage {
//!     HelloAck { session, server_time_ms, protocol, limits: Limits, features: FeatureFlags },
//!     Platform(PlatformEvent),
//!     MatchUpdate(MatchUpdateFrame),   // projected view: attach, resync, rules change
//!     Game(GameEventFrame),            // incremental redacted events
//!     Ack { seq: u32, at: StateVersion },
//!     Reject { seq: u32, error: ProtocolError },
//!     Pong { nonce: u32 },
//!     Draining { retry_after_ms: u32 },  // reconnect immediately; Close(4411) follows
//! }
//! ```
//!
//! ## ADR-008 in one sentence
//!
//! Game payloads are **opaque bytes tagged by `(game_id, game_version)`**. The
//! platform routes without knowing games (I-9); games stay strongly typed. That
//! is what avoids a universal `GameState` mega-enum, which would make every game
//! a compile-time dependency of every other game.
//!
//! ## Postcard is positional — five rules that are not negotiable (doc 05 §3.3)
//!
//! 1. Never reorder, insert-in-the-middle, or remove a field or variant without
//!    a protocol version bump. **Append only.**
//! 2. Enum variants appended at the end, never renumbered.
//! 3. `Option<T>` is the supported way to add a field — **at the end**.
//! 4. **Never** `#[serde(flatten)]`, untagged enums, or `serde_json::Value` in a
//!    wire type.
//! 5. Every wire type has a golden vector.
//!
//! ## Hostile input limits (doc 05 §9.1)
//!
//! Decoders face hostile input directly — they are the fuzz target.
//!
//! ```text
//! max inbound frame    64 KiB
//! max game payload     16 KiB
//! max outbound frame    1 MiB
//! every Vec / String in a wire type has a bounded deserialize_with helper
//! JSON mode: NO deny_unknown_fields (a newer client must be able to add fields)
//! ```
//!
//! ## Codec negotiation (doc 05 §4.1)
//!
//! Subprotocol strings: `tabula.v1.postcard` (binary WS frames, production) and
//! `tabula.v1.json` (text frames, dev). JSON is gated to staff accounts or
//! `TABULA_ALLOW_JSON_WS=1` and refused in production otherwise.
//!
//! JSON is a **developer-experience requirement**, not a nice-to-have: being
//! able to read traffic in browser devtools and `websocat` materially changes
//! debugging speed. It is also **never** a storage format — canonical encoding
//! is always Postcard (doc 05 §4.3).
//!
//! ## I-13: golden vectors are the version gate
//!
//! `tests/vectors/` holds, per protocol version, one file per wire type:
//! the constructed value, its Postcard bytes as hex, and its JSON. CI decodes
//! and re-encodes each, asserting byte equality.
//!
//! ```text
//! Changing a wire type ⇒ vectors change ⇒ CI fails ⇒ the author must either
//!    (a) revert, or
//!    (b) run `xtask gen-protocol-vectors --bump minor|major`, which also
//!        updates PROTOCOL_VERSION and appends to
//!        docs/architecture/protocol-changelog.md
//! ```
//!
//! ## Security tests this crate must carry (doc 05 §9.3)
//!
//! | Test | Assertion |
//! |---|---|
//! | `no_state_type_on_the_wire` | A `NeverOnWire` marker trait; `State` types implement it, wire types must not contain one |
//! | `spectator_frames_match_spectator_projection` | Every spectator frame equals `project(state, Spectator)` |
//! | `seat_frames_never_contain_other_seats_secrets` | Per seat, against the `SecretModel` |
//! | `resync_equals_fold` | Resync view at *v* equals the fold of `ViewEvent`s since the last `MatchUpdate` |
//! | `join_token_scope` | A token for match A cannot attach to B |
//! | `audit_viewer_unreachable` | `Viewer::Audit` is not constructible from a gateway session |
//!
//! ## ⚠ Open contradiction to resolve BEFORE writing code
//!
//! `MatchUpdateFrame` carries `capabilities: GameCapabilities` (doc 05 §2), but
//! `GameCapabilities` lives in `tabula-game-api`, which this crate **may not
//! depend on** (doc 01 §3). Two ways out:
//!
//! 1. Move `GameCapabilities` down into `tabula-core`.
//! 2. Carry an encoded mirror type on the frame, converted by `tabula-registry`.
//!
//! Option 2 preserves the layering; option 1 is simpler. **Write the ADR before
//! writing the code** — this is exactly the kind of thing that gets decided by
//! whoever types first and is then permanent.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/envelope.rs   ClientEnvelope, ServerEnvelope
//! src/client.rs     ClientMessage, PlatformCommand, GameCommandFrame
//! src/server.rs     ServerMessage, PlatformEvent, MatchUpdateFrame, GameEventFrame
//! src/version.rs    ProtocolVersion, negotiation, support window
//! src/codec.rs      Codec enum (postcard | json), subprotocol strings
//! src/error.rs      ErrorCode, ProtocolError, WS close codes
//! src/limits.rs     frame/payload caps, bounded deserialize helpers
//! tests/vectors/    golden wire vectors, one file per type per version
//! ```

#![forbid(unsafe_code)]
