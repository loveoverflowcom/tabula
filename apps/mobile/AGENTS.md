# Working on the CMP shell

Read the repository [`AGENTS.md`](../../AGENTS.md), architecture
[doc 00](../../docs/architecture/00-architecture-principles.md), and the
[`tabula-engineering`](../../.agents/skills/tabula-engineering/SKILL.md) workflow
before changing this tree. The Gradle root is `apps/mobile`, separate from the
Rust workspace. Follow [the agentic coding guide](AGENTIC-CODING.md) for the
existing Desktop Preview and official JetBrains Hot Reload MCP.

## Ownership

- Reuse `shared` and `previewApp`; do not add another application shell.
- CMP owns UI/navigation and permitted device services. Rust/Macroquad owns game
  rules, projection, replay, presentation and rendering (I-5/I-6, I-9, I-10).
- Keep ADR-0043/0045/0046's boundaries: production native gameplay, login and
  social adapters are unavailable. Explicit preview doubles exercise shared
  presentation; they grant no production authority or native device evidence.
- Do not edit generated tokens or catalog adapters. Change their Rust-owned
  sources and run the existing generators/freshness checks.

## Inspect, edit, interact and assert

Use `bash tools/mobile-preview.sh run --no-auto` from the repository root to open the
existing preview. The MCP client starts `bash tools/mobile-preview.sh mcp` over
stdio. These wrappers resolve the nested Gradle workspace from their own path.
Do not send ordinary Gradle output to MCP stdout.

Discover `tools/list`, then inspect `status`, `list_windows` and
`get_semantic_tree`. Find a node by its current `TestTag` and, where exposed,
semantic action. Use semantic `click`, `type_text`, `scroll` or `scroll_to_index`; never
guess pixel coordinates. Node IDs are runtime handles: refresh the tree after
navigation, edits or reloads. Assert the destination/text/action in the returned
tree, then collect screenshots and runtime logs as separate evidence. The
pinned upstream tree exposes click actions but omits empty editable text and
text/scroll action metadata; identify these controls by their stable tags.

Reuse `shell-*`, `discovery-*` and `account-*` tags. Tags describe stable purpose
and optional opaque catalog ID, not translated text or layout position. Attach
an interactive tag once, to the node that owns the action. Do not add redundant
semantics, expose private facts, or change production behavior for automation.

Preserve the original bytes before a temporary demonstration edit. Use the
bounded smoke script for edit/reload/restore acceptance; it refuses a dirty
target and restores the source on failure. A successful task lookup, MCP startup
or reload response alone does not prove the requested UI appeared. Assert the
edited text, restore, reload and assert the original text again. Failed builds,
timeouts, missing windows and assertion failures must remain failures/blockers.

## Checks and evidence

Run the applicable existing mobile checks from `apps/mobile`:

```bash
./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug
```

For the focused deterministic semantic regression:

```bash
./gradlew --console=plain :previewApp:test --tests '*AgenticSemanticSmokeTest'
```

Run `just check` at the repository root before a PR. iOS framework/Xcode checks
follow the [mobile README](README.md), with their actual target/environment
results. Run the interactive smoke on a real desktop session:

```bash
python3 tools/mobile-agent-smoke.py --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/run-01
```

Use a fresh empty evidence directory for each repeated smoke run.

Keep raw captures/logs under the preview's ignored `build/reports/cmp-agentic-loop`
directory. Commit a bounded, reviewed Markdown verification ledger with exact
commands, toolchain, nonempty test counts, selected semantic excerpts and capture
hashes. Screenshots and complete snapshots stay in build reports or CI artifacts;
repository hygiene excludes non-Markdown files from `docs/verification`. Exclude
caches, classes, downloaded runtimes, APKs and credentials. Report `PASS`, `FAIL`, `BLOCKED` or
`NOT_RUN` for each scope. CI's deterministic shared-shell tests and local MCP
interaction are distinct; neither establishes Android/iOS native gameplay.
