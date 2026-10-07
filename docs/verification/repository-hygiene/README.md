# Generated evidence cleanup — 2026-10-07

Base: `develop` at `80d9fdb96f18cd59fa85c9401b533d66bf04b5d7`, freshly fetched before
editing. This is a normal Git-tree cleanup, not a history purge or runtime change.
The [retention policy](../../repository-hygiene.md) owns future output placement.

## Removed and preserved scope

- Removed 556 tracked generated evidence/design-bundle files: 51,622,047 raw file bytes
  (49.23 MiB). This measures deleted file contents, not repository history/clone size
- Six reusable files moved rather than discarded: Werewolf byte/hash budgets and
  original artwork provenance, plus four Python capture/comparison/summary/layout tools
- Preserved all existing game asset files, source artwork, pack manifests, runtime
  brand/font/native resources, Gradle wrapper JAR and authored/generated token inputs.
  Source helpers and Markdown pointers change only where the relocation requires it
- Historical command examples use generic workstation/SDK placeholders; source identity
  remains recorded by pinned commits, not by private local usernames/paths
- Kept all 93 existing Markdown records under `docs/verification/` and `docs/ui/`,
  including every `docs/ui/screens/*.md`; archives are source-pinned and labelled historical
- Independent source audit found no runtime/test consumer of the deleted design copies,
  receipts, logs, XML or historical fixed-path fixture snapshots. The active Werewolf
  budget oracle and original provenance moved byte-for-byte to the game owner

Independent final review also verified 152 runtime/authoring/native/token/test files
(12,790,773 bytes) unchanged and all 332 pinned archive-link occurrences resolve to
BASE objects. It identified one moved-summary relative-link defect; the fix pins
both generated ledger/protocol links to the historical source and was rechecked
against six restored original receipts. No actionable finding remains.

## Executed focused checks

| Check | Result / scope |
|---|---|
| `python3 tools/check-repository-hygiene.py` | PASS indexed docs boundary; checks force-added files too |
| `python3 -m unittest discover -s tools/tests -v` | PASS 20 tests, including six new hygiene regressions, six brand resource tests, three loading-budget tests and five native-policy tests |
| Canonical `check_skills.py`, `test_check_skills.py`, `test_ai_doc_contracts.py` | PASS structural check, 34 skill tests and six documentation-contract tests |
| `check-game-asset-locality.py` | PASS three game-owned packs / 24 pinned source files |
| `check-primary-theme.py` | PASS 684 scalar mappings, four schemes and existing contrast checks |
| `check-mobile-native-policy.py` | PASS source/config only; zero packaged artifacts inspected |
| `cargo fmt --all -- --check` | PASS using the available Rust 1.96.1 toolchain |
| JS syntax checks and Python compile/CLI probes for changed/moved helpers | PASS; no browser/device execution implied |
| Moved issue59 summary with absent/restored historical receipts | PASS negative probe exits 1; six restored BASE receipts reproduce six rows and use valid source-pinned ledger/protocol links from any output directory |
| Runtime/source/token/native blob comparison against BASE | PASS 129 protected files / 12,454,194 bytes unchanged; existing game runtime/source art, pack metadata, brand/fonts, native images/JAR and token adapters |
| Original budget/provenance relocation | PASS byte-for-byte equality with BASE |
| Current-generator exports compared to unchanged BASE generator | PASS all 20 emitted values equal in the current environment; only budget destination changes |
| Actual Git ignore boundary and local Markdown target audit | PASS local output ignored, runtime/tokens/records eligible for Git; zero new missing local targets |
| `git diff --check` | PASS |

`games/werewolf/assets/generate.py --check` reports stale `villager@1x.png` with both
installed Pillow 12.3.0 and isolated CI-pinned Pillow 11.3.0. The unchanged BASE
generator produces the same mismatch in this environment. This is an existing
reproduction limitation; approved runtime PNGs, source art, hashes and budget oracle
are preserved, and this cleanup does not regenerate them to hide it.

Full `cargo xtask check`, native/mobile packaging, new runtime/browser/device
acceptance and full source builds were NOT_RUN for this documentation/tooling slice.
Focused checks are not a full-core-gate PASS. GitHub CI was neither checked nor waited
for, as requested. No merge, deployment, force-push, history rewrite or changes to
separate public evidence branches were performed.
