#!/usr/bin/env python3
"""Actual, independently launched Chromium UI acceptance, never mocked HTTP.

Secrets remain in runtime memory or the enclosing private temporary directory.
Only closed result booleans, public build provenance, action names, and masked
screenshots of positively verified rendered UI states become artifacts. This script
is intended for the explicitly authorized disposable CI route, not as a fallback
around a denied local-browser, networking, or certificate-security boundary.
"""
from __future__ import annotations

import argparse
import http.client
import io
import json
import os
from pathlib import Path
import ssl
import subprocess
import threading
import time
from urllib.parse import urlsplit
from PIL import Image
from playwright.sync_api import TimeoutError as BrowserTimeout, sync_playwright
from capture_evidence import CaptureEvidence
from startup_diagnostics import http_status, navigation_failure, origin_class, process_diagnostics

ORIGIN = "https://localhost:9443"
SESSION_COOKIE = "__Host-tabula_session"
GAME_PATH = "/games/com.tabula.chess"
TERMINAL_STATUS = "Game over / Black wins / checkmate"
FORBIDDEN_FRAME_FIELDS = frozenset(
    {"canonical_state", "canonical_version", "input_index", "logical_ms",
     "state_hash", "rules_hash", "seed", "ledger", "canonical_events"}
)


class AcceptanceFailure(Exception):
    """Closed, secret-free failure label; never embed an HTTP body or credential."""


def require(condition: bool, label: str) -> None:
    if not condition:
        raise AcceptanceFailure(label)


def private_frame_keys(value: object) -> bool:
    if isinstance(value, dict):
        return any(key in FORBIDDEN_FRAME_FIELDS or private_frame_keys(item)
                   for key, item in value.items())
    if isinstance(value, list):
        return any(private_frame_keys(item) for item in value)
    return False


def setup_browser_trust(private: Path, role: str, ca: Path) -> tuple[Path, Path]:
    home = private / role / "home"
    profile = private / role / "profile"
    # Chromium uses an existing legacy path even on releases whose default is
    # .local/share/pki/nssdb. Each process has its own HOME and NSS database.
    nss = home / ".pki" / "nssdb"
    nss.mkdir(parents=True, mode=0o700)
    profile.mkdir(parents=True, mode=0o700)
    subprocess.run(["certutil", "-N", "--empty-password", "-d", f"sql:{nss}"],
                   check=True, capture_output=True)
    subprocess.run(["certutil", "-A", "-d", f"sql:{nss}", "-n",
                    "Tabula disposable browser acceptance CA", "-t", "C,,",
                    "-i", str(ca)], check=True, capture_output=True)
    return home, profile


def api(page, path: str, body: dict | str | None = None, csrf: str | None = None) -> dict:
    require(path.startswith(("/api/", "/__fixture/")), "invalid acceptance request path")
    response = page.evaluate("""async ({path, body, csrf}) => {
        const headers = {};
        if (body !== null) headers['Content-Type'] = 'application/json';
        if (csrf !== null) headers['X-Tabula-CSRF'] = csrf;
        const response = await fetch(path, {
            method: body === null ? 'GET' : 'POST', credentials: 'same-origin',
            cache: 'no-store', headers,
            body: body === null ? undefined : typeof body === 'string' ? body : JSON.stringify(body)
        });
        const text = await response.text();
        return {status: response.status, body: text ? JSON.parse(text) : null};
    }""", {"path": path, "body": body, "csrf": csrf})
    require(not private_frame_keys(response["body"]), "canonical facts crossed browser boundary")
    return response


def denied(response: dict, allowed: set[int], label: str) -> None:
    require(response["status"] in allowed, label)
    body = response["body"]
    require(not isinstance(body, dict) or not body.get("frames"),
            "denied request released gameplay frames")


def wire_probe(ca: Path, path: str, headers: list[tuple[str, str]], body: str) -> dict:
    """Real TLS/header-negative probe; values stay opaque and unlogged.

    Browsers forbid changing Origin/Cookie headers. These security partitions
    therefore use a separate real, explicitly CA-validated HTTPS client; actual
    create/join/rendered gameplay remains driven by genuine browser UI events.
    """
    require(path.startswith(("/api/v1/matches/", "/__fixture/publication/")), "invalid hostile probe path")
    connection = http.client.HTTPSConnection("localhost", 9443, timeout=30,
                                            context=ssl.create_default_context(cafile=str(ca)))
    encoded = body.encode("utf-8")
    try:
        connection.putrequest("POST", path)
        for name, value in headers:
            connection.putheader(name, value)
        connection.putheader("Content-Length", str(len(encoded)))
        connection.endheaders(encoded)
        response = connection.getresponse()
        payload = response.read(2_097_153)
        require(len(payload) <= 2_097_152, "hostile probe response exceeded budget")
        value = json.loads(payload) if payload else None
        require(not private_frame_keys(value), "hostile header probe disclosed canonical facts")
        return {"status": response.status, "body": value}
    finally:
        connection.close()


def hostile_header_probes(ca: Path, match_id: str, cookie: str, csrf: str) -> int:
    path, body = f"/api/v1/matches/{match_id}/grant", '{"version":1}'
    baseline = [("Origin", ORIGIN), ("Cookie", f"{SESSION_COOKIE}={cookie}"),
                ("Content-Type", "application/json"), ("X-Tabula-CSRF", csrf)]
    cases = [
        ([(k, v) for k, v in baseline if k != "Origin"], 403),
        ([(k, "https://foreign.tabula.invalid" if k == "Origin" else v) for k, v in baseline], 403),
        ([(k, "null" if k == "Origin" else v) for k, v in baseline], 403),
        ([(k, v) for k, v in baseline if k != "X-Tabula-CSRF"], 403),
        ([(k, "A" * 43 if k == "X-Tabula-CSRF" else v) for k, v in baseline], 403),
        ([(k, "text/plain" if k == "Content-Type" else v) for k, v in baseline], 415),
        (baseline + [("Cookie", f"{SESSION_COOKIE}={cookie}")], 400),
        (baseline + [("Authorization", f"Bearer {cookie}")], 400),
    ]
    for headers, expected in cases:
        denied(wire_probe(ca, path, headers, body), {expected},
               "hostile HTTP transport partition was not rejected")
    return len(cases)


def publication_control(ca: Path, operation: str, token: str) -> dict:
    require(operation in ("status", "release"), "invalid publication control operation")
    return wire_probe(ca, f"/__fixture/publication/{operation}",
                      [("Origin", ORIGIN), ("Content-Type", "application/json"),
                       ("X-Tabula-Fixture-Control", token)], '{"version":1}')


def start_native_poll(match_id: str, attachment_id: str, cookie: str, csrf: str):
    """CI-only real upstream TCP observer, before TLS-edge body buffering.

    The browser game still uses verified HTTPS. This separate native-server
    boundary oracle counts actual bytes from the approved private Rust listener;
    it retains no response body, headers, credential, or grant in artifacts.
    """
    require(os.environ.get("CI") in ("true", "1")
            and os.environ.get("TABULA_ONLINE_MATCH_DISPOSABLE") == "1",
            "native publication oracle requires disposable CI opt-in")
    done = threading.Event()
    result = {"status": None, "json_content_type": False, "no_store": False,
              "body_bytes": 0, "body_error": False}
    body = json.dumps({"version": 1, "attachment_id": attachment_id})

    def observe():
        connection = http.client.HTTPConnection("127.0.0.1", 3000, timeout=90)
        try:
            connection.request("POST", f"/api/v1/matches/{match_id}/poll", body,
                               {"Origin": ORIGIN, "Content-Type": "application/json",
                                "Cookie": f"{SESSION_COOKIE}={cookie}", "X-Tabula-CSRF": csrf})
            response = connection.getresponse()
            result["status"] = response.status
            result["json_content_type"] = response.getheader("Content-Type", "").split(";")[0] == "application/json"
            result["no_store"] = "no-store" in response.getheader("Cache-Control", "").split(",")
            while True:
                chunk = response.read(65_536)
                if not chunk:
                    break
                result["body_bytes"] += len(chunk)
                require(result["body_bytes"] <= 2_097_152, "native publication body exceeded budget")
        except http.client.IncompleteRead as error:
            result["body_bytes"] += len(error.partial)
            result["body_error"] = True
        except Exception:
            result["body_error"] = True
        finally:
            connection.close()
            done.set()

    thread = threading.Thread(target=observe, name="native-publication-byte-oracle", daemon=True)
    thread.start()
    return thread, done, result


def prove_held_publication(white, black, match_id: str, white_attachment: dict,
                          original_command: str, facts: list[dict], cookie: str, ca: Path) -> dict:
    grant = api(black, f"/api/v1/matches/{match_id}/grant", {"version": 1}, facts[1]["csrf_token"])
    require(grant["status"] == 200 and grant["body"]["ready"], "passive opponent grant setup failed")
    attachment = api(black, f"/api/v1/matches/{match_id}/attach",
                     {"version": 1, "binding_id": grant["body"]["binding_id"]}, facts[1]["csrf_token"])
    require(attachment["status"] == 200, "passive opponent attachment setup failed")
    # This second actor has no browser runtime polling it. Initial snapshots
    # were consumed above, and Black has issued no command or receipt request.
    command = json.loads(original_command)
    command["attachment_id"] = white_attachment["attachment_id"]
    command["command"]["command"]["match_id"] = int(match_id, 16)
    accepted = api(white, f"/api/v1/matches/{match_id}/command", json.dumps(command), facts[0]["csrf_token"])
    require(accepted["status"] == 200 and any("Ack" in frame.get("body", {})
            for frame in accepted["body"]["frames"]), "actual visible queued move was not accepted")
    armed = api(black, "/__fixture/publication/arm",
                {"version": 1, "match_id": match_id,
                 "attachment_id": attachment["body"]["attachment_id"]}, facts[1]["csrf_token"])
    require(armed["status"] == 200, "native held-body control setup failed")
    token = armed["body"]["control_token"]
    thread, done, observed = start_native_poll(match_id, attachment["body"]["attachment_id"],
                                              cookie, facts[1]["csrf_token"])
    released = False
    try:
        deadline = time.monotonic() + 30
        while True:
            status = publication_control(ca, "status", token)
            require(status["status"] == 200, "native capture witness status unavailable")
            phase = status["body"]["phase"]
            require(phase not in ("failed", "expired"), "real poll lacked a nonempty projected capture witness")
            if phase == "held":
                break
            require(not done.is_set() and time.monotonic() < deadline,
                    "native poll did not reach a held nonempty first-frame boundary")
            time.sleep(.1)
        require(not done.is_set(), "native poll body completed before its fixture release")
        # Existing storage publication exclusions expire independently within2s;
        # the actual inner body remains unpolled while the separate logout commits.
        time.sleep(2.5)
        logout = api(black, "/api/v1/auth/logout", {}, facts[1]["csrf_token"])
        require(logout["status"] == 204, "separate real authority logout did not commit while body held")
        require(not done.is_set(), "held native body was released before actual revocation committed")
        release = publication_control(ca, "release", token)
        require(release["status"] == 200, "native publication release failed")
        released = True
        thread.join(timeout=45)
        require(done.is_set(), "released actual native poll did not terminate")
        require(observed["status"] == 200 and observed["json_content_type"] and observed["no_store"],
                "native held request did not carry the real successful guarded poll headers")
        require(observed["body_bytes"] == 0, "revoked held native poll delivered protected body bytes")
        suppressed = publication_control(ca, "status", token)
        require(suppressed["status"] == 200 and suppressed["body"]["phase"] == "suppressed",
                "held fixture did not forward an actual inner publication-guard error")
        return {"actual_native_nonempty_capture_held": True,
                "separate_real_logout_before_release": True,
                "actual_inner_publication_guard_error": True,
                "native_poll_status": observed["status"],
                "native_poll_body_bytes": observed["body_bytes"],
                "native_poll_body_error": observed["body_error"],
                "observer_before_tls_edge_buffering": True}
    finally:
        if not released:
            try:
                publication_control(ca, "release", token)
            except Exception:
                pass


def context_facts(page) -> dict:
    response = api(page, "/api/v1/auth/context")
    require(response["status"] == 200, "real session context unavailable")
    return response["body"]


def rendered_canvas_pixels(page) -> bytes:
    """Check actual authorized pixels in memory without retaining a page dump."""
    canvas = page.locator("#glcanvas")
    require(canvas.is_visible(), "actual game canvas is not visible")
    shot = canvas.screenshot(timeout=30_000)
    image = Image.open(io.BytesIO(shot)).convert("RGB")
    require(image.width >= 600 and image.height >= 400, "rendered canvas is unexpectedly small")
    # A compiled WASM or empty/clear-colored WebGL canvas is never rendering proof.
    require(len(image.resize((160, 120)).getcolors(19_201) or []) > 32,
            "actual canvas pixels are blank or lack rendered game content")
    return shot


NEUTRAL_UNAVAILABLE = """() => {
    const root = document.documentElement, canvas = document.querySelector('#glcanvas');
    const error = document.querySelector('#runtime-error'), detail = document.querySelector('#error-detail');
    const privateKeys = ['onlineSeat', 'onlineRevision', 'onlineStatus', 'onlineConnection'];
    const statusNodes = document.querySelectorAll('[data-testid="online-seat"],[data-testid="online-revision"],[data-testid="online-status"],[data-testid="online-connection"]');
    return root.dataset.onlineAvailability === 'unavailable'
        && privateKeys.every(key => !Object.hasOwn(root.dataset, key))
        && canvas && canvas.width === 0 && canvas.height === 0
        && canvas.getAttribute('aria-hidden') === 'true'
        && getComputedStyle(canvas).visibility === 'hidden'
        && error && error.getBoundingClientRect().width > 0 && error.getBoundingClientRect().height > 0
        && detail && detail.textContent.trim() === 'The online connection is unavailable. Moves are blocked. Return to Tabula to reopen this match.'
        && Array.from(statusNodes).every(node => node.textContent.trim() === '');
}"""


def capture_live_authority_loss(white, third, csrf: str, evidence: CaptureEvidence,
                                secrets: list[str]) -> dict:
    """Actual live-board poll401 concealment after normal current-session logout.

    A separate auxiliary actor keeps the passive held-body recipient untouched.
    This proves one active-document authority-loss boundary, not reconnect,
    interrupted-command handling or browser page-cache lifecycle guarantees.
    """
    white.goto(ORIGIN + GAME_PATH, wait_until="domcontentloaded")
    white.get_by_test_id("online-create").wait_for(state="visible", timeout=30_000)
    with white.expect_response(lambda response: urlsplit(response.url).path == "/api/v1/matches", timeout=30_000) as created_response:
        white.get_by_test_id("online-create").click()
    created = created_response.value.json()
    require(created_response.value.status == 200 and created["seat"] == 0,
            "live concealment actual create setup failed")
    match_id, code = created["match_id"], created["join_code"]
    secrets.append(code)
    third.goto(ORIGIN + GAME_PATH, wait_until="domcontentloaded")
    third.get_by_test_id("online-join-code").fill(code)
    with third.expect_response(lambda response: urlsplit(response.url).path == "/api/v1/matches/join", timeout=30_000) as joined_response:
        third.get_by_test_id("online-join").click()
    joined = joined_response.value.json()
    require(joined_response.value.status == 200 and joined["match_id"] == match_id
            and joined["seat"] == 1 and joined["ready"],
            "live concealment actual opponent join setup failed")
    enter_game(white, match_id, 0)
    white.wait_for_function("() => document.documentElement.dataset.onlineRevision === '0'", timeout=30_000)
    rendered_canvas_pixels(white)
    control = white.context.new_page()
    try:
        control.goto(ORIGIN + GAME_PATH, wait_until="domcontentloaded")
        with white.expect_response(lambda response: urlsplit(response.url).path == f"/api/v1/matches/{match_id}/poll"
                                   and response.status == 401, timeout=60_000) as rejected_poll:
            logout = api(control, "/api/v1/auth/logout", {}, csrf)
            require(logout["status"] == 204, "live board authority logout did not commit")
        rejected = rejected_poll.value
        denied({"status": rejected.status, "body": rejected.json()}, {401},
               "live authority-loss poll was not denied")
        white.wait_for_function(NEUTRAL_UNAVAILABLE, timeout=30_000)
        white.locator("#runtime-error").wait_for(state="visible", timeout=30_000)
        evidence.capture(white, "10-authority-unavailable.png", "Actual live-board authority loss: neutral unavailable UI", "White browser",
                         "A previously rendered live initial board received real poll401 after committed normal logout; canvas hidden and zero-sized, projected status cleared, moves unavailable", secrets=secrets)
        return {"actual_live_board_before_logout": True, "current_session_logout_committed": True,
                "actual_live_poll_status": 401, "neutral_error_visible": True,
                "canvas_hidden_zero_sized_and_status_cleared": True}
    finally:
        control.close()


def board_square(width: float, height: float, name: str, flipped: bool) -> tuple[float, float]:
    """Pointer geometry from maintained Chess BoardLayout, never a board oracle.

    Actual page canvas bounds determine the viewport. Real UI events still pass
    through Rust presentation and real server legality; the durable oracle checks
    the exact commands that landed independently of this coordinate calculation.
    """
    require(len(name) == 2 and name[0] in "abcdefgh" and name[1] in "12345678",
            "invalid scripted square")
    margin = min(width * .035, height * .025, 24)
    gap = min(height * .008, 8)
    title = min(height * .06, 40)
    player = min(max(height * .07, 44 if height >= 450 else 24), 56)
    rail = (width >= 760 and height >= 420) or (width >= 600 and height < 420)
    rail_width = min(width * .28, 360) if rail else 0
    game_width = max(width - margin * 2 - rail_width - (gap * 2 if rail else 0), 0)
    columns = max(int((game_width + 4) // 76), 1)
    controls = ((6 + columns - 1) // columns) * 48 - 4
    coordinate = min(game_width * .03, 12 if height < 420 else 16)
    status = 0 if rail else min(height * .1, 64)
    remaining = max(height - margin * 2 - title - player * 2 - controls
                    - status - gap * 6 - coordinate * 2, 0)
    side = min(max(game_width - coordinate * 2, 0), remaining, 680)
    left = margin + (game_width - side) * .5
    top = margin + title + gap + player + gap + coordinate
    file, rank = ord(name[0]) - ord("a"), int(name[1]) - 1
    column, row = (7 - file, rank) if flipped else (file, 7 - rank)
    return left + (column + .5) * side / 8, top + (row + .5) * side / 8


def game_status_class(status: str | None) -> str:
    return {"White to move": "white_turn", "Black to move": "black_turn",
            "White to move / CHECK": "white_in_check", "Black to move / CHECK": "black_in_check",
            TERMINAL_STATUS: "black_checkmate", None: "not_available"}.get(status, "other_status")


def visible_game_facts(page, role: str) -> dict:
    require(role in ("white", "black"), "unknown game diagnostic role")
    values = page.evaluate("""() => {
        const data = document.documentElement.dataset;
        return {revision:data.onlineRevision ?? null, seat:data.onlineSeat ?? null,
                status:data.onlineStatus ?? null, connection:data.onlineConnection ?? null,
                availability:data.onlineAvailability ?? null};
    }""")
    return {"role": role,
            "revision": int(values["revision"]) if values["revision"] in ("0", "1", "2", "3", "4") else "not_observed_in_expected_range",
            "seat": int(values["seat"]) if values["seat"] in ("0", "1") else "not_available",
            "status_class": game_status_class(values["status"]),
            "connection_class": {"Connected · server-authoritative": "ready", "Sending move…": "sending",
                                 "The server rejected that action. Choose another move": "rejected",
                                 "Moves are blocked": "blocked", None: "not_available"}.get(values["connection"], "other_connection"),
            "availability": values["availability"] if values["availability"] in ("available", "unavailable") else "not_reported"}


def move(page, source: str, target: str, flipped: bool, match_id: str,
         trace: list[dict] | None = None) -> str:
    require(source + target in ("f2f3", "e7e5", "g2g4", "d8h4"), "unexpected scripted acceptance move")
    observed = {"move": source + target, "response_observed": False,
                "http_status": None, "ack_present": False}
    if trace is not None:
        trace.append(observed)
    canvas = page.locator("#glcanvas")
    bounds = canvas.bounding_box()
    require(bounds is not None, "actual canvas has no pointer bounds")
    path = f"/api/v1/matches/{match_id}/command"
    with page.expect_response(lambda r: urlsplit(r.url).path == path, timeout=30_000) as result:
        for square in (source, target):
            x, y = board_square(bounds["width"], bounds["height"] - 56, square, flipped)
            canvas.click(position={"x": x, "y": y}, delay=70)
    response = result.value
    observed.update({"response_observed": True, "http_status": http_status(response.status)})
    require(response.status == 200, "rendered legal move was not accepted by real server")
    body = response.json()
    require(not private_frame_keys(body), "canonical facts leaked in command result")
    observed["ack_present"] = any("Ack" in frame.get("body", {}) for frame in body["frames"])
    require(observed["ack_present"],
            "rendered legal move lacks durable acknowledgement")
    # Keep exact bytes in memory: parsing into JS numbers would round a u128
    # identity and turn the intended exact retry into a different command.
    command = response.request.post_data
    require(command is not None, "actual command request body is absent")
    return command


def enter_game(page, match_id: str, expected_seat: int) -> tuple[dict, str]:
    path = f"/api/v1/matches/{match_id}/attach"
    with page.expect_response(lambda r: urlsplit(r.url).path == path, timeout=60_000) as result:
        page.get_by_test_id("online-enter").click()
    response = result.value
    require(response.status == 200, "actual gameplay attachment failed")
    attachment = response.json()
    require(attachment["seat"] == expected_seat, "browser entered the wrong opponent seat")
    require(not private_frame_keys(attachment), "canonical facts leaked on attach")
    page.wait_for_function("expected => document.documentElement.dataset.onlineSeat === String(expected)",
                           arg=expected_seat, timeout=60_000)
    page.locator("#loader").wait_for(state="hidden", timeout=60_000)
    require(page.locator("#glcanvas").is_visible(), "WASM game failed to render")
    grant_body = response.request.post_data
    require(grant_body is not None, "actual grant-bound attachment request was absent")
    return attachment, grant_body


def wait_revision(page, revision: int) -> None:
    page.wait_for_function("revision => Number(document.documentElement.dataset.onlineRevision) >= revision",
                           arg=revision, timeout=30_000)


def enroll_actual_page(page, role: str, results: dict) -> None:
    require(role in ("white", "black", "third"), "unknown disposable browser role")
    results["stage"] = f"submit {role} actual enrollment form"
    observed = {"role": role, "status": None, "origin_class": "not_observed",
                "form_media_type_expected": False, "redirect_completed": False}
    results["startup"]["enrollment"].append(observed)
    def is_enrollment(response):
        return urlsplit(response.url).path == "/__fixture/enroll" and response.request.method == "POST"
    def response_observed(response):
        if is_enrollment(response):
            observed.update({"status": http_status(response.status),
                             "origin_class": origin_class(response.request.header_value("origin"), ORIGIN),
                             "form_media_type_expected": (response.request.header_value("content-type") or "").split(";")[0].strip().lower() == "application/x-www-form-urlencoded"})
    page.on("response", response_observed)
    try:
        with page.expect_response(is_enrollment, timeout=20_000) as submitted:
            page.get_by_test_id("fixture-enroll").click(timeout=20_000)
        response = submitted.value
        response_observed(response)
    finally:
        page.remove_listener("response", response_observed)
    # A denied real form must fail here, rather than disappear into a generic
    # 60s redirect timeout. Header values and response bodies remain private.
    require(observed["origin_class"] == "exact", "actual enrollment form did not carry exact HTTPS Origin")
    require(observed["form_media_type_expected"], "actual enrollment form media type was unexpected")
    require(response.status == 303, "actual enrollment form was rejected before session issuance redirect")
    results["stage"] = f"complete {role} enrollment redirect"
    page.wait_for_url(ORIGIN + GAME_PATH, timeout=30_000)
    observed["redirect_completed"] = True


def run(args) -> None:
    require(os.environ.get("CI") in ("true", "1")
            and os.environ.get("TABULA_ONLINE_MATCH_DISPOSABLE") == "1",
            "explicit CI-only disposable browser acceptance opt-in is required")
    private, artifacts, ca = Path(args.private), Path(args.artifacts), Path(args.ca)
    require(private.is_dir() and ca.is_file(), "private runtime and approved test CA are required")
    artifacts.mkdir(parents=True, exist_ok=True)
    actions: list[str] = []
    results: dict = {"status": "fail", "stage": "start",
                     "authority_mode": "isolated fixture identities through real durable session authority",
                     "browser_processes": 3, "independent_homes_profiles_cookie_jars": True,
                     "tls_errors_ignored": False, "chromium_sandbox": True,
                     "moves": [], "startup": {"https_get": {"attempts": 0, "status": None,
                                 "fixture_page_ready": False, "last_navigation_failure": "none"},
                                 "enrollment": []}}
    browsers = []
    try:
        with sync_playwright() as playwright:
            results["stage"] = "launch separate Chromium processes with scoped CA trust"
            for role in ("white", "black", "third"):
                home, profile = setup_browser_trust(private, role, ca)
                environment = os.environ.copy()
                environment["HOME"] = str(home)
                environment["XDG_DATA_HOME"] = str(home / ".local" / "share")
                browser = playwright.chromium.launch_persistent_context(
                    str(profile), headless=True, channel="chromium", chromium_sandbox=True,
                    env=environment, viewport={"width": 1100, "height": 850},
                    reduced_motion="reduce", locale="en-US",
                )
                browsers.append(browser)
            white, black, third = [browser.new_page() for browser in browsers]
            browser_pids = []
            for browser in browsers:
                inspector = browser.browser.new_browser_cdp_session()
                try:
                    processes = inspector.send("SystemInfo.getProcessInfo")["processInfo"]
                    pids = [process["id"] for process in processes if process["type"] == "browser"]
                    require(len(pids) == 1, "actual Chromium browser process identity unavailable")
                    browser_pids.append(pids[0])
                finally:
                    inspector.detach()
            require(len(set(browser_pids)) == 3, "acceptance contexts share an actual browser process")
            results["distinct_actual_browser_processes_verified"] = True
            results["chromium_version"] = browsers[0].browser.version
            # Navigation validates TLS in actual Chromium. Before issuance there
            # is no interception, route mock, cookie injection or shared storageState.
            results["stage"] = "validate HTTPS enrollment document in actual Chromium"
            probe = results["startup"]["https_get"]
            def failed_navigation(request):
                if request.method == "GET" and request.url == ORIGIN + "/__fixture/enroll":
                    probe["last_navigation_failure"] = navigation_failure(request.failure)
            white.on("requestfailed", failed_navigation)
            deadline = time.monotonic() + 45
            while True:
                probe["attempts"] += 1
                probe["status"] = None
                probe["last_navigation_failure"] = "none"
                try:
                    startup = white.goto(ORIGIN + "/__fixture/enroll", wait_until="domcontentloaded", timeout=5_000)
                    probe["status"] = http_status(startup.status if startup is not None else None)
                    if startup is not None and startup.status == 200 and white.get_by_test_id("fixture-enroll").is_visible():
                        probe["fixture_page_ready"] = True
                        break
                except BrowserTimeout:
                    probe["last_navigation_failure"] = "navigation_timeout"
                except Exception:
                    if probe["last_navigation_failure"] == "none":
                        probe["last_navigation_failure"] = "other_navigation_failure"
                require(time.monotonic() < deadline, "validated fixture HTTPS and real authority startup failed")
                require(probe["attempts"] < 60, "fixture readiness exceeded bounded navigation attempts")
                time.sleep(.25)
            white.remove_listener("requestfailed", failed_navigation)
            for page, role in ((white, "white"), (black, "black")):
                if page is black:
                    page.goto(ORIGIN + "/__fixture/enroll", wait_until="domcontentloaded")
                require(not any(cookie["name"] == SESSION_COOKIE for cookie in page.context.cookies()),
                        "fresh independent browser unexpectedly shares a session")
                require(page.evaluate("localStorage.getItem('tabula-acceptance-isolation')") is None
                        and page.evaluate("sessionStorage.getItem('tabula-acceptance-isolation')") is None,
                        "fresh browser shares another process's document storage")
                page.evaluate("role => { localStorage.setItem('tabula-acceptance-isolation', role); sessionStorage.setItem('tabula-acceptance-isolation', role); }", role)
                enroll_actual_page(page, role, results)
            third.goto(ORIGIN + GAME_PATH, wait_until="domcontentloaded")
            require(third.evaluate("localStorage.getItem('tabula-acceptance-isolation')") is None
                    and third.evaluate("sessionStorage.getItem('tabula-acceptance-isolation')") is None,
                    "third browser inherited opponent local/session storage")
            require(white.evaluate("localStorage.getItem('tabula-acceptance-isolation')") == "white"
                    and black.evaluate("localStorage.getItem('tabula-acceptance-isolation')") == "black",
                    "opponent browsers' isolated storage markers changed each other")
            results["independent_local_and_session_storage_verified"] = True
            require(not any(c["name"] == SESSION_COOKIE for c in third.context.cookies()),
                    "third independent browser inherited opponent credentials")
            facts = [context_facts(page) for page in (white, black)]
            require(all(fact["disposition"] == "authenticated" for fact in facts),
                    "fixture issuance did not create real authenticated sessions")
            require(facts[0]["account_id"] != facts[1]["account_id"],
                    "opponent browsers share the same actual account")
            cookies = [[c for c in b.cookies() if c["name"] == SESSION_COOKIE] for b in browsers[:2]]
            require(all(len(c) == 1 and c[0]["httpOnly"] and c[0]["secure"]
                        and c[0]["path"] == "/" and c[0]["sameSite"] == "Lax" for c in cookies),
                    "real issued session cookie violates browser channel protection")
            require(cookies[0][0]["value"] != cookies[1][0]["value"],
                    "opponent browsers share the same session credential")
            require(all(SESSION_COOKIE not in page.evaluate("document.cookie") for page in (white, black)),
                    "session credential is readable by document JavaScript")
            results["distinct_actual_accounts_sessions_and_httponly_cookies"] = True
            results["chromium_version"] = browsers[0].browser.version
            actions.append("Independent White and Black pages issued separate durable HttpOnly sessions")
            evidence = CaptureEvidence(artifacts, Path(__file__).resolve().parents[2])
            redacted_values = [fact["csrf_token"] for fact in facts] + [fact["account_id"] for fact in facts]
            redacted_values += [entry[0]["value"] for entry in cookies]
            white.goto(ORIGIN + "/", wait_until="domcontentloaded")
            white.locator("#featured").wait_for(state="visible", timeout=30_000)
            white.locator('ul[aria-labelledby="featured"] .card__action[href="/games/com.tabula.chess"]').wait_for(state="visible", timeout=30_000)
            evidence.capture(white, "00-dashboard-discovery.png", "Dashboard and featured-game discovery", "White browser",
                             "Served shell home and featured games visibly rendered after actual session issuance", secrets=redacted_values)
            white.goto(ORIGIN + "/games", wait_until="domcontentloaded")
            white.locator(".section__title").first.wait_for(state="visible", timeout=30_000)
            white.locator('ul[aria-labelledby="results"] .card__action[href="/games/com.tabula.chess"]').wait_for(state="visible", timeout=30_000)
            evidence.capture(white, "01-game-library.png", "Actual game discovery library", "White browser",
                             "Served library route visibly rendered", secrets=redacted_values)
            white.goto(ORIGIN + GAME_PATH, wait_until="domcontentloaded")
            white.get_by_test_id("online-create").wait_for(state="visible", timeout=30_000)
            white.get_by_test_id("online-join-code").wait_for(state="visible", timeout=30_000)
            white.get_by_test_id("online-join").wait_for(state="visible", timeout=30_000)
            evidence.capture(white, "02-create-join-controls.png", "Create and join controls", "White browser",
                             "Actual online create and join controls visibly available", secrets=redacted_values)

            results["stage"] = "create and join a real code through the shell"
            with white.expect_response(lambda r: urlsplit(r.url).path == "/api/v1/matches", timeout=30_000) as created_response:
                white.get_by_test_id("online-create").click()
            created = created_response.value.json()
            require(created_response.value.status == 200 and created["seat"] == 0,
                    "actual shell create action failed")
            match_id, code = created["match_id"], created["join_code"]
            require(white.get_by_test_id("online-code").inner_text().strip() == code,
                    "shell did not display the real returned join code")
            redacted_values.append(code)
            evidence.capture(white, "03-created-code-waiting.png", "Created match waiting for opponent; active code redacted", "White browser",
                             "Actual successful create response has rendered its waiting admission", secrets=redacted_values)
            denied(api(third, f"/api/v1/matches/{match_id}/grant", {"version": 1}, "A" * 43),
                   {401}, "third unauthenticated browser obtained opponent output")
            black.get_by_test_id("online-join-code").fill(code)
            with black.expect_response(lambda r: urlsplit(r.url).path == "/api/v1/matches/join", timeout=30_000) as joined_response:
                black.get_by_test_id("online-join").click()
            joined = joined_response.value.json()
            require(joined_response.value.status == 200 and joined["match_id"] == match_id
                    and joined["seat"] == 1 and joined["ready"], "actual code join failed")
            black.get_by_test_id("online-enter").wait_for(state="visible", timeout=30_000)
            evidence.capture(black, "04-opponent-joined.png", "Opponent joined the real match; code/input redacted", "Black browser",
                             "Actual successful join rendered the opposite seat and enter control", secrets=redacted_values)
            duplicate = api(black, "/api/v1/matches/join", {"version": 1, "code": code}, facts[1]["csrf_token"])
            require(duplicate["status"] == 200 and duplicate["body"]["seat"] == 1
                    and duplicate["body"]["match_id"] == match_id,
                    "same-player duplicate join is not idempotent")
            actions.append("White created a real code; Black joined it as the opposite seat; duplicate join preserved binding")
            # Test full-roster admission before completion can mask that boundary.
            third.goto(ORIGIN + "/__fixture/enroll")
            enroll_actual_page(third, "third", results)
            third_facts = context_facts(third)
            third_csrf = third_facts["csrf_token"]
            require(third_facts["account_id"] not in [f["account_id"] for f in facts],
                    "third fixture account is not independent")
            denied(api(third, "/api/v1/matches/join", {"version": 1, "code": code}, third_csrf),
                   {403, 409}, "third player changed a full live opponent roster")
            results["third_client_full_live_roster_join_denied"] = True
            results["stage"] = "attach and render opposing actual browser seats"
            white_attachment, white_grant_body = enter_game(white, match_id, 0)
            black_attachment, _black_grant_body = enter_game(black, match_id, 1)
            initial_white = evidence.capture(white, "05-white-initial-board.png", "White independent rendered Chess board", "White browser",
                                             "Actual Rust-decoded seat0 initial projection rendered", canvas=True, secrets=redacted_values)
            evidence.capture(black, "06-black-initial-board.png", "Black independent rendered Chess board", "Black browser",
                             "Actual Rust-decoded seat1 initial projection rendered", canvas=True, secrets=redacted_values)

            results["stage"] = "tap f2-f3"
            first = move(white, "f2", "f3", False, match_id, results["moves"])
            wait_revision(black, 1)
            actions.append("White tapped f2-f3")
            results["stage"] = "tap e7-e5"
            # Both actual presenters start White-bottom; no flip control is used.
            move(black, "e7", "e5", False, match_id, results["moves"])
            wait_revision(white, 2)
            actions.append("Black tapped e7-e5")
            results["stage"] = "tap g2-g4"
            move(white, "g2", "g4", False, match_id, results["moves"])
            wait_revision(black, 3)
            actions.append("White tapped g2-g4")
            results["stage"] = "tap d8-h4 and render identical terminal verdicts"
            last = move(black, "d8", "h4", False, match_id, results["moves"])
            for page in (white, black):
                wait_revision(page, 4)
                page.wait_for_function("expected => document.documentElement.dataset.onlineStatus === expected", arg=TERMINAL_STATUS, timeout=30_000)
            results["terminal_views"] = [visible_game_facts(white, "white"), visible_game_facts(black, "black")]
            white_terminal = evidence.capture(white, "07-white-terminal-result.png", "White actual terminal result", "White browser",
                                              "Rendered Black wins and checkmate after four actual legal pointer moves; exact game-owned accessibility status includes checkmate", canvas=True, secrets=redacted_values)
            evidence.capture(black, "08-black-terminal-result.png", "Black actual terminal result", "Black browser",
                             "Independently rendered Black wins and checkmate after four actual legal pointer moves; exact game-owned accessibility status includes checkmate", canvas=True, secrets=redacted_values)
            evidence.capture(white, "09-terminal-result-page.png", "Authorized terminal result page and connection UI", "White browser",
                             "Actual completed game remains authorized before its document is closed", secrets=redacted_values)
            require(initial_white != white_terminal, "actual canvas pixels did not change after full game")
            results["both_rendered_terminal_verdicts"] = TERMINAL_STATUS
            results["full_game_pointer_moves"] = ["f2f3", "e7e5", "g2g4", "d8h4"]
            actions.append("Black tapped d8-h4; both independently rendered Game over / Black wins")

            # Stop White's live polling before a positive duplicate receipt can
            # be consumed outside its Rust client. Reuse only its own cookie jar.
            white.close()
            white = browsers[0].new_page()
            white.goto(ORIGIN + GAME_PATH, wait_until="domcontentloaded")

            results["stage"] = "replay exact committed command after actual rendered gameplay completes"
            # Consuming a fresh Ack outside Rust during play would create an
            # artificial transport-frame gap, so this probe follows both captures.
            duplicate_move = api(white, f"/api/v1/matches/{match_id}/command", first, facts[0]["csrf_token"])
            require(duplicate_move["status"] == 200 and any(
                frame.get("body", {}).get("Ack", {}).get("seq") == 1
                for frame in duplicate_move["body"]["frames"]),
                "exact duplicate committed command did not return its original acknowledgement")
            require(black.evaluate("document.documentElement.dataset.onlineRevision") == "4",
                    "duplicate command advanced the opponent projection")
            results["exact_duplicate_command_ack_without_new_projection"] = True
            actions.append("Exact original f2-f3 command returned its stored Ack without advancing the opponent projection")
            black.close()
            black = browsers[1].new_page()
            black.goto(ORIGIN + GAME_PATH, wait_until="domcontentloaded")

            results["stage"] = "deny third-client commands and private output"
            denied(api(third, f"/api/v1/matches/{match_id}/grant", {"version": 1}, third_csrf),
                   {403}, "third actual account obtained another player's grant")
            denied(api(third, f"/api/v1/matches/{match_id}/poll",
                       {"version": 1, "attachment_id": white_attachment["attachment_id"]}, third_csrf),
                   {403, 404}, "third account obtained opponent projection output")
            denied(api(third, f"/api/v1/matches/{match_id}/command", first, third_csrf),
                   {403, 404}, "third account issued an opponent command")
            results["third_client_command_and_output_denied"] = True

            results["stage"] = "reject real hostile Origin, CSRF, credential and envelope requests"
            results["hostile_header_partitions_denied"] = hostile_header_probes(
                ca, match_id, cookies[0][0]["value"], facts[0]["csrf_token"])
            fresh_grant = api(white, f"/api/v1/matches/{match_id}/grant", {"version": 1}, facts[0]["csrf_token"])
            require(fresh_grant["status"] == 200 and fresh_grant["body"]["ready"],
                    "fresh signed grant security partition setup failed")
            fresh_grant_body = {"version": 1, "binding_id": fresh_grant["body"]["binding_id"]}
            denied(api(black, f"/api/v1/matches/{match_id}/attach", fresh_grant_body, facts[1]["csrf_token"]),
                   {401}, "foreign-subject signed grant was accepted")
            denied(api(white, f"/api/v1/matches/{match_id}/attach",
                       {"version": 1, "binding_id": "invalid-grant"}, facts[0]["csrf_token"]),
                   {400}, "malformed signed grant was accepted")
            tampered = fresh_grant_body.copy()
            token = tampered["binding_id"]
            tampered["binding_id"] = token[:-2] + ("B" if token[-2] == "A" else "A") + token[-1]
            denied(api(white, f"/api/v1/matches/{match_id}/attach", tampered, facts[0]["csrf_token"]),
                   {401}, "invalid HMAC signed grant was accepted")
            invented_seat = json.loads(first)
            invented_seat["seat"] = 1
            denied(api(white, f"/api/v1/matches/{match_id}/command", json.dumps(invented_seat), facts[0]["csrf_token"]),
                   {400}, "client-selected seat bypassed DTO validation")
            zero_seq = json.loads(first)
            zero_seq["command"]["seq"] = 0
            denied(api(white, f"/api/v1/matches/{match_id}/command", json.dumps(zero_seq), facts[0]["csrf_token"]),
                   {400}, "zero-sequence envelope bypassed validation")
            wrong_match = json.loads(first)
            wrong_match["command"]["command"]["match_id"] = int(match_id, 16) ^ 1
            denied(api(white, f"/api/v1/matches/{match_id}/command", json.dumps(wrong_match), facts[0]["csrf_token"]),
                   {400}, "URL and opaque envelope match mismatch bypassed validation")
            results["hostile_grant_seat_and_envelope_partitions_denied"] = 6

            results["stage"] = "deny cross-match commands and private output"
            other = api(white, "/api/v1/matches", {"version": 1, "game_id": created["game_id"],
                        "seats": 2, "config": {"clock": "untimed"}}, facts[0]["csrf_token"])
            require(other["status"] == 200, "independent cross-match admission fixture failed")
            other_id = other["body"]["match_id"]
            second_join = api(black, "/api/v1/matches/join",
                              {"version": 1, "code": other["body"]["join_code"]}, facts[1]["csrf_token"])
            require(second_join["status"] == 200 and second_join["body"]["ready"],
                    "actual second-match opponent roster setup failed")
            second_grant = api(white, f"/api/v1/matches/{other_id}/grant", {"version": 1}, facts[0]["csrf_token"])
            require(second_grant["status"] == 200 and second_grant["body"]["ready"],
                    "actual second-match current grant setup failed")
            second_attach = api(white, f"/api/v1/matches/{other_id}/attach",
                                {"version": 1, "binding_id": second_grant["body"]["binding_id"]}, facts[0]["csrf_token"])
            require(second_attach["status"] == 200, "actual second-match actor setup failed")
            denied(api(white, f"/api/v1/matches/{other_id}/poll",
                       {"version": 1, "attachment_id": white_attachment["attachment_id"]}, facts[0]["csrf_token"]),
                   {403}, "same-account cross-match attachment released output")
            cross_command = json.loads(first)
            cross_command["command"]["command"]["match_id"] = int(other_id, 16)
            denied(api(white, f"/api/v1/matches/{other_id}/command", json.dumps(cross_command), facts[0]["csrf_token"]),
                   {403}, "same-account cross-match command was admitted")
            results["cross_match_command_and_output_denied"] = True

            results["stage"] = "rebind the same durable scope and fence its old attachment"
            grant = api(white, f"/api/v1/matches/{match_id}/grant", {"version": 1}, facts[0]["csrf_token"])
            require(grant["status"] == 200 and grant["body"]["ready"], "fresh authorized reattach grant failed")
            reattached = api(white, f"/api/v1/matches/{match_id}/attach",
                             {"version": 1, "binding_id": grant["body"]["binding_id"]}, facts[0]["csrf_token"])
            require(reattached["status"] == 200 and reattached["body"]["next_seq"] == 3,
                    "same actual session reattachment reset durable command sequence")
            denied(api(white, f"/api/v1/matches/{match_id}/poll",
                       {"version": 1, "attachment_id": white_attachment["attachment_id"]}, facts[0]["csrf_token"]),
                   {403}, "old attachment released output after same-session rebind")
            denied(api(white, f"/api/v1/matches/{match_id}/command", first, facts[0]["csrf_token"]),
                   {403}, "old attachment issued a command after same-session rebind")
            results["same_session_reattach_retained_next_seq_and_denied_old_attachment"] = True

            results["stage"] = "revoke through real authority and deny stale credential replay"
            revoked_cookie = {"name": SESSION_COOKIE, "value": cookies[1][0]["value"],
                              "url": ORIGIN + "/", "secure": True, "httpOnly": True, "sameSite": "Lax"}
            results["held_body_publication"] = prove_held_publication(
                white, black, other_id, second_attach["body"], first, facts,
                cookies[1][0]["value"], ca)
            actions.append("A real nonempty native poll body stayed held through committed logout; release delivered zero protected bytes")
            # Replay the exact previous runtime credential in the same browser.
            browsers[1].add_cookies([revoked_cookie])
            denied(api(black, f"/api/v1/matches/{match_id}/poll",
                       {"version": 1, "attachment_id": black_attachment["attachment_id"]}, facts[1]["csrf_token"]),
                   {401}, "revoked session obtained protected gameplay output")
            denied(api(black, f"/api/v1/matches/{match_id}/command", last, facts[1]["csrf_token"]),
                   {401}, "revoked session issued a cached opponent command")
            results["revoked_command_and_output_denied"] = True
            actions.append("Third-client, cross-match, and revoked-verifier commands and output were denied")

            results["stage"] = "conceal an actual live board after current authority loss"
            results["live_board_authority_loss"] = capture_live_authority_loss(
                white, third, facts[0]["csrf_token"], evidence, redacted_values)
            actions.append("A separate actual live Chess board received poll401 after normal logout and rendered neutral unavailable UI with projected pixels and status concealed")

            # Private account identities do not enter uploaded artifacts.
            (private / "audit-input.json").write_text(json.dumps(
                {"match_id": match_id, "accounts": [fact["account_id"] for fact in facts]}))
            results["status"] = "pass"
            results["stage"] = "complete"
    finally:
        if results["status"] != "pass":
            results["failure_views"] = []
            for variable, role in ((locals().get("white"), "white"), (locals().get("black"), "black")):
                try:
                    if variable is not None and not variable.is_closed() and urlsplit(variable.url).path == "/play/local/":
                        results["failure_views"].append(visible_game_facts(variable, role))
                except Exception:
                    results["failure_views"].append({"role": role, "diagnostic": "unavailable"})
        results["startup"]["processes"] = process_diagnostics(
            private, getattr(args, "native_pid", None), getattr(args, "tls_pid", None))
        (artifacts / "browser-result.json").write_text(json.dumps(results, indent=2) + "\n")
        (artifacts / "actions.json").write_text(json.dumps(actions, indent=2) + "\n")
        for browser in reversed(browsers):
            try:
                browser.close()
            except Exception:
                pass


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--private", required=True)
    parser.add_argument("--artifacts", required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--native-pid", type=int)
    parser.add_argument("--tls-pid", type=int)
    args = parser.parse_args()
    try:
        run(args)
    except AcceptanceFailure as error:
        print(f"FAIL: {error}")
        return 1
    except Exception:
        # Playwright errors can embed URLs, DOM text and request diagnostics.
        print("FAIL: browser acceptance infrastructure or actual interaction failed")
        return 1
    print("PASS: actual two-browser rendering and authority interaction")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
