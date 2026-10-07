# October 5, 2026 PR integration review

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


The owner requested review, tests and sequential merges into `develop`, with
follow-up fixes allowed directly on that branch. PRs #69, #70 and #75 were
normally merged in that order after conflict reconciliation and successful
checks. #70 was retargeted from the Werewolf branch to `develop` after #69 merged.
Each resulting merge tree was byte-identical to its tested head tree.

| PR | Tested head | Merge commit | Local portable gate | Terminal pre-merge CI |
|---|---|---|---|---|
| [#69](https://github.com/loveoverflowcom/tabula/pull/69) | `64e4d86fe802e7b1279a3bb69801519d75a3a79a` | `ccfae4e2ff4069f5555d86d15acbeb5723ae0c28` | PASS: 1,195 tests, 0 failed, 18 existing ignored | 16/16 SUCCESS |
| [#70](https://github.com/loveoverflowcom/tabula/pull/70) | `f994e2e20ef0edcbb4303bf856ab4101df6b1593` | `0df5bd4db4901f3876b6bbb86ce284d1c3a79f62` | PASS: 1,201 tests, 0 failed, 18 existing ignored | 14/14 SUCCESS |
| [#75](https://github.com/loveoverflowcom/tabula/pull/75) | `d696e4b8d1192c2c851ebcd6f43b6847e8f054aa` | `6d8c3026e4c7028d344a8d1c1656de4cc93e0a80` | PASS: 1,201 tests, 0 failed, 18 existing ignored | 17/17 SUCCESS |

The portable command was `cargo xtask check` on Linux with Rust 1.96. It ran
formatting, all-feature Clippy, workspace tests, dependency/game-ID/manifest
checks, generated-token and raw-color checks, and cargo-deny in its defined order.
Ignored tests are not passing evidence. No branch protection or repository
rulesets were configured at review time; check success was verified explicitly
before each merge, which also required the reviewed head SHA.

## Reviewed claims and additional execution

| Claim / invariant | Owner and failure mode | Oracle and executed evidence | Residual scope |
|---|---|---|---|
| Deterministic, transactional Werewolf rules, I-2/I-8 and R2/R8 | Reducer; wrong phase resolution or partial rejection | SOURCE-READ against W-D1–W-D18; PASS: `cargo test -p tabula-game-werewolf --features presentation,testkit` (131 tests, including conformance, privacy and retained replay checks) | Not an exhaustive rules proof or native/WASM byte comparison |
| Authorized views/events and presentation concealment, I-5/I-6/I-10 | Projection and presenter; private action existence or stale revealed card | PASS: the Werewolf security/presentation suite above; `cargo test -p tabula-game-client --no-default-features --features werewolf --test werewolf_simulator` (4); browser host Node tests (90) | Mocked host and headless presentation do not establish real browser/mobile pixels or online privacy |
| Reachable simulation remains deterministic and terminates | Test-only simulation and reducer | PASS: `cargo run -p tabula-game-werewolf --features testkit --example verify_simulation -- 100 N`, for N=6,12,20; 300/300 terminated, 20,715 attempted inputs, zero failures | Base seed `[64;32]`, three-round cap, 1,000-input bound, 5% hostile inputs, projection checks enabled; sampled local evidence only |
| Asset relocation preserves identity | Game-owned sources and pack/staging tooling; missing or altered assets | 51 moved source blobs byte-identical, only three source READMEs changed; PASS: locality check for 3 packs/20 runtime files, Node host tests (90), spike Node tests (17), real Rust-backed staging/HTTP Python tests (11) | CI additionally rebuilt all three packs and compared pinned manifests; no rendering/timing claim |
| Native voice lifecycle and credential boundary | CMP controller, Android/Swift adapters; late callback, unsolicited capture or credential crossing GameHost | SOURCE-READ of grant expiry, attempt/command fencing, receive-only join, cleanup and capture denial; PASS: `./gradlew --console=plain :shared:testAndroidHostTest :android:assembleDebug` (52 tests and APK), `./gradlew --console=plain :previewApp:test` (13 tests), fixture Python tests (2) | Controller/UI tests use doubles. Local APK build did not require a staged game; CI did. Device/SFU audio remains NOT_RUN |

The generated 390-pixel CMP voice screenshot was inspected for text and control
layout. Its simulated game page is explicitly labelled; it is not a native
WebView or live voice screenshot. Skill validation also passed, including 32
checker tests and six documentation-contract tests.

## CI receipts

- #69: [core/mobile/WASM/Werewolf](https://github.com/loveoverflowcom/tabula/actions/runs/37318385982), [Kanidm](https://github.com/loveoverflowcom/tabula/actions/runs/37318386000), [durable match](https://github.com/loveoverflowcom/tabula/actions/runs/37318386011), [durable session](https://github.com/loveoverflowcom/tabula/actions/runs/37318385974).
- #70: [core/mobile/WASM/Werewolf and asset identity](https://github.com/loveoverflowcom/tabula/actions/runs/37318433621), [durable match](https://github.com/loveoverflowcom/tabula/actions/runs/37318433627).
- #75: [core/Android/iOS/WASM/Werewolf](https://github.com/loveoverflowcom/tabula/actions/runs/37318456434), [Kanidm](https://github.com/loveoverflowcom/tabula/actions/runs/37318456365), [durable match](https://github.com/loveoverflowcom/tabula/actions/runs/37318456375), [durable session](https://github.com/loveoverflowcom/tabula/actions/runs/37318456457).

These are pre-merge receipts. The iOS job compiled and linked the simulator app;
it did not run microphone or two-client audio acceptance. Post-merge CI must be
read independently rather than inferred from these links.

## Local execution qualifications

Initial #70/#75 aggregate attempts shared a Cargo target directory with #69 and
executed its stale 108-test xtask binary, failing on the removed host-local cover
path. Rebuilding the focused #70 test selected its current 114-test binary and
passed. All workspace source timestamps were then refreshed and the aggregate
gates rerun sequentially, producing the successful results above without source
changes. Only those fresh aggregate receipts support #70/#75.

The first renderer-spike Python invocation lacked its `embedding_fixture`
executable and was BLOCKED at setup. After `cargo build -p xtask --example
embedding_fixture`, the suite ran with `TABULA60_AUTHORITY_TOOL` pointing to that
binary and passed all 11 tests. Neither failed setup nor stale-binary results
are counted as successful checks.

Raw local logs, initial failures, fresh passes and GitHub check snapshots were
retained in the review machine's ignored `target/pr-integration-20261005/`.
They are local artifacts, not checked-in evidence available to a fresh clone.

## Unmerged and remaining scope

[PR #80](https://github.com/loveoverflowcom/tabula/pull/80), reviewed at
`83f05bf3485bbc52cc7423eb65e136bbc70c5e05`, contained only eight documentation
files. Its authenticated join-code/browser-play implementation was absent and
its acceptance ledger explicitly recorded pending/NOT_RUN gates. It remains
draft; green baseline CI does not prove that feature. Implementing the missing
online slice is separate work, not a merge-conflict fix.

[PR #38](https://github.com/loveoverflowcom/tabula/pull/38) targets `main` from
`develop`, so it was outside this request. Real Werewolf target rendering,
native voice/device/SFU acceptance, online play, production activation and broad
phase exits remain governed by their existing ADRs and ledgers.
