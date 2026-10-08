# Agentic coding with the existing CMP Desktop Preview

The existing `previewApp` hosts the same `shared` Compose UI used by Android/iOS.
The official JetBrains Compose Hot Reload MCP lets a coding agent inspect its
semantic tree, interact, edit Kotlin, reload and assert the resulting UI. This
workflow requires a desktop session and no emulator, ADB or coordinate input.
The [verification ledger](../../docs/verification/cmp-agentic-loop/README.md)
records actual executions and their limits.

The preview's account scenarios and GameHost are explicitly synthetic. The
default discovery catalog is generated public registry data; entering setup
does not launch native gameplay. ADR-0043/0045/0046, Rust ownership and production
authority boundaries remain unchanged.

## Pinned tooling and workspace

`apps/mobile/gradle/libs.versions.toml` pins Compose Multiplatform **1.12.0**,
Kotlin **2.4.20** and Compose Hot Reload **1.2.0**. The Hot Reload plugin is
applied only to `previewApp`. Its Kotlin/JVM tasks are
`:previewApp:hotRun` and `:previewApp:hotMcpServer`; the `Jvm` task suffix used by
multiplatform application modules does not apply here. `hotRun` invokes the
existing `com.loveoverflow.tabula.mobile.preview.DesktopPreviewKt` entrypoint
and keeps the existing `preview.*` properties. Ordinary `:previewApp:run`
remains available without Hot Reload. [JetBrains setup](https://github.com/JetBrains/compose-hot-reload/blob/v1.2.0/README.md)
and [the MCP implementation](https://github.com/JetBrains/compose-hot-reload/blob/v1.2.0/hot-reload-mcp/src/main/kotlin/org/jetbrains/compose/reload/mcp/McpServer.kt)
are the upstream references for this pinned version.

Gradle/Android use the existing JDK 17 toolchain and Android SDK platform 37.
Hot Reload uses its supported JetBrains Runtime, automatically provisioned by
`compose.reload.jbr.autoProvisioningEnabled=true`. This project's
`jvmToolchain(17)` resolves JBR 21, Hot Reload's minimum runtime; CHR 1.2.0's
default without a project toolchain is JBR 25. The existing JVM 17 bytecode
target is compatible. Initial Gradle/JBR dependency resolution needs network
access and writable local caches. A stock JVM running
the application is not evidence that Hot Reload is using a compatible JBR.
[JetBrains 1.2.0 release notes](https://github.com/JetBrains/compose-hot-reload/discussions/568)
record the JBR default and compatibility requirements.

If automatic provisioning is blocked, use a compatible official JBR already
available locally, or verify its official archive/checksum before extracting
into a temporary/cache directory. Set `TABULA_JBR_JAVA` to that runtime's
absolute `bin/java` path and forward the supported override:

```bash
bash tools/mobile-preview.sh run --no-auto -Pcompose.reload.jbr.binary="$TABULA_JBR_JAVA"
```

This is a user-selected local path, not a committed machine path or system JVM
replacement. Preserve the provisioning failure in the evidence and record the
override's actual JBR version. Do not disable TLS validation to bypass a trust
store/repository error. The ledger records whether this run used automatic
provisioning or an explicit verified runtime.

The Gradle wrapper lives in `apps/mobile`; running a task from the Rust root
without selecting that build is incorrect. The repository wrappers derive the
Gradle root from their own location, so no committed machine paths are needed.
From the repository root:

```bash
java -version
bash tools/mobile-preview.sh tasks
bash tools/mobile-preview.sh run --no-auto -Ppreview.width=390 -Ppreview.height=844 -Ppreview.language=en
```

`tasks` verifies configuration only. Leave `run` alive while an MCP client
connects to the same Gradle workspace. The underlying commands are:

```bash
cd apps/mobile
./gradlew --console=plain :previewApp:hotRun --no-auto -Ppreview.width=390 -Ppreview.height=844 -Ppreview.language=en
./gradlew --no-daemon --quiet --console=plain :previewApp:hotMcpServer
```

Run the two long-lived commands in separate processes. The second command is a
stdio server for an MCP client; typing it in a terminal alone is not an
interactive MCP test. Do not wrap its stdout in `tee` or emit banner text there.
The MCP client owns JSON-RPC requests and captures server stderr separately.
`--no-auto` selects explicit reloads so the agent's edit and `reload` assertion
cannot race continuous compilation.

## Coding agent configuration

For Claude Code, start the agent at the repository root and use the checked-in
[`.mcp.json`](../../.mcp.json) entry named `tabula-compose`. It launches the same
wrapper with repository-relative arguments. Follow the client's normal MCP
configuration trust flow, then confirm that its connected server exposes the
official tools. The config does not launch the preview; launch `run` separately.

For Codex, register the stdio server from the repository root:

```bash
codex mcp add tabula-compose -- bash "$PWD/tools/mobile-preview.sh" mcp
codex mcp list
```

`$PWD` resolves this checkout when registering the local configuration; no
machine path is authored in the repository. Re-register after moving the
checkout. Initial Gradle startup can exceed an agent's default MCP timeout.
For a Codex CLI session, allow bounded startup/tool time:

```bash
codex -c 'mcp_servers.tabula-compose.startup_timeout_sec=180' -c 'mcp_servers.tabula-compose.tool_timeout_sec=180'
```

The same values can be saved in the existing `[mcp_servers.tabula-compose]`
table in the user's configuration. Do not replace unrelated configuration.
The installed CLI's `codex mcp add --help` confirms the registration syntax;
[official Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)
describes stdio and timeout settings. Configuration and CLI support are distinct
from an executed agent session. Any client supporting local MCP stdio can use
the wrapper; only tested sessions belong in the execution ledger.

## Semantic interaction contract

Call `tools/list` first and use its input schemas. The pinned official server
provides these capabilities:

| Need | MCP tool and selected arguments |
|---|---|
| Application/compilation/reload state | `status` |
| Current application windows | `list_windows` |
| Semantic nodes, text, tags and selected actions | `get_semantic_tree` (`window_id` optional) |
| Activate a semantic action | `click` / `long_click` (`nodeId`, optional `window_id`) |
| Enter text through Compose semantics | `type_text` (`nodeId`, `text`, optional `window_id`) |
| Scroll a semantic container | `scroll` (`nodeId`, `deltaX`/`deltaY`, optional `window_id`) |
| Scroll an indexed container | `scroll_to_index` (`nodeId`, `index`, optional `window_id`) |
| Recompile and wait for reload | `reload` (`timeout_seconds` optional), `await_reload` |
| Capture window pixels | `take_screenshot` (`window_id`, absolute `save_to` optional) |
| Bounded runtime diagnostics | `get_logs` (`limit` optional), `get_ui_error` |

Find nodes by their current `TestTag` and, where exposed, semantic action, then
pass the returned integer `nodeId`. Refresh the tree after each screen transition/reload: node IDs
are not durable selectors. Trees can contain multiple roots for popups/dialogs;
select the intended window and require exactly one matching interactive node.
The pinned renderer lists click/long-click actions and omits empty editable
text; it does not list text/scroll actions. Select those controls by tag and
exercise the official semantic tool. Missing action metadata does not by itself
mean typing or scrolling is unavailable.

Reuse existing tag conventions. `shell-*` identifies shell navigation/content,
`discovery-*` public catalog controls and `account-*` account presentation.
Opaque catalog IDs suffix repeated game controls. Tags do not contain localized
labels, screen coordinates or generated indices. Put a tag on the action owner
once; adding identical tags to a parent and child makes automation ambiguous.

Useful selectors for the closed loop are:

| Purpose | Existing stable tag |
|---|---|
| Home content | `shell-home` |
| Library navigation | `shell-nav-games` |
| Search field | `discovery-search` |
| Game detail action | `shell-details-<catalog-id>` |
| Detail destination | `shell-detail` |
| Enter setup | `shell-setup-action` |
| Setup destination | `shell-setup` |
| Toolbar Back | `shell-back` |
| Scrollable content | `shell-content-scroll` |

Use click actions exposed by the current node and stable tags for text/scroll
controls, then assert the expected next semantic tree. A click response alone
does not prove navigation; a screenshot alone does not prove semantic
accessibility or runtime authority.

## Reproduce the complete local loop

Stop any preview launched for exploration, keep the demonstration target clean,
then run from the repository root:

```bash
python3 tools/mobile-agent-smoke.py --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/run-01
```

With the verified local JBR override described above:

```bash
python3 tools/mobile-agent-smoke.py --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/run-01 --gradle-arg="-Pcompose.reload.jbr.binary=$TABULA_JBR_JAVA"
```

Use a fresh empty directory on each repetition, for example `run-02`; the script
rejects nonempty evidence directories to preserve the earlier run's provenance.

The script is an MCP **client**, not another server or application. It launches
the existing preview at 320×640 dp with explicit reloads and the official stdio
server, discovers tools, records status
and windows, inspects Home, navigates Library → detail → setup → Back, and uses
semantic text/scroll actions. It temporarily changes the unique English
`ShellCopy.HomeHeading` source value in
[`ShellStrings.kt`](shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/localization/ShellStrings.kt),
triggers Hot Reload, asserts the changed text in the live tree, restores the
original bytes, reloads and asserts the original heading again. It captures
selected trees, screenshots and bounded runtime logs.

The script refuses a dirty demonstration target and restores the original
source in its cleanup path if compilation, interaction or reload fails. Do not
run concurrent editors against that file during the demo. Each assertion must
complete; missing windows/tools, JSON-RPC errors, timeouts and failed reloads
produce failure evidence and a nonzero exit. A failed run is not made green by
task discovery or headless tests. Confirm source restoration with `git diff`.

For agent-driven work beyond the smoke, repeat: inspect current semantics → edit
the shared UI → call `reload` → interact with current nodes → assert the intended
tree → collect evidence. Temporary edits must be restored and verified; intended
product edits should additionally pass the applicable deterministic regressions.

## Regression checks, CI and evidence

From `apps/mobile`:

```bash
./gradlew --console=plain :previewApp:test --tests '*AgenticSemanticSmokeTest'
./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug
```

`AgenticSemanticSmokeTest` exercises the same Home/Library/detail/setup/Back,
typing and scrolling contracts through Desktop Compose UI tests and asserts
stable selectors and production unavailability. The full existing preview
suite remains the regression gate. CI verifies official task wiring, runs the
full deterministic suite, requires nonempty unskipped agent semantic results,
and uploads XML/HTML results, task reports and shared-shell screenshots.
It does not run the interactive MCP edit/reload demonstration or prove native
Android/iOS gameplay. A configured CI job is not a successful CI run or known
branch-protection enforcement.

The [execution ledger](../../docs/verification/cmp-agentic-loop/README.md)
separates task discovery, compilation, deterministic tests, live semantic
interaction, reload assertions and screenshot capture. Record exact command,
commit/toolchain/OS, selected test counts, exit result, semantic evidence and
remaining scope. Keep raw output under the preview's ignored
`apps/mobile/previewApp/build/reports/cmp-agentic-loop` directory. The repository's
source guards exclude build trees; arbitrary ignored directories can still be
scanned for game IDs inside runtime semantic JSON. Review selected semantic
excerpts and capture hashes for the committed Markdown ledger. Complete
snapshots, screenshots and logs remain in build reports or CI artifacts;
repository hygiene excludes non-Markdown files from `docs/verification`. Keep
caches, classes, APKs, downloaded JBRs and private data out of the PR.

Before opening a PR run the repository's `just check` core gate and required
mobile checks, plus iOS checks where the environment supports them. Report the
actual `PASS`, `FAIL`, `BLOCKED` or `NOT_RUN` result for each; compilation cannot
stand in for interaction or device acceptance.

## Diagnose runtime blockers

If MCP initializes but shows no connected application, verify that `hotRun` is
alive in the same checkout and Gradle root. Capture `status`, `get_logs` and
server stderr; another workspace's running preview does not satisfy this run.
If semantic actions or reload time out, inspect compilation/UI errors and
refresh windows/tree before retrying. Keep the original failure in the ledger.

A missing graphical desktop, JBR provisioning failure, unavailable dependency
repository or incompatible toolchain blocks the interactive loop. Record the
exact prerequisite and command; do not substitute an emulator, another UI shell
or successful task lookup. Headless Compose regressions can still run and are
reported separately. This workflow does not change native GameHost blockers or
enable production account/social/gameplay services.
