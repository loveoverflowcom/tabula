# ADR-0026: The deterministic rules kernel — reducer model, state hash, RNG, rejection semantics

- **Status:** accepted
- **Date:** 2026-08-30
- **Supersedes:** none. Refines ADR-002 (pure sync rules), ADR-021 (canonical encoding), ADR-025 (testkit).
- **Invariants touched:** none broken. Adds enforcement for I-2, I-4, I-7, I-8; adds contract rule **R8**.

## Context

Phase 0 (doc 07) requires `tabula-core` and `tabula-game-api` to be real. Four
load-bearing pieces were still `todo!()` or placeholders:

| Site | State before |
|---|---|
| `hash::canonical_encode` | `todo!()` |
| `hash::canonical_hash` | `todo!()` |
| `rng::DetRng` (all 6 methods) | `todo!()` |
| `GameRules::state_hash` | `canonical_hash("state", state)` — **version-blind** |

Doc 09 §4 lists "`DetRng` algorithm, canonical encoding, `state_hash`" as **FROZEN
FOREVER**: changing any of them invalidates every stored replay. They therefore
have to be pinned correctly *now*, with committed stability vectors, rather than
implemented plausibly and corrected later.

Implementing them surfaced three places where the docs specify the same thing
differently, plus two questions the docs never answer. Both classes are resolved
below, because a frozen contract cannot be left ambiguous.

### Conflicts found

1. **State-hash construction, specified three ways.**
   - doc 05 §7.2 (normative): `blake3(b"tabula.state.v1" ‖ rules_version_le ‖ canonical(state))`
   - doc 02 §2 signature: `canonical_hash(tag: &str, value: &T)`
   - doc 02 §3 default body: `canonical_hash(Self::RULES_VERSION.tag(), state)`, whose own
     doc comment one line above says `canonical_hash("state", state)`

2. **`RulesVersion::tag()`** is referenced by doc 02 §3 but cannot exist as written
   — it must return `&'static str` from a runtime value. `ids.rs` carries a
   `TODO(phase 0)` admitting this.

3. **`ENCODING_VERSION` width/endianness/value.** Doc 05 §7.1 says only "a 2-byte
   encoding-version prefix". `hash.rs` already declares `ENCODING_VERSION: u16 = 1`;
   endianness was never stated.

### Questions the docs never answer

4. **`DetRng::stream(&self, domain) -> DetRng` takes `&self`, not `&mut self`.** So a
   substream must be a *pure derivation from the root*, not a draw. But `DetRng`
   as declared holds only a `ChaCha8Rng`, from which the seed cannot be recovered.
   The derivation formula for `stream` is given nowhere. (doc 00 §5.2 describes a
   third shape, `derive(MatchSeed, domain, input_index)`.)

5. **RNG behaviour on a rejected input.** R2 covers state. Nothing covers the RNG.

## Decision

### 1. Reducer model: keep `&mut State` (Model A)

Both models were re-evaluated from scratch rather than inherited.

| | **A — `apply(&mut State, …) -> Result<Outcome, E>`** | **B — `apply(&State, …) -> Result<Transition{state, events}, E>`** |
|---|---|---|
| R2 (rejection cannot corrupt) | Contract + mechanical test | **Free, structural** |
| Cost per command | Zero | **Full state rebuild or clone** |
| Incremental structures (zobrist, union-find) | Supported | Defeated — the incremental value must be rebuilt too |
| Rollback | Runtime holds the previous snapshot | Free |
| Event ordering | Explicit `SmallVec`, ordered | Same |
| Large state (tiles, 20-seat werewolf) | Fine | 10M+ clones across the 100k-match nightly |
| Debugging a partial mutation | Harder | N/A |

The decisive point is that **Model B's R2 guarantee is not actually free.** The
cheap spelling — `fn apply(state: State, …) -> Result<Transition, (State, RuleError)>` —
moves rather than clones, but the game can still have mutated the moved-in `State`
before returning `Err`, so it hands back a corrupted value and buys nothing. The
version that genuinely makes corruption unrepresentable is `&State -> new State`,
and that costs a full rebuild on every command, forever, in the hot path of
selfplay benchmarks (`selfplay chess --matches 10000`).

So Model B trades a real, permanent, per-command cost for an invariant that
Model A gets from a test that runs on every game automatically. We keep Model A —
matching doc 02 §3.3 and ADR-002 — but close the gap that made it risky:

- **R2 detection is not opt-in.** `determinism::assert_transactional_on_error`
  hashes the canonical encoding before and after every rejected input, and
  `conformance!` wires it in for every game. A game author cannot forget it.
- **The failure is named, not just detected.** A violation reports the input index
  and both hashes, so it is a bisect rather than a mystery.

A third model — game returns a `Plan`, platform applies it — was considered and
rejected: it either duplicates legality logic (the exact failure doc 02 §3.2
rejected `validate_command` for) or forces `Event` to become a complete state
diff, which contradicts doc 02's "emit *semantic* events" and would bloat every
replay.

**`is_terminal()` is deliberately not added** to the trait, despite being a common
shape. Terminality is already expressed by `Effect::EndMatch { outcome }`. A
second, independently-computed answer to "is this match over" is a divergence
source: a game could return `is_terminal() == false` after emitting `EndMatch`,
and the platform would have two authorities. Self-play observes the effect.

### 2. State hash

Doc 05 §7.2 wins — it is the formulation doc 09 §4 freezes. Pinned exactly:

```text
StateHash = blake3( b"tabula.state.v1"          // STATE_HASH_DOMAIN, 15 bytes, fixed
                  ‖ rules_version.to_le_bytes() // u32, little-endian, 4 bytes, fixed
                  ‖ canonical(state) )          // ENCODING_VERSION ‖ postcard(state)
```

Both prefixes are fixed-width, so the preimage is unambiguous without length
prefixing.

The stringly-typed `tag: &str` parameter is **removed**. It was the direct cause
of the version-blind default (`canonical_hash("state", state)`): an author *could*
pass a tag that omitted the rules version, and the shipped default did exactly
that. Replaced with a typed parameter that cannot be got wrong:

```rust
pub fn state_hash<T: Serialize>(rules_version: RulesVersion, state: &T) -> StateHash
```

`GameRules::state_hash`'s default is now `tabula_core::state_hash(Self::RULES_VERSION, state)`.
The version is no longer something a game author supplies, so it cannot be omitted.
`RulesVersion::tag()` is dropped — the `TODO` it carried is resolved by threading
the typed version into the hash input, which is the option `ids.rs` itself
identified as correct.

**What is in the hash, decided explicitly:**

| | In? | Why |
|---|---|---|
| `rules_version` | **yes** | Two rules versions must never collide on a structurally identical state — that would make a legitimate behaviour change look like determinism rot |
| `ENCODING_VERSION` | **yes**, inside `canonical()` | An encoding-framework change must not silently produce the old hash |
| authoritative `State` | **yes** | It is the subject |
| derived caches held in `State` | **yes** | A divergent cache *is* a divergence (doc 02 §12.4, tiles) |
| `GameId` | **no** | Different games are different Rust types with different encodings; a collision needs identical bytes *and* identical `rules_version`, and a `StateHash` is only ever compared within one match. Including it would also put a variable-length `String` in the preimage. Game identity is bound by `rules_hash` (doc 05 §6.2), which is where it belongs |
| `Config` | **no** | Config's entire effect is already captured in the state `create` produced. Including it would break `fn state_hash(state: &State)` — the hash could no longer be computed from the state alone |
| presentation / animation / camera | **no** | I-10 keeps it out of `State` entirely, in `GamePresentation::Local` |

**There is no separate `state_schema_version`.** `RulesVersion` already covers both
encoding and behaviour by definition (doc 02 §9.2). Adding a third number that
must be bumped in lockstep with a second number is a number that will drift.

The three version axes, and only three:

| Axis | Type | Scope | Bumped when |
|---|---|---|---|
| `ENCODING_VERSION` | `u16` | platform-wide | the canonical encoding framework changes |
| `RulesVersion` | `u32` | per game | `State`/`Command`/`Event` encoding **or** `apply`/`project` behaviour changes |
| `GameVersion` | semver | per game package | presentation, bots, assets, docs — never affects a live match |

**Across schema migrations:** a hash is only meaningful within one `RulesVersion`.
Comparing across versions is not "a divergence", it is a category error, and the
version in the preimage makes it impossible to do by accident. `GameRules::migrate`
produces a state under the *new* version, whose hash is computed under the new
version; the replay is marked "migrated" (doc 05 §10.2) and its old checkpoints
are not re-checked.

### 3. Canonical serialization

Postcard over the type's **derived** `Serialize`, with a 2-byte prefix:

```text
canonical(x) = ENCODING_VERSION.to_le_bytes() ‖ postcard::to_allocvec(x)
```

Little-endian, matching `rules_version_le` — one endianness in the whole kernel.

Ordinary Serde is sufficient *given* the type discipline, and is chosen over a
bespoke binary protocol because a hand-rolled encoder is a second thing to freeze
forever. Postcard is non-self-describing with fixed varint encoding, so it has no
key-ordering or float-formatting freedom of its own. Its one instability is
inherited: it serializes a map in *iteration order*, so a `HashMap` in canonical
state would encode nondeterministically. That is not fixable in the encoder — it
is fixed upstream by the existing `clippy.toml` `disallowed-types` ban on
`HashMap`/`HashSet` in every rules-tier crate, and it is *caught* by
`determinism_same_inputs`, which builds the state twice from scratch: two
independently-constructed `HashMap`s in one thread get different `RandomState`
seeds, so their iteration orders differ and the hashes diverge.

JSON is never used for hashing or storage, only as a debug transport (doc 05 §4.3).

Replay/persistence evolution: the `ENCODING_VERSION` prefix is *inside* every
canonical blob, so a reader can dispatch on it. Version boundaries live at exactly
two places — the 2-byte prefix (framework) and `RulesVersion` (schema/behaviour).

### 4. RNG: stays in `tabula-core`, algorithm pinned

`DetRng` is already in `tabula-core`, not `tabula-testkit` — no move needed. It is
a production primitive and lives with the rest of the kernel.

`DetRng` gains a `key: [u8; 32]` field so substream derivation can be a **pure
function of the root**, which is what `stream(&self, …)` taking `&self` requires
and what the current single-field struct made impossible:

```text
for_input(seed, index) = from_key( blake3( seed ‖ b"input"  ‖ index.to_le_bytes()  ) )
stream(&self, domain)  = from_key( blake3( self.key ‖ b"stream" ‖ domain.to_le_bytes() ) )
from_key(k)            = DetRng { key: k, inner: ChaCha8Rng::from_seed(k) }
```

Substreams compose to any depth and never consume the parent, so adding a draw in
one subsystem cannot shift another — the property doc 00 §5.2 requires.

`below(n)` uses rejection sampling over the full 2^32 range, computed in `u64` so
the modulus is exact:

```text
if n <= 1 { 0 } else {
    zone = 2^32 - (2^32 mod n)
    loop { x = next_u32(); if x < zone { return x mod n } }
}
```

`shuffle` is Fisher-Yates, `i` from `len-1` down to `1`, swapping with `below(i+1)`.

All four are covered by **committed stability vectors** — literal expected bytes
in `rng.rs` tests. That is what stops a `rand_chacha` bump from silently re-seeding
every match ever played. ChaCha8 is reached only through this wrapper, so its
semantics cannot change without an explicit compatibility decision here.

### 5. Rejected-command semantics — new contract rule R8

```text
A rejected input is a total no-op:
    state          unchanged   (R2, existing)
    state_version  unchanged   (I-7, existing)
    RNG stream     irrelevant  (R8, new)
```

R8 is *free*, and that is the point worth recording. Because
`DetRng::for_input(seed, index)` re-derives a fresh stream from `(seed, index)`,
the number of draws made while applying input *N* cannot affect input *N+1* — it
draws from a stream derived independently. So there is nothing to rewind: a game
that drew from `ctx.rng` and then returned `Err` has consumed nothing that any
later input can observe.

This is the "simple invariant" outcome, obtained structurally rather than by
discipline. It holds only because RNG is derived per-input; it would *not* hold
for a single match-long RNG stream, which is the design this rules out.

Consequence for the runtime (Phase 4, recorded here because this contract
constrains it). The runtime is free to choose **either** scheme:

| Scheme | `InputIndex` assigned | Replay sees rejections |
|---|---|---|
| A — log rejections (audit, abuse counting) | to every input, accepted or not | yes; they reject again identically |
| B — drop rejections | only on acceptance | no; they were no-ops |

Both are sound. The single binding requirement is that **replay assigns the same
index to the same input as the live run did** — because the RNG stream is derived
from that index. Mixing the two (assigning an index optimistically, then dropping
the row) is the one combination that breaks: later inputs would draw from
different streams on replay than they did live.

`tabula-testkit`'s reference runner implements scheme A. Whichever Phase 4 picks,
a rejection must never advance `state_version` (I-7).

## Consequences

**Easy now.** A game gets a correct, version-separated state hash by writing
nothing. Determinism drift is detectable in production from Phase 0. Replay,
persistence, and any future client-side prediction can rely on a frozen,
vector-tested encoding and RNG.

**Hard now.** `canonical_hash(tag, value)` is gone; its single caller was the
`GameRules::state_hash` default. R2 remains a contract rather than a type
guarantee — the mitigation is mechanical detection, not elimination, and that is
an accepted, documented trade rather than an oversight.

**Enforcement changed.** `determinism::assert_transactional_on_error` and
`assert_deterministic` are real and hash-based. `rng.rs` and `hash.rs` carry
committed stability vectors; changing an algorithm now fails a test that names
the frozen contract.

**Docs to amend** (same PR): doc 02 §2 (`canonical_hash` signature), doc 02 §3
(default body, `RulesVersion::tag`), doc 05 §7.1 (state the endianness and the
`ENCODING_VERSION` value), doc 02 §3 contract table (add R8).

## Revisit when

- A game's state is large enough that per-command `state_hash` shows up in a
  profile — the answer is `state_hash` override with an incremental structure
  (doc 02 §12.4), not a change here.
- Postcard's wire format changes across a major version. The `ENCODING_VERSION`
  prefix is the migration lever; pin the dependency until it is used.
- A game genuinely needs a match-long RNG stream rather than per-input streams.
  That breaks R8 and needs a superseding ADR, not a patch.
