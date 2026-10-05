# ADR-0035: opt-in Werewolf local referee and isolated-seat simulator

- **Status:** accepted for the explicitly requested standalone scope; rollout remains gated
- **Date:** 2026-10-04
- **Implementation baseline:** `develop @ 115159cd38cba11f7fa94c3c49698b7a45d59e5f`
- **Publication compatibility cutoff:** `develop @ 729417ffb6de4b376d2736bdfbf78c46f455630e`
- **Extends:** ADR-0030's separate Macroquad document containment
- **Invariants:** I-1, I-2, I-3, I-4, I-5/I-6, I-9, I-10, I-13, I-15 unchanged

## Context and authorized boundary

The owner requested completing additional Werewolf standalone on the dot cloud computer.
The baseline contains only W1/W2 configuration, assignment and state invariants.
Phase 3 owns a complete headless referee, but the normal Phase 7 social client is gated.
This explicit narrow exception permits a local game-owned presenter for exercising real
ClassicV1 rules through the existing Macroquad renderer and deterministic local host.
It does not close the portfolio, protocol, social or voice phase exits.

## Decision

- Implement the maintained six-role ClassicV1 decisions in `games/werewolf` as pure
  `GameRules`: deterministic assignment, real commands, fixed timers, night/day/vote,
  simultaneous deaths, outcome, authorized projections and event non-existence
- Expose an opt-in local simulator, explicitly labelled as controlling isolated seat views
  on one device. It has no substitution bot and no LLM moderator. The computer is the
  deterministic referee. One selected seat or public outsider projection enters presentation
- Seat switches, conceal, Escape, focus loss, private-phase transitions and restart clear
  private presentation selection. The local simulator grants its operator each explicit
  seat view for testing; it is not an authenticated social game or secure human hot-seat mode
- A simulation Next phase control advances logical time to the current deadline; it does
  not shorten canonical windows or bypass reducer validation. Ordinary elapsed time also
  continues through blur and host dialogs. Timer inputs enter the ordinary canonical stream
- Presentation receives only `View`/`ViewEvent`. Public UI receives no role assignment map,
  secret command count, canonical input index, raw events, replay seed or canonical hash
- The approved six-role design exports are reused as original artwork in a standard hashed
  Werewolf asset pack. Live Vietnamese type and common opaque back are code-owned. Art
  assets are public resources; their availability does not authorize revealing assignments
- Native and WASM reuse the existing renderer and local `LocalSpriteResources` verified
  size/BLAKE3 loading boundary. Browser payload delivery retains same-origin content-hashed
  SHA-256 verification, bounded loading/error/retry, and document teardown. The dashboard
  remains shared M3 and does not eagerly fetch Werewolf portraits
- The standalone document is opt-in. `game.toml` rollout stays disabled; this does not
  silently activate registry launch support or ship to any production host

## Provenance and compatibility

Inputs were resolved by their existing Library identities and inspected locally: source
review ZIP v1, six-role PNG ZIP, deck contact sheet, mobile card states and style comparison.
These contain original generated artwork with layout references, not publisher-imported art.
An unpublished Mac implementation (`b457222d2cd86e08ddfce85ae95e7afaaea0fc33`, evidence
`1eae6abf75fa251ed6ddd660d2ac3e2ff1f090b7`) is offline and was not transferred or claimed
as cloud source. The necessary cloud implementation is recreated from the approved exports
and current contracts. The predecessor review pages are design references, not runtime evidence.

New reducer/state shape advances Werewolf rules identity to version 2. Existing W1/W2
skeleton snapshots are not silently migrated. Authentication/session policy ADR-0031
remains unchanged; local inputs contain no credentials or network grants.

## Remaining gates and evidence

Online multiplayer, real human social play, private chat/socket enforcement, moderation,
voice/SFU, auth, persisted match/replay/resume, native catalog and rollout remain gated.
The [verification ledger](../verification/werewolf-standalone/README.md) distinguishes
semantic, privacy, build, headless, native and browser evidence. A target build or generated
mockup is not real-runtime screenshot evidence. Publication requires separate permission.


## Later compatible platform work

The source is reconciled with the CMP foundation (ADR-0032), first-party WebView host
(ADR-0033), gated Kanidm/account/service skeleton (ADR-0034), and current
`tabula-render-macroquad` / `tabula-render-headless` crate names. Their implemented
code and declared gates are preserved. Werewolf remains outside the mobile bundle
registry/launch set; the ordinary standalone document has no native bridge port.
No auth listener, account runtime or network authority is introduced by this slice.
