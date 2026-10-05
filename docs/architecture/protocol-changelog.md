# Executable protocol changelog

## 0.1 — isolated offline match actor (ADR-0039)

This is the first executable pre-release wire contract, not the future production v1
sketch in doc 05. ADR-0039 authorizes only isolated offline actor acceptance; Phase 4
and production listeners, auth upgrades, negotiation, resume, lobby and client networking
remain closed. No capabilities mirror or `tabula-game-api` dependency is introduced.

- Client envelope: exact version 0.1, nonzero `u64` sequence, opaque `u64` correlation,
  and one bounded game command tagged with match, canonical game identity and package version
- Game identity <=128 UTF-8 bytes, package version <=64, opaque command <=16 KiB
- Server envelope: exact version 0.1, optional correlation, nonzero per-attachment frame,
  and `Ack`, fixed-code `Reject`, or attachment-projected `MatchUpdate`
- Output metadata contains no canonical state version, input index, seed, authority identity
  or game diagnostics. Update revision is an attachment's observable counter. Invisible
  actions must not advance it; invisible events are absent, as required by I-5/I-6
- Projected view <=512 KiB, each redacted event <=16 KiB, event count <=64; aggregate
  complete Postcard envelope <=1 MiB even through ordinary serde. All dynamic fields
  use bounded visitors that reject oversized length hints before collection allocation
- Codec entrypoints cap actual client frames at 64 KiB and server frames at 1 MiB.
  JSON's expansion/whitespace/unknown fields count toward those caps; a valid bounded
  domain value can be too large for JSON. Use the frame entrypoints at transport boundaries;
  generic serde cannot measure arbitrary source-format whitespace or ignored fields
- JSON permits unknown fields. Postcard decode requires complete frame consumption
- `crates/tabula-protocol/tests/vectors/0.1/wire.json` pins actual Postcard hex and exact
  compact JSON for every wire type and every codec/error/server-message variant. Tests
  assert construction, decoding, re-encoding, and codec entrypoint equality (I-13)

The existing `xtask gen-protocol-vectors` / `check-protocol` design is not an implemented
version-bump workflow. Executed crate golden-vector tests are the current evidence;
changes must explicitly update this version, fixtures and changelog together.
