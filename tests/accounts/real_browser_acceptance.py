#!/usr/bin/env python3
"""Actual isolated account/social authority in independent trusted HTTPS browsers.

No HTTP doubles, certificate bypass, browser storage credentials or fixture auth.
The existing provider helper drives real Kanidm password/TOTP forms over verified
HTTPS; Chromium completes the real cookie-bound callback. It does not claim a
manual provider UI, assistive-technology, password-manager or OS IME examination.
Public receipts contain fixed case labels only. Private values remain job-local.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time
import traceback
from urllib.parse import urlsplit

from playwright.async_api import Error as PlaywrightError, async_playwright, expect

ORIGIN = "https://app.localhost:8444"
ROOT = Path(__file__).resolve().parents[2]
SESSION_COOKIE = "__Host-tabula_session"


class CheckFailure(Exception):
    """Fixed public-safe assertion message, unlike raw browser diagnostics."""


def browser_failure_kind(error):
    """Closed diagnostic labels; never publish raw exception text or a URL."""
    if not isinstance(error, PlaywrightError):
        return None
    text = str(error)
    for fragment, label in (
        ("No resource with given identifier found", "response_body_unavailable"),
        ("No data found for resource with given identifier", "response_body_unavailable"),
        ("Request content was evicted from inspector cache", "response_body_unavailable"),
        ("Execution context was destroyed", "document_context_retired"),
        ("strict mode violation", "ambiguous_locator"),
        ("Element is not attached", "detached_element"),
        ("Target page, context or browser has been closed", "browser_target_closed"),
        ("net::", "browser_transport"),
    ):
        if fragment in text:
            return label
    return "playwright_error"


def require(condition, message):
    if not condition:
        raise CheckFailure(message)


def private_config(path):
    require(path.is_file() and not path.is_symlink()
            and stat.S_IMODE(path.stat().st_mode) == 0o600, "private provider config required")
    value = json.loads(path.read_text())
    require(value["provider_origin"] == "https://localhost:8443"
            and value["callback_url"] == ORIGIN + "/api/v1/auth/oidc/callback",
            "fixed disposable provider required")
    return value


def trust_database(private, app_ca, provider_ca):
    """Task-only Chromium NSS. An existing user NSS requires a private wrapper."""
    nss = private / "xdg" / "pki" / "nssdb"
    nss.mkdir(parents=True, mode=0o700)
    for command in (["certutil", "-N", "--empty-password", "-d", "sql:" + str(nss)],
                    ["certutil", "-A", "-d", "sql:" + str(nss), "-n", "Tabula job app CA",
                     "-t", "C,,", "-i", str(app_ca)],
                    ["certutil", "-A", "-d", "sql:" + str(nss), "-n", "Tabula job provider CA",
                     "-t", "C,,", "-i", str(provider_ca)]):
        subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    executable = os.environ.get("TABULA_ACCOUNTS_CHROME")
    if (Path.home() / ".pki" / "nssdb").exists():
        require(bool(executable) and os.environ.get("TABULA_ACCOUNTS_PRIVATE_NSS") == "1",
                "existing user NSS needs an explicitly configured private mount wrapper")
    environment = dict(os.environ, XDG_DATA_HOME=str(private / "xdg"),
                       TABULA_BROWSER_NSS_DIR=str(nss))
    return executable, environment


async def api(page, path, method="GET", body=None, csrf=True):
    require(path.startswith("/api/"), "only fixed same-origin authority APIs")
    return await page.evaluate("""async ({path,method,body,csrf}) => {
      const headers={};
      if(body!==null) headers['Content-Type']='application/json';
      if(csrf && method!=='GET') {
        const c=await fetch('/api/v1/auth/context',{credentials:'same-origin',cache:'no-store'});
        const v=await c.json(); headers['X-Tabula-CSRF']=v.csrf_token;
      }
      const r=await fetch(path,{method,credentials:'same-origin',cache:'no-store',headers,
        body:body===null?undefined:JSON.stringify(body)});
      const text=await r.text(); let value=null; try { value=JSON.parse(text); } catch {}
      return {status:r.status,value};
    }""", {"path": path, "method": method, "body": body, "csrf": csrf})


async def operation(page):
    return await page.evaluate("""() => {
      const bytes=crypto.getRandomValues(new Uint8Array(16));
      return Array.from(bytes,b=>b.toString(16).padStart(2,'0')).join('');
    }""")


async def eventually(predicate, message, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        await asyncio.sleep(.1)
    raise CheckFailure(message)


class BrowserObservation:
    """Private in-memory frames; receipts expose only counts and case outcomes."""
    def __init__(self, page):
        self.errors = []
        self.frames = []
        self.active = 0
        self.maximum = 0
        self.opened = 0
        self.document = 0
        page.on("pageerror", lambda _error: self.errors.append("page_error"))
        page.on("websocket", self.socket)

    async def observe_documents(self, context, page):
        # A cross-document navigation retires the old CDP network body/socket
        # handles. Page.frameNavigated identifies that boundary; SPA navigation
        # uses Page.navigatedWithinDocument and preserves the app socket count.
        self.cdp = await context.new_cdp_session(page)
        await self.cdp.send("Page.enable")
        self.cdp.on("Page.frameNavigated", self.document_changed)

    def document_changed(self, event):
        if not event["frame"].get("parentId"):
            self.document += 1
            self.active = 0

    def socket(self, socket):
        if not socket.url.endswith("/api/v2/lobby/ws"):
            self.errors.append("unexpected_socket")
            return
        self.active += 1
        self.opened += 1
        self.maximum = max(self.maximum, self.active)
        document = self.document
        socket.on("close", lambda: setattr(self, "active", max(0, self.active - 1))
                  if document == self.document else None)
        socket.on("framereceived", lambda payload: self.frame(payload)
                  if document == self.document else None)

    def frame(self, payload):
        try:
            value = json.loads(payload)
            if value.get("type") == "snapshot":
                self.frames.append(value["snapshot"])
        except (TypeError, ValueError, KeyError):
            self.errors.append("invalid_socket_frame")

    def presence(self, peer):
        if not self.frames:
            return None
        return next((friend["presence"]["state"] for friend in self.frames[-1]["friends"]
                     if friend["identity"]["user_id"] == peer), None)


async def provider_callback(page, config_path, button, endpoint):
    # The app consumes the start body before navigating. Chromium then retires
    # that response's CDP body handle; observe its actual validated navigation
    # request instead of trying to reread a response from the previous document.
    async with page.expect_request(lambda request:
                                   request.resource_type == "document"
                                   and urlsplit(request.url).netloc == "localhost:8443"
                                   and urlsplit(request.url).path == "/ui/oauth2") as navigation:
        async with page.expect_response(lambda response: urlsplit(response.url).path == endpoint) as waiting:
            await button.click()
    response = await waiting.value
    require(response.status == 200, "provider start denied")
    authorization = (await navigation.value).url
    process = await asyncio.create_subprocess_exec(
        sys.executable, str(ROOT / "tests/kanidm/provider.py"), "authorize", "--config", str(config_path),
        stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
    output, _private_error = await asyncio.wait_for(
        process.communicate(json.dumps({"authorization_url": authorization}).encode()), 60)
    require(process.returncode == 0, "real provider helper failed")
    callback = json.loads(output)["callback_url"]
    require(callback.startswith(ORIGIN + "/api/v1/auth/oidc/callback?"), "unexpected callback target")
    await page.goto(callback, wait_until="domcontentloaded")
    await expect(page.locator("#account-title")).to_be_visible()


async def enroll(page, context, config_path, handle, name, locale):
    await page.goto(ORIGIN + "/register")
    await page.locator("#locale").select_option(locale)
    await provider_callback(page, config_path, page.get_by_test_id("enrollment-start"),
                            "/api/v2/auth/enrollment/start")
    await page.locator("#locale").select_option(locale)
    await expect(page.locator("#register-handle")).to_be_visible()
    await expect(page.locator("#register-handle")).to_have_attribute("autocomplete", "username")
    await expect(page.locator("input[type=password]")).to_have_count(0)
    await expect(page.locator("input[type=checkbox]")).to_have_count(0)
    await page.locator("#register-handle").fill(handle)
    await page.locator("#register-display-name").fill(name)
    # A composed Enter cannot submit while the browser reports composition.
    submitted = []
    page.on("request", lambda request: submitted.append(1)
            if urlsplit(request.url).path == "/api/v2/auth/register" else None)
    await page.locator("#register-display-name").dispatch_event("compositionstart", {"data": "Đặng"})
    await page.get_by_test_id("register-submit").click()
    await asyncio.sleep(.2)
    require(not submitted, "composition prematurely submitted")
    await page.locator("#register-display-name").dispatch_event("compositionend", {"data": "Đặng"})
    async with page.expect_response(lambda response: urlsplit(response.url).path == "/api/v2/auth/register") as result:
        await page.get_by_test_id("register-submit").click()
    registration = await result.value
    require(registration.status == 200
            and (await registration.json())["disposition"] == "accepted_without_session",
            "registration must establish durable account without login")
    await expect(page.get_by_test_id("registration-accepted")).to_be_visible()
    require(not any(cookie["name"] == SESSION_COOKIE for cookie in await context.cookies()),
            "registration issued an automatic session")
    state = await api(page, "/api/v1/auth/context")
    require(state["value"]["disposition"] == "signed_out", "registration became authenticated")
    await page.goto(ORIGIN + "/login")
    await page.locator("#locale").select_option(locale)
    await provider_callback(page, config_path,
                            page.get_by_role("button", name="Continue with Kanidm" if locale == "en" else "Tiếp tục với Kanidm", exact=True),
                            "/api/v1/auth/login")
    state = await api(page, "/api/v1/auth/context")
    require(state["value"]["disposition"] == "authenticated", "fresh verified login missing")
    cookies = [cookie for cookie in await context.cookies() if cookie["name"] == SESSION_COOKIE]
    require(len(cookies) == 1 and cookies[0]["secure"] and cookies[0]["httpOnly"]
            and cookies[0]["sameSite"] == "Lax", "browser session cookie flags")
    require(SESSION_COOKIE not in await page.evaluate("document.cookie"), "session cookie visible to script")
    profile = await api(page, "/api/v2/profiles/me")
    require(profile["status"] == 200 and profile["value"]["handle"] == handle
            and profile["value"]["display_name"] == name, "durable profile mismatch")
    return profile["value"]


async def search(page, handle):
    await page.locator("#friends-query").fill(handle)
    async with page.expect_response(lambda response: urlsplit(response.url).path == "/api/v2/social/search") as waiting:
        await page.get_by_test_id("friends-search").click()
    response = await waiting.value
    require(response.status == 200, "authorized directory search failed")
    try:
        return (await response.json())["results"]
    except PlaywrightError as error:
        error.tabula_step = "search_response_body"
        raise


async def mutate_button(page, marker):
    button = page.get_by_test_id(marker).first
    await expect(button).to_be_visible()
    async with page.expect_response(lambda response: urlsplit(response.url).path == "/api/v2/social/mutate") as waiting:
        await button.click()
    response = await waiting.value
    require(response.status == 200, "authorized friendship mutation failed")
    try:
        return await response.json()
    except PlaywrightError as error:
        error.tabula_step = "mutation_response_body"
        raise


async def fresh_scope(observation, previous, viewer):
    await eventually(lambda: bool(observation.frames)
                     and observation.frames[-1]["scope_id"] != previous
                     and observation.frames[-1]["viewer_id"] == viewer
                     and any(snapshot["scope_id"] == observation.frames[-1]["scope_id"]
                             and snapshot["revision"] == 1 and snapshot["viewer_id"] == viewer
                             for snapshot in observation.frames),
                     "route or reconnect lacked a new current-viewer scope")


async def run(args):
    require(all(os.environ.get(name) == "1" for name in
                ("TABULA_ACCOUNTS_DISPOSABLE", "TABULA_KANIDM_DISPOSABLE", "TABULA_KANIDM_SOCIAL")),
            "explicit disposable acceptance scope required")
    config_path = Path(os.environ["TABULA_KANIDM_TEST_CONFIG"])
    config = private_config(config_path)
    require("social_peer" in config, "independent real provider peer required")
    peer_config = dict(config, **config["social_peer"])
    peer_path = args.private / "peer.json"
    with os.fdopen(os.open(peer_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "w") as stream:
        json.dump(peer_config, stream)
    executable, environment = trust_database(args.private, args.ca, Path(config["ca_path"]))
    cases = []
    stage = "browser_start"
    contexts = []
    observations = []
    try:
        async with async_playwright() as playwright:
            for index, width in enumerate((390, 1440)):
                kwargs = {"user_data_dir": str(args.private / f"browser-{index}"), "headless": True,
                          "viewport": {"width": width, "height": 900}, "env": environment,
                          "args": ["--no-sandbox", "--disable-dev-shm-usage"], "reduced_motion": "reduce"}
                if executable:
                    kwargs["executable_path"] = executable
                context = await playwright.chromium.launch_persistent_context(**kwargs)
                context.set_default_timeout(15000)
                contexts.append(context)
                observations.append(BrowserObservation(context.pages[0]))
                await observations[-1].observe_documents(context, context.pages[0])
            first, second = (context.pages[0] for context in contexts)
            a, b = observations
            stage = "verified_registration_and_explicit_login"
            alice = await enroll(first, contexts[0], config_path, "tabula_alice", "Đặng Mai 東京", "en")
            bob = await enroll(second, contexts[1], peer_path, "tabula_bob", "Nguyễn Bình", "vi")
            cases.extend(["independent_real_provider_enrollment", "no_automatic_registration_session",
                          "committed_unicode_and_composition_guard", "secure_httponly_lax_cookie",
                          "fresh_durable_identity_login", "english_vietnamese_journeys"])

            stage = "profile_edit_and_permission"
            await first.goto(ORIGIN + "/me")
            await first.get_by_test_id("profile-edit").click()
            await first.locator("#profile-display-name").fill("Đặng Mai — 編集")
            await first.locator("#profile-visibility").select_option("private")
            async with first.expect_response(lambda response: urlsplit(response.url).path == "/api/v2/profiles/me"
                                              and response.request.method == "PATCH") as saving:
                await first.get_by_test_id("profile-save").click()
            require((await saving.value).status == 204, "durable profile save failed")
            await expect(first.get_by_test_id("profile-edit")).to_be_visible()
            private = await api(second, "/api/v2/profiles/by-handle/tabula_alice")
            missing = await api(second, "/api/v2/profiles/by-handle/absent_account")
            require(private == missing and private["status"] == 404, "private profile existence leaked")
            hidden_search = await api(second, "/api/v2/social/search?q=tabula_alice")
            require(hidden_search["status"] == 200 and not hidden_search["value"]["results"],
                    "directory enumerated a hidden profile")
            await second.goto(ORIGIN + "/u/tabula_alice")
            await expect(second.get_by_test_id("profile-edit")).to_have_count(0)
            require("Đặng Mai — 編集" not in await second.locator("main").inner_text(), "private name rendered")
            current = (await api(first, "/api/v2/profiles/me"))["value"]
            patch = {"operation_id": await operation(first), "expected_revision": current["revision"],
                     "display_name": current["display_name"], "visibility": "public"}
            require((await api(first, "/api/v2/profiles/me", "PATCH", patch))["status"] == 204,
                    "explicit public disclosure update failed")
            other = await api(second, "/api/v2/profiles/by-handle/tabula_alice")
            require(other["status"] == 200 and "visibility" not in other["value"]
                    and "revision" not in other["value"], "other-profile projection leaked self policy")
            cases.extend(["self_profile_edit_and_unicode", "private_missing_uniform_response",
                          "other_profile_authorized_fields_only"])

            stage = "friends_terminal_states_and_actor_permissions"
            await first.goto(ORIGIN + "/friends")
            await second.goto(ORIGIN + "/friends")
            await second.locator("#locale").select_option("vi")
            stage = "friends_initial_search"
            require(len(await search(first, "tabula_bob")) == 1, "permitted exact search missing")
            stage = "friends_send_first_request"
            sent = await mutate_button(first, "friend-send")
            pending = sent["request"]
            stage = "friends_read_pending_authority"
            durable = await api(first, "/api/v2/social")
            require(durable["status"] == 200 and any(
                request["request_id"] == pending["request_id"] and request["status"] == "pending"
                for request in durable["value"]["requests"]),
                "pending request absent from authoritative participant snapshot")
            forbidden = {"operation_id": await operation(first), "action": "accept",
                         "request_id": pending["request_id"], "expected_revision": pending["revision"]}
            stage = "friends_deny_sender_accept"
            require((await api(first, "/api/v2/social/mutate", "POST", forbidden))["status"] == 403,
                    "sender accepted own request")
            stage = "friends_cancel_request"
            await mutate_button(first, "friend-cancel")
            stage = "friends_search_after_cancel"
            await search(first, "tabula_bob")
            stage = "friends_send_after_cancel"
            await mutate_button(first, "friend-send")
            stage = "friends_decline_request"
            await mutate_button(second, "friend-decline")
            stage = "friends_search_after_decline"
            await search(first, "tabula_bob")
            stage = "friends_send_after_decline"
            await mutate_button(first, "friend-send")
            stage = "friends_accept_request"
            accepted = await mutate_button(second, "friend-accept")
            require(accepted["request"]["status"] == "accepted", "acceptance stayed local")
            cases.extend(["authorized_directory_search", "sender_cannot_accept_own_request",
                          "explicit_cancel_decline_resend_accept"])

            stage = "actual_wss_presence_and_route_scope"
            await eventually(lambda: a.presence(bob["account_id"]) == "online"
                             and b.presence(alice["account_id"]) == "online", "peer presence not observed")
            require(a.maximum == 1 and b.maximum == 1, "multiple simultaneous shell sockets")
            opened = a.opened
            previous_scope = a.frames[-1]["scope_id"]
            await first.locator('a[href="/me"]').first.click()
            await expect(first.locator("#account-title")).to_have_text("Your profile")
            await fresh_scope(a, previous_scope, alice["account_id"])
            await expect(first.get_by_test_id("profile-edit")).to_be_visible()
            require(a.active == 1 and a.opened == opened, "route change replaced app-owned socket")
            previous_scope = a.frames[-1]["scope_id"]
            await first.locator('a[href="/friends"]').first.click()
            await expect(first.locator("#friends-query")).to_be_visible()
            await fresh_scope(a, previous_scope, alice["account_id"])
            await eventually(lambda: a.presence(bob["account_id"]) == "online", "route resync lacked current presence")
            alice_peer = first.locator('[data-social-stream][data-user-id="' + bob["account_id"] + '"]')
            bob_peer = second.locator('[data-social-stream][data-user-id="' + alice["account_id"] + '"]')
            await expect(alice_peer).to_be_visible()
            await expect(alice_peer).to_contain_text("Online")
            previous_peer_scope = b.frames[-1]["scope_id"]
            await contexts[1].set_offline(True)
            await eventually(lambda: a.presence(bob["account_id"]) in ("offline", "stale"),
                             "disconnected peer remained online", timeout=20)
            await expect(second.locator('[data-account-private]:visible')).to_have_count(0)
            await contexts[1].set_offline(False)
            await fresh_scope(b, previous_peer_scope, bob["account_id"])
            await eventually(lambda: a.presence(bob["account_id"]) == "online", "peer reconnect lacked fresh authority", 20)
            await expect(bob_peer).to_be_visible()
            await expect(bob_peer).to_contain_text("Trực tuyến")
            for observation in (a, b):
                retired = set()
                current_scope = None
                previous_revision = 0
                for snapshot in observation.frames:
                    if snapshot["scope_id"] != current_scope:
                        require(snapshot["scope_id"] not in retired, "retired social scope returned")
                        if current_scope is not None:
                            retired.add(current_scope)
                        current_scope = snapshot["scope_id"]
                        previous_revision = 0
                    require(snapshot["revision"] == previous_revision + 1, "social revision gap or regression")
                    previous_revision = snapshot["revision"]
                    for friend in snapshot["friends"]:
                        presence = friend["presence"]
                        if presence["state"] == "online":
                            require(0 <= snapshot["generated_at_ms"] - presence["as_of_ms"] <= 5000,
                                    "online badge lacked a fresh authority timestamp")
            cases.extend(["actual_authenticated_trusted_wss", "server_observed_online_offline_reconnect",
                          "single_app_socket_across_routes", "offline_private_dom_retirement",
                          "monotonic_scoped_snapshot_resync"])

            stage = "revocation_and_browser_storage"
            private_values = {alice["account_id"], bob["account_id"], alice["handle"], bob["handle"],
                              alice["display_name"], bob["display_name"], "Đặng Mai — 編集"}
            for page, context in zip((first, second), contexts):
                current_context = (await api(page, "/api/v1/auth/context"))["value"]
                private_values.add(current_context["csrf_token"])
                private_values.update(cookie["value"] for cookie in await context.cookies())
            before_logout = a.frames[-1]["generated_at_ms"]
            require((await api(second, "/api/v1/auth/logout", "POST", {}))["status"] in (200, 204),
                    "actual durable logout failed")
            await eventually(lambda: a.frames[-1]["generated_at_ms"] > before_logout
                             and a.presence(bob["account_id"]) in ("offline", "stale"),
                             "revoked peer lacked a fresh retained relationship observation", 20)
            await expect(second.locator('[data-account-private]:visible')).to_have_count(0)
            for page in (first, second):
                storage = await page.evaluate("""() => ({
                  local:Object.entries(localStorage),session:Object.entries(sessionStorage)})""")
                require(all(not any(word in key.lower() for word in
                                    ('session','csrf','token','password','profile','friend','account'))
                            for entries in storage.values() for key, _value in entries),
                        "private browser storage persisted")
                persisted = json.dumps(storage, ensure_ascii=False)
                require(all(value not in persisted for value in private_values if value),
                        "private values persisted under innocuous storage keys")
                geometry = await page.evaluate("document.documentElement.scrollWidth <= innerWidth")
                require(geometry, "account/social horizontal overflow")
            require(not a.errors and not b.errors, "browser runtime or frame error")
            cases.extend(["durable_logout_fences_peer_presence", "private_authority_absent_from_browser_storage",
                          "small_large_screen_reflow", "zero_browser_runtime_errors"])
            require(len(cases) == 21 and len(set(cases)) == 21, "exact nonempty actual acceptance selection")
            receipt = {"status": "pass", "authority": "actual Kanidm 1.11.2 + PostgreSQL + isolated native HTTP/WSS",
                       "browser_contexts": 2, "certificate_trust": "task-only NSS; no TLS bypass",
                       "cases": [{"case": case, "pass": True} for case in cases],
                       "manual_at_os_ime_password_manager": "not_run"}
    except Exception as error:
        receipt = {"status": "fail", "stage": stage, "completed_cases": len(cases),
                   "failure_type": type(error).__name__}
        kind = browser_failure_kind(error)
        if kind:
            receipt["browser_failure_kind"] = kind
        step = getattr(error, "tabula_step", None)
        if step in ("search_response_body", "mutation_response_body"):
            receipt["browser_step"] = step
        if isinstance(error, CheckFailure):
            receipt["reason"] = str(error)
        receipt["sockets"] = [{"frames": len(observation.frames), "maximum_active": observation.maximum,
                                "errors": len(observation.errors)} for observation in observations]
        with os.fdopen(os.open(args.private / "browser-failure.txt", os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "w") as debug:
            traceback.print_exc(file=debug)
        raise RuntimeError("Actual account/social acceptance failed at " + stage) from None
    finally:
        for context in contexts:
            try:
                await context.close()
            except Exception:
                pass
        args.artifacts.mkdir(parents=True, exist_ok=True)
        (args.artifacts / "real-browser-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print("PASS actual account/social browsers: 21 cases; trusted HTTPS/WSS; no runtime errors", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--private", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    args = parser.parse_args()
    try:
        asyncio.run(run(args))
    except Exception as error:
        # The exception chain and browser diagnostics may contain private values.
        print(str(error) if isinstance(error, RuntimeError) else "Actual account/social acceptance failed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
