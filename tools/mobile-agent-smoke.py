#!/usr/bin/env python3
"""Real stdio MCP client for the existing preview; no replacement MCP server or UI shell."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import queue
import signal
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "apps/mobile/shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/localization/ShellStrings.kt"
ORIGINAL = 'ShellCopy.HomeHeading -> "Your play space"'
MARKER = "Agent loop verified"


class McpClient:
    def __init__(self, evidence, gradle_args):
        self.evidence = evidence
        self.sequence = 0
        self.messages = queue.Queue()
        self.stderr = (evidence / "mcp-stderr.log").open("w")
        self.transcript = (evidence / "mcp-transcript.jsonl").open("w")
        self.process = subprocess.Popen(
            ["bash", str(ROOT / "tools/mobile-preview.sh"), "mcp", *gradle_args],
            cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr,
            text=True, bufsize=1, start_new_session=True,
        )
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        for line in self.process.stdout:
            try:
                self.messages.put(json.loads(line))
            except json.JSONDecodeError:
                self.messages.put({"protocol_error": line.rstrip()})
        self.messages.put({"eof": True})

    def send(self, message):
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def request(self, method, params, timeout=180):
        self.sequence += 1
        request = {"jsonrpc": "2.0", "id": self.sequence, "method": method, "params": params}
        self.transcript.write(json.dumps({"request": request}) + "\n")
        self.transcript.flush()
        self.send(request)
        deadline = time.monotonic() + timeout
        while True:
            try:
                response = self.messages.get(timeout=max(0.01, deadline - time.monotonic()))
            except queue.Empty as error:
                raise TimeoutError(f"MCP {method} timed out; see mcp-stderr.log") from error
            if "protocol_error" in response or "eof" in response:
                raise RuntimeError(f"MCP transport failed: {response}; see mcp-stderr.log")
            if response.get("id") == self.sequence:
                break
            if time.monotonic() >= deadline:
                raise TimeoutError(f"MCP {method} timed out")
        recorded = json.loads(json.dumps(response))
        for item in recorded.get("result", {}).get("content", []):
            if item.get("type") == "image":
                data = base64.b64decode(item.pop("data"), validate=True)
                filename = f"screenshot-{self.sequence}.png"
                (self.evidence / filename).write_bytes(data)
                item["saved_as"] = filename
                item["sha256"] = hashlib.sha256(data).hexdigest()
        self.transcript.write(json.dumps({"request": request, "response": recorded}) + "\n")
        self.transcript.flush()
        if "error" in response:
            raise RuntimeError(response["error"])
        result = response["result"]
        if result.get("isError"):
            raise RuntimeError(result)
        return result

    def tool(self, name, **arguments):
        result = self.request("tools/call", {"name": name, "arguments": arguments})
        texts = [item["text"] for item in result.get("content", []) if item.get("type") == "text"]
        text = "\n".join(texts)
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return text

    def close(self):
        stop(self.process)
        self.transcript.close()
        self.stderr.close()


def stop(process):
    if process and process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=10)


def nodes(tree):
    if isinstance(tree, list):
        for root in tree:
            yield from nodes(root)
    elif isinstance(tree, dict):
        yield tree
        for child in tree.get("children", []):
            yield from nodes(child)


def require(condition, message):
    # Acceptance checks must also run when PYTHONOPTIMIZE is enabled.
    if not condition:
        raise AssertionError(message)


def wait_for(check, description, timeout=30):
    deadline = time.monotonic() + timeout
    while True:
        result = check()
        if result:
            return result
        if time.monotonic() >= deadline:
            raise TimeoutError(description)
        time.sleep(0.5)


class Preview:
    def __init__(self, client, window, evidence):
        self.client, self.window, self.evidence = client, window, evidence

    def tree(self):
        result = self.client.tool("get_semantic_tree", window_id=self.window)
        if isinstance(result, dict) and "error" in result:
            raise RuntimeError(result)
        return result

    def find(self, tag):
        matches = [node for node in nodes(self.tree()) if node.get("testTag") == tag]
        if len(matches) != 1:
            raise AssertionError(f"Expected one {tag}, got {len(matches)}")
        return matches[0]

    def click(self, tag):
        node = self.find(tag)
        if "onClick" not in node.get("actions", []) or node.get("enabled") is False:
            raise AssertionError(f"{tag} has no enabled semantic click")
        return self.client.tool("click", window_id=self.window, nodeId=node["id"])

    def route(self, tag, stage):
        wait_for(lambda: any(n.get("testTag") == tag for n in nodes(self.tree())), f"Missing route {tag}")
        self.find(tag)
        tree = self.tree()
        (self.evidence / f"{stage}-semantics.json").write_text(json.dumps(tree, indent=2) + "\n")
        self.client.tool("take_screenshot", window_id=self.window)
        error = self.client.tool("get_ui_error", window_id=self.window)
        if error.get("hasError"):
            raise AssertionError(error)
        print(f"PASS {stage}: {tag}", flush=True)

    def text(self, expected):
        return any(n.get("text") == expected for n in nodes(self.tree()))

    def reload(self, previous):
        outcome = self.client.tool("reload", timeout_seconds=60)
        status = wait_for(
            lambda: self.healthy_reload(previous), "Hot Reload did not finish successfully", timeout=120,
        )
        if outcome.get("success") is False or status.get("successfulReloads", 0) <= previous:
            raise AssertionError(f"No successful class reload: {outcome}, {status}")

    def healthy_reload(self, previous):
        status = self.client.tool("status")
        if status.get("reloadState") == "failed" or status.get("lastError") or status.get("uiErrorWindows"):
            raise AssertionError(status)
        return status if status.get("connected") and status.get("reloadState") != "reloading" and status.get("successfulReloads", 0) > previous else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-dir", type=Path, default=ROOT / "apps/mobile/previewApp/build/reports/cmp-agentic-loop/run-01")
    parser.add_argument("--connect-only", action="store_true", help="Use a preview already launched in explicit mode")
    parser.add_argument("--gradle-arg", action="append", default=[], help="Extra Gradle property, e.g. --gradle-arg=-Pcompose.reload.jbr.binary=...")
    args = parser.parse_args()
    # Catch ordinary task cancellation so cleanup restores the temporary source bytes.
    def interrupted(signum, frame):
        raise KeyboardInterrupt(f"Interrupted by signal {signum}")
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    evidence = args.evidence_dir.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    if any(evidence.iterdir()):
        raise RuntimeError("Use a new empty evidence directory to retain provenance")
    dirty = subprocess.run(["git", "status", "--porcelain", "--", str(SOURCE)], cwd=ROOT, capture_output=True, text=True, check=True)
    if dirty.stdout:
        raise RuntimeError("Refusing to temporarily edit a dirty source file")
    original = SOURCE.read_bytes()
    source = original.decode()
    if source.count(ORIGINAL) != 1:
        raise RuntimeError("Expected one English HomeHeading source; update this smoke's edit oracle")
    record = {"status": "FAIL", "scope": "Desktop CMP shared shell, synthetic account, no native gameplay",
              "source_sha256": hashlib.sha256(original).hexdigest(),
              "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "invocation": sys.argv, "commands": []}
    client = application = log = None
    changed = False
    try:
        if not args.connect_only:
            command = ["bash", str(ROOT / "tools/mobile-preview.sh"), "run", "--no-auto", "-Ppreview.width=320", "-Ppreview.height=640", "-Ppreview.language=en", "-Ppreview.reducedMotion=true", *args.gradle_arg]
            record["commands"].append(command)
            log = (evidence / "preview.log").open("w")
            application = subprocess.Popen(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        client = McpClient(evidence, args.gradle_arg)
        record["commands"].append(["bash", str(ROOT / "tools/mobile-preview.sh"), "mcp", *args.gradle_arg])
        initialize = client.request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "tabula-mobile-agent-smoke", "version": "1"}})
        record["server"] = initialize.get("serverInfo")
        record["protocol_version"] = initialize.get("protocolVersion")
        client.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        tools = client.request("tools/list", {})
        (evidence / "tools.json").write_text(json.dumps(tools, indent=2) + "\n")
        required = {"status", "list_windows", "get_semantic_tree", "click", "type_text", "scroll", "reload", "take_screenshot", "get_logs", "get_ui_error"}
        require(required <= {tool["name"] for tool in tools["tools"]}, "Official MCP capabilities missing")
        def connected():
            if application and application.poll() is not None:
                raise RuntimeError(f"Preview exited {application.returncode}; see preview.log")
            state = client.tool("status")
            return state if state.get("connected") else None
        status = wait_for(connected, "Preview did not connect to MCP", timeout=300)
        require(status.get("buildContinuous") is False, "Start in explicit mode; do not race automatic reload")
        windows = client.tool("list_windows")
        candidates = [w for w in windows if w["title"].startswith("Tabula shell")]
        require(len(candidates) == 1, f"Expected one existing Tabula preview: {windows}")
        preview = Preview(client, candidates[0]["id"], evidence)
        preview.route("shell-home", "01-home")
        require(preview.text("Your play space"), "Expected English baseline before editing")
        preview.click("shell-nav-games")
        preview.route("shell-games", "02-library")
        details = [n for n in nodes(preview.tree()) if n.get("testTag", "").startswith("shell-details-")]
        require(len(details) > 1, "Registry catalog must exercise scrolling across several games")
        scroll = preview.find("shell-content-scroll")
        # Compose clips offscreen bounds to zero; select the first card we will scroll into view.
        first_tag = details[0]["testTag"]
        before = preview.find(first_tag)["bounds"]
        client.tool("scroll", window_id=preview.window, nodeId=scroll["id"], deltaY=360)
        wait_for(lambda: preview.find(first_tag)["bounds"] != before and preview.find(first_tag)["bounds"]["height"] > 0,
                 "Semantic scroll did not bring the first card into view")
        client.tool("scroll", window_id=preview.window, nodeId=preview.find("shell-content-scroll")["id"], deltaY=-10000)
        search = preview.find("discovery-search")
        client.tool("type_text", window_id=preview.window, nodeId=search["id"], text="Chess")
        wait_for(lambda: preview.find("discovery-search").get("editableText") == "Chess", "Semantic text input failed")
        filtered = [n for n in nodes(preview.tree()) if n.get("testTag", "").startswith("shell-details-")]
        require(len(filtered) == 1, "Chess search must leave one detail action")
        preview.route("shell-games", "02b-library-filtered")
        preview.click(filtered[0]["testTag"])
        preview.route("shell-detail", "03-detail")
        preview.click("shell-setup-action")
        preview.route("shell-setup", "04-setup")
        require(preview.find("shell-start-local").get("enabled") is False, "Registry browsing must retain native unavailable authority")
        preview.click("shell-back")
        preview.route("shell-detail", "05-back-detail")
        preview.click("shell-back")
        preview.route("shell-games", "06-back-library")
        require(preview.find("discovery-search").get("editableText") == "Chess", "Back lost the search state")
        preview.click("shell-nav-home")
        preview.route("shell-home", "07-before-edit")
        changed = True
        SOURCE.write_text(source.replace(ORIGINAL, f'ShellCopy.HomeHeading -> "{MARKER}"'))
        preview.reload(client.tool("status").get("successfulReloads", 0))
        wait_for(lambda: preview.text(MARKER) and not preview.text("Your play space"), "Reloaded marker missing in semantic tree")
        preview.route("shell-home", "08-edited")
        SOURCE.write_bytes(original)
        changed = False
        preview.reload(client.tool("status").get("successfulReloads", 0))
        wait_for(lambda: preview.text("Your play space") and not preview.text(MARKER), "Restored baseline missing in semantic tree")
        preview.route("shell-home", "09-restored")
        (evidence / "runtime.log").write_text(client.tool("get_logs", limit=200) + "\n")
        record["final_status"] = client.tool("status")
        record["status"] = "PASS"
        print("PASS real MCP navigation, source edit, Hot Reload and restored semantics", flush=True)
    except BaseException as error:
        record["error"] = str(error)
        if client:
            try:
                record["failure_status"] = client.tool("status")
                (evidence / "runtime-failure.log").write_text(client.tool("get_logs", limit=200) + "\n")
            except Exception as diagnostics:
                record["diagnostic_error"] = str(diagnostics)
        raise
    finally:
        if changed:
            SOURCE.write_bytes(original)
            record["restored_after_failure"] = True
            if client:
                try:
                    client.tool("reload", timeout_seconds=60)
                except Exception as error:
                    record["restore_reload_error"] = str(error)
        record["source_restored"] = SOURCE.read_bytes() == original
        if client:
            client.close()
        stop(application)
        if log:
            log.close()
        (evidence / "result.json").write_text(json.dumps(record, indent=2) + "\n")


if __name__ == "__main__":
    main()
