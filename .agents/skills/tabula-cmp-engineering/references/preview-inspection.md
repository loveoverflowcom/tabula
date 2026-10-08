# MCP preview inspection

Use for the agent's live inspect/edit/interact/assert loop. Read the maintained
[agentic coding guide](../../../../apps/mobile/AGENTIC-CODING.md) for registration,
pinned tools, schemas, JBR recovery and the existing smoke client. Root
[`.mcp.json`](../../../../.mcp.json) names the official server `tabula-compose`.
Discover the available client's server/tools; configuration does not establish a
connection. If direct tools are unavailable, the existing bounded smoke client
can verify the documented flow; it is not a replacement general-purpose inspector.

## Launch the existing host

From the repository root, leave the preview process alive:

```bash
bash tools/mobile-preview.sh run --no-auto -Ppreview.width=390 -Ppreview.height=844 -Ppreview.language=en
```

The MCP client independently starts `bash tools/mobile-preview.sh mcp` over
stdio. Both use the same checkout's nested Gradle root. These wrap the real
Kotlin/JVM tasks `:previewApp:hotRun` and `:previewApp:hotMcpServer`, without a
`Jvm` suffix. Keep MCP stdout exclusively JSON-RPC; logs belong to stderr or the
client's receipt. Select explicit reloads with `--no-auto` to avoid edit/build
races. Preview size properties are logical dp; inspect actual capture px/density.
Follow the guide for supported local JBR overrides and bounded startup timeouts;
do not replace global toolchains/configuration or disable TLS validation.

## Inspect current nodes, then act and assert

1. Discover `tools/list` and use its current schemas. Inspect `status`,
   `list_windows` and `get_semantic_tree`; select the intended preview window.
2. Find exactly one current `TestTag`/action owner across the relevant roots.
   Use stable purpose tags, including opaque catalog-ID suffixes for repeated
   controls, not translated labels, coordinates or generated indices. Resolve
   the returned runtime integer `nodeId`; reacquire after navigation or reload.
3. Use semantic `click`/`long_click`, `type_text`, `scroll` or `scroll_to_index`
   as supported. In the pinned export, empty editable text and text/scroll action
   metadata can be absent; select those controls by their existing stable tag
   and invoke the official tool. Do not infer unsupported input or guess pixels.
4. Assert destination, text, enabled/selected state or meaningful scroll change
   in a fresh tree. A success receipt alone is not the UI consequence. For a
   state-preserving refactor, exercise the affected focus/scroll/navigation state
   before and after; `reset_ui` discards remembered state and cannot prove retention.
5. Edit the intended shared source, call `reload` with a bounded timeout, inspect
   build/UI errors and assert the new UI from fresh nodes. Keep the same viewport,
   locale, state and fixture for rechecks. `await_reload` belongs to automatic
   build mode; it does not initiate compilation in this explicit loop.
6. Capture with `take_screenshot`, then open the actual returned image/path for
   [visual review](design-and-visual-review.md). Collect bounded `get_logs` and
   `get_ui_error` independently of semantic assertions. Fix findings at their
   owner and repeat; do not promote captured images to inspected evidence.

Useful existing selectors include `shell-home`, `shell-nav-games`,
`discovery-search`, `shell-details-<catalog-id>`, `shell-detail`,
`shell-setup-action`, `shell-setup`, `shell-back` and `shell-content-scroll`.
Verify them in the current tree. Setup's disabled native start is an expected
ADR-0043 boundary, not a UI bug to bypass.

## Verify tooling with the bounded existing client

For edit/reload/restore acceptance, use a fresh empty evidence directory:

```bash
python3 tools/mobile-agent-smoke.py --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/run-04
```

Choose another run name if it already exists. The client owns startup, schema
discovery, Home/Library/detail/setup/Back, typing/scrolling, a unique temporary
English heading edit, both reload assertions and original-byte restoration.
It refuses a dirty `ShellStrings.kt`; do not edit that target concurrently or
discard someone else's patch to make the demo run. Preserve failures and confirm
restoration with `git diff`. Product edits remain intended changes, not temporary
demo edits. Stop only the preview/MCP processes started for this task when done.

Missing desktop/window, dependency/JBR errors, JSON-RPC failures and failed
build/reload/assertions remain FAIL/BLOCKED with diagnostics. Refresh status,
windows and tree before a justified retry; do not mask the failure with task
discovery or headless tests. The historical
[execution ledger](../../../../docs/verification/cmp-agentic-loop/README.md)
records prior runs, not new acceptance from reading this skill. Raw trees/images/
logs belong in the excluded preview `build/reports` tree, not arbitrary ignored
folders still scanned by source guards.
