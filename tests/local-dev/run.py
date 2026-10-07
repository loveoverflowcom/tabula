#!/usr/bin/env python3
"""Actual service binaries, Kanidm, PostgreSQL and two independent HTTPS browsers.

Run inside tests/kanidm/run.sh. All private config/browser state is temporary;
the public receipt contains closed case labels and source provenance only.
"""
import argparse
import asyncio
import io
import json
import hashlib
import os
from pathlib import Path
import secrets
import signal
import ssl
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/accounts"))
import real_browser_acceptance as account
sys.path.insert(0, str(ROOT / "tests/online-match"))
from browser_acceptance import board_square
from playwright.async_api import async_playwright, Error as PlaywrightError, TimeoutError as PlaywrightTimeoutError
from PIL import Image

ORIGIN = "https://app.localhost:8444"
GAME = "/games/com.tabula.chess"
MOVES = (("f2", "f3"), ("e7", "e5"), ("g2", "g4"), ("d8", "h4"))
TERMINAL = "Game over / Black wins / checkmate"
STAGES = frozenset((
    "source_provenance", "disposable_scope", "provider_configuration", "service_binary_inputs",
    "service_configuration", "service_schema_setup", "auth_readiness", "gameplay_readiness",
    "tls_certificate_setup", "tls_frontend_readiness", "browser_trust_setup", "browser_engine_start",
    "first_browser_launch", "second_browser_launch", "first_provider_enrollment", "second_provider_enrollment",
    "independent_accounts", "match_create", "match_join", "initial_board_attachment", "initial_board_rendering",
    "first_pointer_move", "first_gameplay_drain", "first_gameplay_restart", "first_restart_attachment",
    "remaining_pointer_moves", "terminal_board_rendering", "second_gameplay_drain", "second_gameplay_restart",
    "terminal_restart_attachment", "terminal_restart_rendering", "browser_close", "final_gameplay_drain",
    "auth_drain", "complete",
))

class Progress:
    def __init__(self):
        self.cases, self.stage = [], "source_provenance"
    def enter(self, stage):
        if stage not in STAGES:
            raise ValueError("unsupported public progress stage")
        self.stage = stage
        print("STAGE: " + stage, flush=True)

def exception_kind(error):
    # Never expose exception text, arguments, URLs, paths or browser diagnostics.
    if isinstance(error, urllib.error.URLError) and isinstance(error.reason, ssl.SSLCertVerificationError):
        return "tls_certificate_error"
    for kind, label in ((account.CheckFailure, "check_failure"),
                        (PlaywrightTimeoutError, "browser_timeout"),
                        (PlaywrightError, "browser_error"),
                        (subprocess.TimeoutExpired, "process_timeout"),
                        (subprocess.CalledProcessError, "process_failed"),
                        (ssl.SSLCertVerificationError, "tls_certificate_error"),
                        (TimeoutError, "timeout"), (urllib.error.URLError, "network_error"), (OSError, "os_error"),
                        (KeyError, "missing_required_field"), (ValueError, "configuration_invalid")):
        if isinstance(error, kind):
            return label
    return "unexpected_error"

def require(value, label):
    if not value:
        raise account.CheckFailure(label)

def config_file(path, values):
    with os.fdopen(os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "w") as file:
        for key, value in values.items():
            file.write(key + " = " + json.dumps(value) + "\n")

def service_environment():
    # Test-generated configuration is authoritative: operator overrides must not
    # redirect fixture migrations or session issuance to another database.
    return {name: value for name, value in os.environ.items()
            if not name.startswith(("TABULA_AUTH_", "TABULA_SERVER_"))}

class Processes:
    def __init__(self, private):
        self.private, self.children, self.logs = private, [], []
    def start(self, name, command):
        log = (self.private / (name + ".log")).open("wb")
        child = subprocess.Popen(command, cwd=ROOT, env=service_environment(), stdout=log, stderr=log)
        self.children.append(child); self.logs.append(log)
        return child
    def ready(self, child, port):
        deadline = time.monotonic() + 40
        while time.monotonic() < deadline:
            require(child.poll() is None, "service exited before real readiness")
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/readyz", timeout=1) as reply:
                    if reply.status == 200 and reply.read(16) == b"ready":
                        return
            except OSError:
                pass
            time.sleep(.1)
        raise account.CheckFailure("service did not pass database/schema readiness")
    def drain(self, child):
        child.send_signal(signal.SIGTERM)
        require(child.wait(timeout=20) == 0, "service failed bounded graceful drain")
    def tls_ready(self, child, ca):
        context = ssl.create_default_context(cafile=str(ca))
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                                            urllib.request.HTTPSHandler(context=context))
        # Chromium resolves *.localhost itself; the system resolver may not.
        # The generated leaf also covers this fixed loopback IP, verified below.
        url = ORIGIN.replace("app.localhost", "127.0.0.1", 1) + "/"
        request = urllib.request.Request(url, method="HEAD", headers={"Host": ORIGIN.split("://", 1)[1]})
        deadline = time.monotonic() + 10
        last_error = None
        while time.monotonic() < deadline:
            require(child.poll() is None, "TLS frontend exited before real readiness")
            try:
                with opener.open(request, timeout=min(1, max(.001, deadline - time.monotonic()))) as reply:
                    if (reply.status == 200 and reply.geturl() == url
                            and reply.headers.get_content_type() == "text/html"):
                        return
            except OSError as error:
                last_error = error
            time.sleep(min(.1, max(0, deadline - time.monotonic())))
        if last_error is not None:
            raise last_error
        raise account.CheckFailure("verified TLS frontend did not become ready")
    def close(self):
        for child in reversed(self.children):
            if child.poll() is None:
                child.terminate()
                try: child.wait(timeout=20)
                except subprocess.TimeoutExpired: child.kill(); child.wait(timeout=5)
        for log in self.logs: log.close()

async def board(page, seat, status):
    await page.bring_to_front()
    if status == ("White to move", "Black to move")[seat]:
        status = "Your turn / " + ("White", "Black")[seat]
    await page.wait_for_function("""({seat,status}) => {
      const d=document.documentElement.dataset,c=document.querySelector('#glcanvas');
      return d.onlineAvailability==='available' && d.onlineSeat===String(seat)
        && d.onlineStatus===status && d.onlineConnection==='Connected · server-authoritative'
        && c && !c.hidden && !c.hasAttribute('aria-hidden')
        && getComputedStyle(c).visibility==='visible' && c.getBoundingClientRect().width>0;
    }""", arg={"seat": seat, "status": status}, timeout=60000)

async def play(page, source, target, match_id):
    await page.bring_to_front()
    await page.wait_for_function("document.hasFocus() && document.visibilityState==='visible'", timeout=60000)
    canvas = page.locator("#glcanvas")
    bounds = await canvas.bounding_box()
    require(bounds is not None, "real canvas geometry absent")
    path = f"/api/v1/matches/{match_id}/command"
    async with page.expect_response(lambda r: r.url == ORIGIN + path) as result:
        for square in (source, target):
            x, y = board_square(bounds["width"], bounds["height"] - 56, square, False)
            await canvas.click(position={"x": x, "y": y}, delay=70)
    response = await result.value
    require(response.status == 200, "real pointer command not accepted")
    raw = response.request.post_data
    require(raw is not None, "actual command identity absent")
    # UI has to consume the real response; status-only evidence cannot establish it.
    await page.wait_for_function("document.documentElement.dataset.onlineConnection==='Connected · server-authoritative'", timeout=30000)
    return raw

async def reload_boards(pages, match_id, status):
    # A fresh document cannot satisfy this check with the previous owner's DOM
    # dataset. Each browser must perform a new Attach and consume its projection.
    attachment_path = ORIGIN + f"/api/v1/matches/{match_id}/attach"
    for seat, page in enumerate(pages):
        async with page.expect_response(lambda response: response.url == attachment_path
                                        and response.request.method == "POST") as attached:
            await page.reload(wait_until="domcontentloaded")
        require((await attached.value).status == 200, "fresh post-restart attachment failed")
        await board(page, seat, status)

async def rendered_board(page, path):
    # Match the existing independent-browser oracle: a visible canvas or a
    # compiled WASM alone does not establish actual rendered game content.
    await page.bring_to_front()
    shot = await page.locator("#glcanvas").screenshot(timeout=30000)
    pixels = Image.open(io.BytesIO(shot)).convert("RGB")
    require(pixels.width >= 600 and pixels.height >= 400,
            "rendered canvas is unexpectedly small")
    require(len(pixels.resize((160, 120)).getcolors(19201) or []) > 32,
            "actual canvas pixels are blank or lack rendered game content")
    path.write_bytes(shot)
    return shot

async def browser(args, private, processes, server, server_command, provider, progress):
    cases = progress.cases
    peer = dict(provider, **provider["social_peer"])
    peer_path = private / "peer.json"
    with os.fdopen(os.open(peer_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "w") as file: json.dump(peer, file)
    progress.enter("browser_trust_setup")
    executable, environment = account.trust_database(private, private / "ca.pem", Path(provider["ca_path"]))
    progress.enter("browser_engine_start")
    async with async_playwright() as playwright:
        contexts = []
        try:
            for index in range(2):
                progress.enter(("first_browser_launch", "second_browser_launch")[index])
                options = dict(user_data_dir=str(private / f"browser-{index}"), headless=True,
                               channel="chromium",
                               viewport={"width": 1280, "height": 960}, env=environment,
                               chromium_sandbox=True, args=["--disable-dev-shm-usage"], reduced_motion="reduce")
                if executable: options["executable_path"] = executable
                contexts.append(await playwright.chromium.launch_persistent_context(**options))
            pages = [context.pages[0] for context in contexts]
            for index, (page, context, config) in enumerate(zip(pages, contexts, (Path(os.environ["TABULA_KANIDM_TEST_CONFIG"]), peer_path))):
                progress.enter(("first_provider_enrollment", "second_provider_enrollment")[index])
                observation = account.BrowserObservation(page)
                await observation.observe_documents(context, page)
                await account.enroll(page, context, observation, config, f"service_player_{index}", f"Service player {index}", "en")
            progress.enter("independent_accounts")
            identities = [(await account.api(page, "/api/v1/auth/context"))["value"]["account_id"] for page in pages]
            require(identities[0] != identities[1], "independent provider accounts missing")
            cases.append("two_real_provider_enrollments_and_independent_logins")
            for page in pages: await page.goto(ORIGIN + GAME)
            progress.enter("match_create")
            async with pages[0].expect_response(lambda r: r.url == ORIGIN + "/api/v1/matches") as created:
                await pages[0].get_by_test_id("online-create").click()
            require((await created.value).status == 200, "service match create failed")
            # Read the UI's actual join-code acknowledgment, without replaying Create.
            await pages[0].get_by_test_id("online-code").wait_for(state="visible")
            code = (await pages[0].get_by_test_id("online-code").inner_text()).strip()
            progress.enter("match_join")
            await pages[1].get_by_test_id("online-join-code").fill(code)
            async with pages[1].expect_response(lambda r: r.url == ORIGIN + "/api/v1/matches/join") as joined:
                await pages[1].get_by_test_id("online-join").click()
            require((await joined.value).status == 200, "service match join failed")
            progress.enter("initial_board_attachment")
            for seat, page in enumerate(pages):
                await page.get_by_test_id("online-enter").wait_for(state="visible")
                await page.get_by_test_id("online-enter").click()
                await board(page, seat, "White to move")
            progress.enter("initial_board_rendering")
            initial_white = await rendered_board(pages[0], args.receipt.with_name("white-initial.png"))
            await rendered_board(pages[1], args.receipt.with_name("black-initial.png"))
            match_id = await pages[0].evaluate("new URL(location.href).searchParams.get('match_id')")
            require(isinstance(match_id, str) and len(match_id) == 32, "actual match navigation missing")
            progress.enter("first_pointer_move")
            original = await play(pages[0], *MOVES[0], match_id)
            await board(pages[1], 1, "Black to move")
            progress.enter("first_gameplay_drain")
            processes.drain(server)
            progress.enter("first_gameplay_restart")
            server = processes.start("server-restarted", server_command); processes.ready(server, 3002)
            progress.enter("first_restart_attachment")
            await reload_boards(pages, match_id, "Black to move")
            cases.append("graceful_service_restart_preserves_accepted_move_and_seats")
            cases.append("same_session_fresh_authority_recovery")
            progress.enter("remaining_pointer_moves")
            for index in range(1, 4):
                seat = index % 2
                await play(pages[seat], *MOVES[index], match_id)
                status = TERMINAL if index == 3 else ("Black to move" if seat == 0 else "White to move")
                await board(pages[1-seat], 1-seat, status)
            for seat, page in enumerate(pages): await board(page, seat, TERMINAL)
            progress.enter("terminal_board_rendering")
            terminal_white = await rendered_board(pages[0], args.receipt.with_name("white-terminal.png"))
            await rendered_board(pages[1], args.receipt.with_name("black-terminal.png"))
            require(initial_white != terminal_white,
                    "actual canvas pixels did not change after full game")
            cases.append("two_rendered_boards_complete_durable_checkmate")
            require(json.loads(original)["version"] == 2, "supported HTTP carrier not used")
            progress.enter("second_gameplay_drain")
            processes.drain(server)
            progress.enter("second_gameplay_restart")
            server = processes.start("server-terminal-restarted", server_command); processes.ready(server, 3002)
            progress.enter("terminal_restart_attachment")
            await reload_boards(pages, match_id, TERMINAL)
            progress.enter("terminal_restart_rendering")
            for seat, page in enumerate(pages):
                await rendered_board(page, args.receipt.with_name(("white", "black")[seat] + "-restarted-terminal.png"))
            cases.append("terminal_projection_survives_second_service_restart")
            progress.enter("browser_close")
            for context in contexts: await context.close()
            contexts.clear()
            progress.enter("final_gameplay_drain")
            processes.drain(server)
        finally:
            for context in contexts: await context.close()

def run(args, progress):
    progress.enter("disposable_scope")
    require(os.environ.get("TABULA_LOCAL_DEV_DISPOSABLE") == "1" and os.environ.get("TABULA_KANIDM_DISPOSABLE") == "1", "explicit disposable scope required")
    database = os.environ["TABULA_LOCAL_DEV_BROWSER_DATABASE_URL"]
    require(database.startswith("postgres://tabula_test@127.0.0.1:") and database.endswith("/tabula_local_dev_browser_acceptance"), "dedicated disposable browser database required")
    progress.enter("provider_configuration")
    provider = account.private_config(Path(os.environ["TABULA_KANIDM_TEST_CONFIG"]))
    require("social_peer" in provider, "second real provider account required")
    progress.enter("service_binary_inputs")
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    binaries = {name: target / "debug" / name for name in ("tabula-auth", "tabula-server")}
    require(all(path.is_file() for path in binaries.values()), "compiled actual service binaries required")
    cases = progress.cases
    with tempfile.TemporaryDirectory(prefix="tabula-local-dev-") as directory:
        private = Path(directory); os.chmod(private, 0o700)
        processes = Processes(private)
        try:
            progress.enter("service_configuration")
            key = secrets.token_urlsafe(32)
            shared = dict(mode="local-dev", browser_origin=ORIGIN, database_url=database, csrf_key=key,
                          schema_policy="check", pool_connections=8, lifetime_room_capacity=16, drain_seconds=15)
            auth = dict(shared, bind="127.0.0.1:3001", provider_origin=provider["provider_origin"],
                        client_id=provider["client_id"], client_secret=provider["client_secret"], root_certificate=provider["ca_path"])
            server = dict(shared, bind="127.0.0.1:3002", request_capacity=16, work_capacity=16, live_match_capacity=16)
            config_file(private / "auth.toml", auth); config_file(private / "server.toml", server)
            environment = dict(service_environment(), TABULA_AUTH_SCHEMA_POLICY="apply")
            progress.enter("service_schema_setup")
            setup = subprocess.run([binaries["tabula-auth"], "enrollment-enable", "--config", private / "auth.toml"], env=environment, capture_output=True, timeout=40)
            require(setup.returncode == 0, "explicit service schema/enrollment setup failed")
            progress.enter("auth_readiness")
            auth_process = processes.start("auth", [binaries["tabula-auth"], "serve", "--config", private / "auth.toml"]); processes.ready(auth_process, 3001)
            server_command = [binaries["tabula-server"], "serve", "--config", private / "server.toml"]
            progress.enter("gameplay_readiness")
            server_process = processes.start("server", server_command); processes.ready(server_process, 3002)
            cases.append("actual_service_entrypoints_and_database_schema_readiness")
            progress.enter("tls_certificate_setup")
            commands = [
                ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-sha256", "-days", "1", "-subj", "/CN=Tabula service acceptance CA", "-addext", "basicConstraints=critical,CA:TRUE", "-keyout", private / "ca.key", "-out", private / "ca.pem"],
                ["openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=app.localhost", "-keyout", private / "tls.key", "-out", private / "leaf.csr"],
            ]
            (private / "leaf.ext").write_text("subjectAltName=DNS:app.localhost,DNS:localhost,IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n")
            commands.append(["openssl", "x509", "-req", "-sha256", "-days", "1", "-in", private / "leaf.csr", "-CA", private / "ca.pem", "-CAkey", private / "ca.key", "-CAcreateserial", "-extfile", private / "leaf.ext", "-out", private / "tls.pem"])
            for command in commands: subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            progress.enter("tls_frontend_readiness")
            tls_process = processes.start("tls", [sys.executable, ROOT / "tests/local-dev/tls_frontend.py", "--dist", ROOT / "apps/web/dist", "--cert", private / "tls.pem", "--key", private / "tls.key"])
            processes.tls_ready(tls_process, private / "ca.pem")
            asyncio.run(browser(args, private, processes, server_process, server_command, provider, progress))
            progress.enter("auth_drain")
            processes.drain(auth_process)
        finally:
            processes.close()
    progress.enter("complete")

def digest_file(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()

def build_provenance():
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    artifacts = {name: digest_file(target / "debug" / name) for name in ("tabula-auth", "tabula-server")}
    digest = hashlib.sha256()
    for path in sorted((ROOT / "apps/web/dist").rglob("*")):
        if path.is_file():
            digest.update(str(path.relative_to(ROOT / "apps/web/dist")).encode())
            digest.update(bytes.fromhex(digest_file(path)))
    artifacts["served_shell_and_gameplay_tree"] = digest.hexdigest()
    artifacts["working_diff"] = hashlib.sha256(subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=ROOT)).hexdigest()
    return artifacts

def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument("--receipt", type=Path, required=True); args = parser.parse_args()
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.unlink(missing_ok=True)
    for seat in ("white", "black"):
        for stage in ("initial", "terminal", "restarted-terminal"):
            args.receipt.with_name(seat + "-" + stage + ".png").unlink(missing_ok=True)
    progress = Progress()
    receipt = {"status": "fail", "source_sha": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
               "working_tree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=ROOT, text=True).strip()), "source_tree": subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], cwd=ROOT, text=True).strip(), "cases": progress.cases}
    try:
        receipt["artifacts"] = build_provenance()
        run(args, progress); receipt["status"] = "pass"
    except Exception as error:
        receipt["failure"] = "acceptance prerequisite or browser/process check failed"
        receipt["failure_exception"] = exception_kind(error)
    receipt["progress_stage"] = progress.stage
    args.receipt.parent.mkdir(parents=True, exist_ok=True); args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print("PASS: actual local/dev service/provider/browser acceptance" if receipt["status"] == "pass" else "FAIL: actual local/dev service/provider/browser acceptance")
    return 0 if receipt["status"] == "pass" else 1

if __name__ == "__main__":
    raise SystemExit(main())
