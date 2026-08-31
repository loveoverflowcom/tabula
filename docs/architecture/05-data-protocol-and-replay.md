# 05 — Data, Protocol, and Replay

> Prerequisites: [`00`](./00-architecture-principles.md), [`02`](./02-game-module-and-sdk-design.md),
> [`03`](./03-backend-and-multiplayer-plan.md).
> **Status: LOCK NOW** for the envelope shape, the dual-codec design, and the replay model.

---

## 1. Goals and constraints

| Goal | Consequence |
|---|---|
| The platform routes messages without knowing any game | Game payloads are **opaque bytes tagged by `(game_id, game_version)`** (ADR-008) |
| Games stay strongly typed | Decode happens inside the module, at one point (doc 03 §4.1) |
| Traffic must be debuggable | A JSON codec exists and is a first-class dev path (§4) |
| Bandwidth must be modest on mobile | Postcard binary in production; typical command < 40 bytes |
| A client from three months ago must not corrupt a match | Version negotiation + golden vectors + I-13 |
| Replays must work in five years | Canonical encoding + `rules_hash` + explicit unreplayable marking |
| No secret ever crosses the boundary | Only `View`/`ViewEvent` bytes are ever produced for a client (I-5) |

Non-goals: cross-language third-party clients (would push us toward Protobuf), sub-millisecond
serialization, streaming/chunked payloads, and RPC-style request/response semantics beyond what
`correlation_id` provides.

---

## 2. Message envelopes

```rust
// crates/tabula-protocol/src/lib.rs

pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientEnvelope {
    /// Rejected immediately if major differs from the negotiated version.
    pub v: ProtocolVersion,
    /// Client-assigned, monotonic per (session, match). Used for acks and idempotency.
    pub seq: u32,
    /// Echoed in every response caused by this message; also the tracing correlation id.
    pub corr: CorrelationId,
    pub body: ClientMessage,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    /// First message on any connection.
    Hello {
        protocol: ProtocolVersion,
        client: ClientIdent,          // build, platform, locale
        auth: AuthCredential,         // Bearer session token
        codec: Codec,                 // must match the negotiated subprotocol
    },
    Platform(PlatformCommand),
    /// The only message carrying game-specific bytes.
    Game(GameCommandFrame),
    Ping { nonce: u32 },
    Bye { reason: ByeReason },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GameCommandFrame {
    pub match_id: MatchId,
    /// Both are carried so a mismatch is a clean error rather than a mis-decode.
    pub game: GameId,
    pub game_version: GameVersion,
    /// Client's last known state_version. Advisory: lets the server detect a stale client
    /// and include a resync hint in the rejection. NOT used for optimistic concurrency.
    pub at: StateVersion,
    /// Opaque. Decoded by ErasedMatch::decode_command inside the actor.
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PlatformCommand {
    Attach { match_id: MatchId, join_token: Option<JoinToken>, as_: AttachAs,
             resume_from: Option<StateVersion>, last_client_seq: Option<u32> },
    Detach { match_id: MatchId },
    Chat { channel: ChannelKey, body: String },
    Subscribe(LobbyTopic), Unsubscribe(LobbyTopic),
    ReadyToggle { room_id: RoomId, ready: bool },
    VoiceJoin { channel: ChannelKey }, VoiceLeave { channel: ChannelKey },
    VoiceMute { muted: bool },
    RequestResync { match_id: MatchId },
    RequestLegalMoves { match_id: MatchId },
    Resign { match_id: MatchId },           // platform-level intent; forwarded as the game's own
                                            // resign command if the game defines one, else Admin
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerEnvelope {
    pub v: ProtocolVersion,
    /// Present when this message answers a client message.
    pub corr: Option<CorrelationId>,
    /// Server-side monotonic per connection; lets clients detect dropped frames.
    pub frame: u32,
    pub body: ServerMessage,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    HelloAck { session: SessionId, server_time_ms: u64, protocol: ProtocolVersion,
               limits: Limits, features: FeatureFlags },
    Platform(PlatformEvent),
    /// Snapshot of the projected view. Sent on attach, resync, and rules-version change.
    MatchUpdate(MatchUpdateFrame),
    /// Incremental redacted events.
    Game(GameEventFrame),
    Ack { seq: u32, at: StateVersion },
    Reject { seq: u32, error: ProtocolError },
    Pong { nonce: u32 },
    /// Server is going away; reconnect immediately (close code 4411 follows).
    Draining { retry_after_ms: u32 },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MatchUpdateFrame {
    pub match_id: MatchId,
    pub game: GameId, pub game_version: GameVersion, pub rules_version: RulesVersion,
    pub at: StateVersion,
    pub seat: Option<SeatId>,
    pub viewer: Viewer,
    pub capabilities: GameCapabilities,
    pub roster: SeatRosterView,
    /// Opaque encoding of the game's `View`. (I-5: never `State`.)
    #[serde(with = "serde_bytes")]
    pub view: Vec<u8>,
    pub reason: UpdateReason,     // Attach | Resync | RulesVersionChanged | SpectatorCatchUp
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GameEventFrame {
    pub match_id: MatchId,
    /// The version AFTER applying these events.
    pub at: StateVersion,
    /// Logical time of the input that produced them (drives animation timing and replay).
    pub logical_ms: u64,
    /// Opaque encodings of the game's `ViewEvent`s, in order.
    pub events: Vec<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PlatformEvent {
    RoomUpdate(RoomView), QueueUpdate(QueueState), Presence(PresenceDelta),
    Chat { channel: ChannelKey, msg: ChatMessage },
    ChatScopes { match_id: MatchId, scopes: ChatScopesView },
    VoiceGrant { channel: ChannelKey, url: String, token: String, ice: Vec<IceServer> },
    VoiceScopes { match_id: MatchId, scopes: VoiceScopesView },
    SeatUpdate { match_id: MatchId, seat: SeatId, state: SeatConnState },
    MatchEnded { match_id: MatchId, outcome: MatchOutcomeView, rating_delta: Option<i32> },
    Notice { level: NoticeLevel, key: String, args: BTreeMap<String, String> },
    MatchInvite { room_id: RoomId, from: UserId },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProtocolError {
    pub code: ErrorCode,
    /// Stable i18n key so the client can localize; never raw server text.
    pub key: String,
    /// Present for rule rejections — the game's own code.
    pub rule: Option<RuleErrorCode>,
    /// Set when the client should resync.
    pub resync_at: Option<StateVersion>,
    /// Set for rate limits.
    pub retry_after_ms: Option<u32>,
}
```

### 2.1 Field-by-field rationale

| Field | Why it exists | Why not something else |
|---|---|---|
| `v` on every envelope | Cheap (2 bytes in Postcard), catches version confusion on long-lived sockets | Negotiating once is not enough when proxies or replays re-inject frames |
| `seq` | Ack correlation + idempotency (doc 03 §8.2) | Server-assigned ids would not survive reconnect |
| `corr` | One id through gateway → actor → log → traces; makes distributed debugging possible | Reusing `seq` conflates retry identity with request identity |
| `frame` on server messages | Lets a client detect a gap and request resync without waiting for a timeout | — |
| `game` + `game_version` in the frame | The platform can reject a mismatch *before* decoding, and a replay tool can pick the right decoder | Deriving from `match_id` requires a lookup, and gives no protection against a confused client |
| `at` on commands | Lets rejections include a precise resync hint and detects stale clients | Not used as a CAS token — commands are ordered by the actor, not by the client |
| `logical_ms` on event frames | Animation timing and replay fidelity | Wall-clock timestamps drift between server and client |
| `payload: Vec<u8>` | ADR-008 | A typed enum would drag every game into the protocol crate |

---

## 3. Serialization strategy

### 3.1 The comparison

| Option | Size (chess move) | CPU | Schema evolution | Debuggability | Rust ergonomics | Verdict |
|---|---|---|---|---|---|---|
| **Postcard** | ~24 B | Very fast, no alloc for small types | Positional; field add/remove is breaking unless versioned deliberately | Poor without a tool (we build one, §11) | Excellent — plain `serde` derive | **Recommended for production** |
| JSON (`serde_json`) | ~140 B | Slower, allocates | Tolerant (named fields, `#[serde(default)]`) | Excellent — browser devtools, curl, logs | Excellent | **Recommended for dev/debug** |
| Bincode | ~28 B | Fast | Same positional issue as Postcard, with a less careful varint story | Poor | Good | Rejected: Postcard is smaller and more deliberate about `no_std`/varints |
| MessagePack (`rmp-serde`) | ~45 B | Fast | Tolerant if named | Fair (needs a tool) | Good | Rejected: JSON already covers "tolerant + inspectable"; MsgPack is a middle ground that wins nothing here |
| Protobuf (`prost`) | ~28 B | Fast | **Excellent** — the field-number model is designed for it | Good (well-known tooling) | Mediocre — separate `.proto` files, codegen step, generated types that fight our domain types (no enums-with-data, no `Option<T>` fidelity) | Rejected **for now**; the clear alternative if we ever need non-Rust clients or third-party servers |
| FlatBuffers / Cap'n Proto | ~30 B | Zero-copy read | Good | Poor | Poor for `serde`-shaped domain types | Rejected: zero-copy buys nothing at our message rates |
| CBOR | ~50 B | Fast | Tolerant | Fair | Good | Rejected: no advantage over JSON+Postcard pair |

### 3.2 Recommendation

> **Recommended:** Postcard on the wire in production, JSON in development and debugging, selected
> by WebSocket subprotocol. One `serde` model, two codecs, zero duplicated type definitions.
>
> **Alternative:** Protobuf for the *game payload only*, keeping Postcard/JSON for the envelope.
>
> **Reconsider when** any of: (a) a non-Rust client or third-party game server becomes a real
> requirement; (b) we hit a concrete schema-evolution failure that our versioning discipline (§6)
> cannot handle; (c) payload sizes become a measured cost on mobile data (they will not — a
> 40-byte command at 30 commands/minute is 1.2 KB/min).

Why this is the right trade for *this* project: the whole platform is one language. Protobuf's main
benefit — a language-neutral schema — buys nothing today, while its costs (a codegen step, a second
type system, lossy mapping of Rust enums-with-payloads) are paid every day by the person adding a
game. Postcard's main weakness — positional encoding, so field order matters — is handled by the
same versioning discipline we need for replays anyway (§6), and by golden vectors (§9.2).

### 3.3 Postcard usage rules

Because Postcard is positional, these are mandatory:

1. **Never reorder, insert-in-the-middle, or remove** a field or enum variant of a wire type
   without a protocol version bump. Append only.
2. **Enum variants are appended at the end**, never renumbered. Reserve nothing; just append.
3. **`Option<T>` is fine** (1 byte tag) and is the supported way to add a field *at the end*.
4. **Never use `#[serde(flatten)]`, untagged enums, or `serde_json::Value`** in wire types —
   Postcard cannot represent them meaningfully.
5. **Fixed-size arrays over `Vec` where the size is known** (board squares, seat arrays).
6. `u64`/`i64` are varint-encoded; prefer small types where the domain is small (`u8` for seats).
7. Every wire type has a golden vector (§9.2), so violations of 1–4 fail CI.

---

## 4. Codec negotiation and the dual-codec design

### 4.1 Negotiation

```text
Client: GET /ws
        Sec-WebSocket-Protocol: tabula.v1.postcard, tabula.v1.json
Server: 101 Switching Protocols
        Sec-WebSocket-Protocol: tabula.v1.postcard
```

- The server prefers `postcard`. It selects `json` only if the client offers *only* json **and** the
  account is staff/dev **or** the server runs with `TABULA_ALLOW_JSON_WS=1` (dev/staging default on,
  production default off).
- Postcard frames are WebSocket **binary**; JSON frames are WebSocket **text**. This makes the codec
  self-evident in devtools and prevents accidental mixing.
- The codec applies to **both** the envelope and the game payload — the payload's codec is the
  connection's codec, so a JSON debug session shows fully readable game commands. This is the
  single most valuable debugging property in the system, and it is the reason `Codec` is threaded
  through `ErasedMatch` (doc 02 §8).

### 4.2 Cost of the dual codec

Each game's `Command`/`Event`/`View` must be `Serialize + DeserializeOwned` under both codecs —
which they already are, being plain `serde` types. The registry's erased adapter has a two-arm match
on `Codec`. That is the entire cost: roughly twenty lines in one crate.

### 4.3 What is *not* dual

The **canonical encoding** used for the event log, snapshots, replays, and state hashes is
**always Postcard**, regardless of the connection codec (§7). JSON is a transport convenience, never
a storage format.

---

## 5. Protocol versioning

### 5.1 Version semantics

```text
major  incompatible envelope change (field removed/reordered, semantics changed)
minor  additive change (new message variant appended, new optional field at the end)
```

- Client and server exchange versions in `Hello`/`HelloAck`.
- **Same major, server minor ≥ client minor** → proceed. The client simply does not send or expect
  the newer messages.
- **Same major, client minor > server minor** → proceed, but the server sends
  `Notice{level: Warn, key: "protocol.client_newer"}` and the client must degrade gracefully
  (this happens during rollouts when the client CDN updates before all servers).
- **Different major** → `Close(4400)` with the server's supported range; the client shows an
  update prompt.

### 5.2 Support window

The server supports the **current major and the immediately previous major** for 90 days after a
major bump. Native mobile clients cannot be force-updated instantly, so a major bump is a
significant event that requires: a written migration note, a dual-support implementation, and a
telemetry-driven decision on when to drop the old major (when < 0.5% of sessions use it).

### 5.3 Feature flags instead of minor bumps where possible

`HelloAck.features: FeatureFlags` advertises optional capabilities (delayed spectators, voice,
async matches, board reader). A client checks flags rather than inferring from version numbers. This
keeps minor bumps rare and rollouts safe.

---

## 6. Game payload versioning

Two independent version axes, and it is important not to conflate them:

```text
protocol_version   the ENVELOPE (platform-owned)
rules_version      the game's State/Command/Event encoding and behavior (game-owned)
```

### 6.1 Rules for game payload evolution

| Change | Requires | Effect on live matches | Effect on replays |
|---|---|---|---|
| Append a `Command` variant | `rules_version` bump | Old clients simply never send it | Old replays unaffected |
| Append a field to a `Command` (as `Option`) | `rules_version` bump | Old clients omit it; `apply` must handle `None` | Unaffected |
| Change the meaning of an existing field | `rules_version` bump **and** a `migrate` implementation | Live matches continue on the old version (doc 02 §9.2) | Old replays need `migrate` or are marked unreplayable |
| Remove a variant/field | `rules_version` bump + migration | Old version stays linked until matches drain | `migrate` or unreplayable |
| Change `apply` behavior (bug fix that changes outcomes) | `rules_version` bump | Old version stays linked | **Old replays must run on the old rules** — this is why `rules_hash` is stored |
| Fix a presentation-only bug | `version` bump only | None | None |

### 6.2 `rules_hash` as the safety net

`rules_hash = blake3(RULES_VERSION tag ‖ hash of the rules-half source files)`, computed by
`xtask` at build time and stored on every match. If someone changes `apply` without bumping
`rules_version`:

- New matches record a different `rules_hash` for the same `rules_version`.
- The nightly replay job detects that stored replays with the old hash no longer reproduce, and
  fails loudly with the exact commit range.

Without this, a behavior change without a version bump would silently corrupt replay history — the
most insidious failure mode in a deterministic system.

---

## 7. Canonical encoding and state hashes

### 7.1 Canonical encoding

```text
canonical(x) = ENCODING_VERSION.to_le_bytes() ‖ postcard(x)

   ENCODING_VERSION: u16 = 1, little-endian (one endianness in the whole kernel)
   - the type's derived Serialize (no custom human-friendly impls on canonical types)
   - all maps as BTreeMap (sorted keys) — HashMap is banned in these types (I-2)
   - no floats in canonical types (doc 00 §5.1)
```

The prefix is **checked on read**, not skipped: a blob written under a different
`ENCODING_VERSION` fails loudly rather than deserializing into a plausible wrong
state. That is the difference between an honest "unreplayable" (§10.2) and a fake
replay.

Postcard has no key-ordering or float-formatting freedom of its own, but it does
serialize a map in *iteration order* — which is why the `HashMap` ban above is
load-bearing and is enforced by `clippy.toml` rather than by the encoder.

Used for: `match_inputs.payload`, `match_events.payload`, `match_snapshots.payload`, replay files,
and everything hashed.

### 7.2 State hash

```rust
StateHash = blake3( b"tabula.state.v1" ‖ rules_version_le ‖ canonical(state) )
```

- Stored on every snapshot and every *N*-th event row (default N = 20).
- Cheap: blake3 over a few KB is microseconds.
- The **only** mechanism that detects determinism drift in production.

### 7.3 Divergence handling

When a replay's recomputed hash differs from the stored hash:

```text
1. The replay job records (match_id, input_index, expected, actual, rules_hash, build).
2. The affected rules_version is flagged: no NEW matches may be created on it (feature flag),
   existing ones continue.
3. Sev-2 alert with the minimal reproducing input prefix, auto-committed to
   tests/replays/<game>/divergence/.
4. The fix is either a rules_version bump (behavior legitimately changed) or a real determinism
   bug (HashMap, float, unordered iteration) — both are found quickly because the failing input
   index is known.
```

---

## 8. Replay format

### 8.1 The model

A replay is **the input stream plus enough metadata to re-run it**. Events are *not* required
(they are derivable) but a checkpoint hash list is included for verification.

For the Phase 1 canonical artifact, the input stream is the ordered set of inputs
that the live runtime accepted into its canonical log. Rejected hostile inputs are
not replay frames: a rejection while replaying a stored frame is therefore a
divergence/corruption error, never a successful no-op. The runtime may choose a
different audit log policy later, but the live and replay input-index assignment
must remain identical (ADR-026 §5).

```text
.tbr file layout (Tabula Binary Replay)

┌ header (postcard, versioned) ──────────────────────────────────────────┐
│ magic: b"TBR1"                                                         │
│ format_version: u16                                                    │
│ match_id: MatchId                                                      │
│ game_id: GameId, game_version: GameVersion                             │
│ rules_version: RulesVersion, rules_hash: [u8;32]                       │
│ config: canonical bytes                                                │
│ roster: SeatRoster (occupants pseudonymized unless owner-authorized)    │
│ seed: Option<MatchSeed>            ← present ONLY in canonical replays  │
│ initial_snapshot: Option<bytes>    ← for partial/projected replays      │
│ started_at: unix_ms, duration_ms: u64                                  │
│ outcome: Option<MatchOutcome>                                          │
│ kind: Canonical | Projected(Viewer)                                    │
└────────────────────────────────────────────────────────────────────────┘
┌ input frames (repeated, length-prefixed) ─────────────────────────────┐
│ input_index: varint                                                    │
│ logical_ms: varint                                                     │
│ input: canonical(Input<Command>)                                       │
│ checkpoint: Option<[u8;32]>        ← every N inputs                     │
└────────────────────────────────────────────────────────────────────────┘
┌ trailer ──────────────────────────────────────────────────────────────┐
│ input_count: u64, final_state_hash: [u8;32], crc32 of the body         │
└────────────────────────────────────────────────────────────────────────┘

Whole file is zstd-framed. Typical chess game: ~3 KB. Typical werewolf game: ~15 KB.
```

### 8.2 Two kinds of replay, and why

| Kind | Contains | Who may have it | Use |
|---|---|---|---|
| **Canonical** | Seed + full inputs; re-running reproduces canonical state, including all secrets | Server-side only; audit/support tooling with `Viewer::Audit` | Anti-cheat audit, bug reproduction, determinism verification |
| **Projected** | A viewer's `View` at the start + that viewer's `ViewEvent` stream; **no seed** | Downloadable by users | "Watch my match", sharing, spectator VOD |

This split is a security requirement. Handing a player the canonical replay of a poker or werewolf
match would reveal every hidden hand and role after the fact — which is often exactly as damaging as
revealing it live (opponents' tendencies, role-assignment patterns).

For fully-public-information games (chess), the projected replay *is* the canonical one minus the
seed, so nothing is lost.

### 8.3 Replay playback

```rust
// crates/tabula-testkit/src/replay.rs  (also used by ops tooling)
// Phase 1 keeps the replay boundary typed. It never links the registry or erases
// the selected game's rules implementation.
pub struct ReplayRunner<R: GameRules> { /* ... */ }

impl<R: GameRules> ReplayRunner<R> {
    pub fn open(path: &Path, identity: ReplayIdentity) -> Result<Self, ReplayError>;
    pub fn from_bytes(bytes: &[u8], identity: ReplayIdentity) -> Result<Self, ReplayError>;
    /// Verifies rules_hash availability; returns Unreplayable if no authoritative identity exists.
    pub fn check(&self) -> ReplayVerdict;
    pub fn step(&mut self) -> Result<Option<StepResult>, ReplayError>;
    /// Replays through `to` and returns stored checkpoint evidence, or an explicit
    /// reconstructed position when no checkpoint was encountered.
    pub fn seek(&mut self, to: StateVersion) -> Result<PrefixPosition, ReplayError>;
    /// Re-runs everything, comparing checkpoints, final state hash, and terminal outcome.
    pub fn verify(&mut self) -> Result<VerifyReport, ReplayError>;
}
pub enum PrefixPosition {
    Verified(PositionEvidence),
    Reconstructed(PositionEvidence),
}
pub enum ReplayVerdict {
    Exact,                                  // rules_hash matches a linked build
    CompatibleVersion,                      // rules_version matches, hash differs → verify only
    NeedsMigration { from: RulesVersion },
    Unreplayable { reason: String },
}
```

Phase 4 may use the registry at the tooling or runtime edge to select and erase the concrete
`GameRules` implementation. That selection does not change the Phase 1 evidence contract: a
position is only `Verified` when the traversed stored checkpoints agree, and a complete
verification also checks the final state hash and the terminal outcome.

Client-side playback (the replay viewer, Phase 9) uses **projected** replays and drives the normal
presenter with the recorded `ViewEvent` stream and `logical_ms` timings — so replay looks exactly
like live play, including animations, at 0.5×/1×/2×/4× speed with scrubbing via the initial view
plus a re-fold from the nearest checkpoint.

### 8.4 Retention

| Artifact | Retention | Storage |
|---|---|---|
| `match_inputs` | 18 months (ranked), 6 months (casual) | Postgres, partitioned |
| `match_events` | 90 days | Postgres, partitioned; dropped by partition |
| Snapshots | latest 3 per match + final, until match retention ends | Postgres / object storage |
| Projected replay files | Same as `match_inputs`; generated lazily on first request and cached | Object storage |
| Canonical replay files | Generated on demand for audit; never stored long-term | Ephemeral |

---

## 9. Protocol security and testing

### 9.1 Hostile-input posture

Every decoder is a trust boundary:

- `cargo-fuzz` targets for: envelope decode (both codecs), each game's `Command` decode, snapshot
  restore, and replay header parse.
- Hard limits before decode: frame size (64 KiB), payload size (16 KiB), `events` vec length,
  string lengths, and collection lengths (Postcard's varint lengths are validated against
  `#[serde(deserialize_with)]` bounded helpers for every `Vec`/`String` in a wire type).
- Decode failures are `Reject{MALFORMED}` and counted per session; a threshold trips a temporary
  ban (doc 03 §21).
- **No `deny_unknown_fields` in JSON debug mode** (it would break minor-version tolerance), but
  **strict length caps in both**.

### 9.2 Golden wire vectors (I-13)

`crates/tabula-protocol/tests/vectors/` holds, per protocol version, a file per wire type with:
a constructed value, its Postcard bytes (hex), and its JSON. CI decodes and re-encodes each,
asserting byte equality.

```text
Changing a wire type ⇒ vectors change ⇒ CI fails ⇒ the author must either
   (a) revert, or
   (b) run `xtask gen-protocol-vectors --bump minor|major`, which also updates
       PROTOCOL_VERSION and writes a line into docs/architecture/protocol-changelog.md.
```

This makes an accidental breaking change nearly impossible and a deliberate one documented.

### 9.3 Projection/security tests

Beyond the per-game `SecretModel` scan (doc 02 §7.3), the protocol layer adds:

| Test | Assertion |
|---|---|
| `no_state_type_on_the_wire` | No type reachable from `ServerMessage` is a game `State`; enforced by a trait-based marker (`NeverOnWire`) that `State` types implement and wire types must not contain |
| `spectator_frames_match_spectator_projection` | For recorded self-play matches, every frame delivered to a spectator session equals `project(state, Spectator)` at that version |
| `seat_frames_never_contain_other_seats_secrets` | Same, per seat, against the `SecretModel` |
| `resync_equals_fold` | `Resync` view at version *v* equals the fold of all `ViewEvent`s from the last `MatchUpdate` (for games opting into folding) |
| `join_token_scope` | A join token for match A cannot attach to match B, or to a seat it does not own |
| `audit_viewer_unreachable` | No code path from a client session can construct `Viewer::Audit` (compile-time: the constructor is `pub(crate)` in `tabula-core` with an `AuditGrant` capability token) |

---

## 10. Compatibility and migration policy

### 10.1 Client/server compatibility matrix

| Client | Server | Behavior |
|---|---|---|
| protocol 1.0 | 1.0 | Full |
| protocol 1.0 | 1.3 | Full; client unaware of new optional messages |
| protocol 1.3 | 1.0 | Works; server warns `protocol.client_newer`; client hides newer features via `features` flags |
| protocol 1.x | 2.x (within support window) | Server runs a 1.x compatibility shim; features gated |
| protocol 1.x | 2.x (after window) | `Close(4400)` + update prompt |
| game version older than server's | — | Server accepts commands for the match's recorded version; the client's *presentation* may lack new art (falls back to placeholder) |
| game version newer than server's | — | Rejected at match creation with `key: "game.version_unavailable"` |

### 10.2 Replay compatibility matrix (I-16)

| Situation | Verdict | Action |
|---|---|---|
| `rules_hash` matches a linked build | `Exact` | Replay normally |
| `rules_version` linked, hash differs | `CompatibleVersion` | Replay with verification; a checkpoint mismatch is a divergence report (§7.3) |
| `rules_version` not linked, `migrate` available | `NeedsMigration` | Migrate the initial snapshot, replay from there; mark the replay "migrated" in the UI |
| `rules_version` not linked, no `migrate` | `Unreplayable` | Show the stored outcome + event summary (from `match_events` if retained), never a fake replay |
| Format version newer than the reader | `Unreplayable` | Prompt to update the client |

**Policy: we never fake a replay.** A match that cannot be reproduced is shown as a result summary
with an explicit note. Silently showing an approximate replay would poison the one thing replays are
for.

### 10.3 Deprecation process

```text
1. Announce in protocol-changelog.md with the target removal version and date.
2. Add telemetry counting use of the deprecated thing, labeled by client build.
3. Ship the replacement; clients migrate.
4. Remove when usage < 0.5% of sessions for 14 consecutive days AND the support window has passed.
5. Bump major only if removal is not representable as additive.
```

---

## 11. Debug tooling

Developer ergonomics for a binary protocol are not optional; without tooling, a binary protocol
costs more than it saves.

| Tool | What it does |
|---|---|
| **JSON codec** (§4) | The primary tool: connect with `tabula.v1.json` and read everything in browser devtools or `websocat`. Available in dev/staging by default. |
| `xtask ws` | A CLI client: authenticates, attaches to a match, pretty-prints frames with color, sends commands from a JSON file or interactively. Speaks both codecs. |
| `xtask decode <hex\|file>` | Decodes any captured Postcard frame given its type name; used for reading production logs and crash dumps. |
| `xtask trace <match_id>` | Reconstructs a match's full timeline from the log: inputs, events, effects, timings, and per-viewer frames. The primary support tool. |
| `xtask replay <file> [--verify] [--at N]` | Replays locally, prints divergence with the exact input index. |
| Browser devtools | Text frames for JSON mode; binary frames show length and hex (paste into `xtask decode`). |
| `websocat` recipe | Documented in `docs/dev/protocol-debugging.md`, including how to mint a dev session token. |
| OpenTelemetry | Every command is a span carrying `corr`, `match_id`, `game_id`, `seat`, `state_version`, apply duration, persist duration, fan-out size (doc 06 §9). |
| Protocol inspector page | An admin-only Leptos page that attaches as `Spectator`, shows a live frame log with filtering, and can export a `.tbr`. Phase 9. |
| **No Postman** | Postman has no useful WebSocket-binary story for us; `xtask ws` is strictly better and lives in the repo. HTTP endpoints get a checked-in `.http`/`hurl` file collection instead. |

---

**Next:** [`06-scaling-deployment-and-observability.md`](./06-scaling-deployment-and-observability.md)
