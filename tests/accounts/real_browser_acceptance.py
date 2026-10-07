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
POST_LIMIT = 4096
ACK_COPY = {
    "en": {"ready": "Account information is ready.", "saved": "Your changes were saved."},
    "vi": {"ready": "Thông tin tài khoản đã sẵn sàng.", "saved": "Đã lưu thay đổi."},
}


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
        self.failed_social_reads = []
        self.operation_ids = set()
        page.on("pageerror", lambda _error: self.errors.append("page_error"))
        page.on("requestfailed", self.request_failed)
        page.on("websocket", self.socket)

    def request_failed(self, request):
        path = urlsplit(request.url).path
        if path in ("/api/v2/social/search", "/api/v2/social/mutate") and len(self.failed_social_reads) < 16:
            self.failed_social_reads.append({
                "task": "search" if path.endswith("search") else "mutation",
                "kind": "aborted" if request.failure == "net::ERR_ABORTED" else "transport",
            })

    async def observe_documents(self, context, page):
        # A new document retires old socket handles. SPA navigation preserves
        # the app-owned socket count; this session never intercepts transport.
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


# Observe the existing public task state without altering it. Attribute history
# also proves a fast busy->ack transition delivered in one observer batch.
ACK_ARM = """({expected,accepted}) => {
  const root=document.querySelector('.account');
  const state=root?.querySelector(':scope > .account__state');
  if(!state || state.getAttribute('aria-busy')!=='false' || window.__tabulaQaAck) return false;
  const ack={busy:false,complete:false,valid:false,retired:false,overflow:false,changes:0};
  const retire=() => { if(!ack.complete) ack.retired=true; };
  const listeners=[];
  for(const [target,kinds] of [[window,['focus','pageshow','pagehide','online','offline']],
                             [document,['visibilitychange','freeze']]])
    for(const kind of kinds) { target.addEventListener(kind,retire); listeners.push([target,kind]); }
  const observer=new MutationObserver(records => {
    ack.changes+=records.length;
    if(ack.changes>64) { ack.overflow=true; observer.disconnect(); return; }
    const settled=records.filter(record => record.target===state && record.type==='attributes'
      && record.attributeName==='aria-busy' && record.oldValue==='true');
    const alertAdded=records.some(record => record.type==='childList'
      && Array.from(record.addedNodes).some(node => node.nodeType===Node.ELEMENT_NODE
        && (node.matches('[role=alert]') || node.querySelector('[role=alert]'))));
    // Reject even a subsequently removed error subtree or another busy cycle.
    // The original action has one completer; new requests/recovery are fenced
    // separately. This observes a fresh ack, not internal phase history.
    if(settled.length>1 || alertAdded) {
      ack.complete=true; ack.valid=false; observer.disconnect(); return;
    }
    if(settled.length) ack.busy=true;
    if(state.getAttribute('aria-busy')==='true') ack.busy=true;
    if(ack.busy && !ack.complete && state.getAttribute('aria-busy')==='false') {
      ack.complete=true;
      ack.valid=state.querySelector('[role=status]')?.textContent.trim()===expected
        && !root.querySelector('[role=alert]')
        && (!accepted || !!root.querySelector('[data-testid=registration-accepted]'));
      observer.disconnect();
    }
  });
  observer.observe(root,{subtree:true,attributes:true,attributeOldValue:true,
    attributeFilter:['aria-busy'],childList:true,characterData:true});
  ack.cleanup=() => { observer.disconnect(); for(const [target,kind] of listeners)
    target.removeEventListener(kind,retire); delete window.__tabulaQaAck; };
  window.__tabulaQaAck=ack;
  return true;
}"""


async def ui_action(page, observation, method, path, button, acknowledged, accepted=False):
    """One real UI request, its actual HTTP status, and its fresh validated UI ack.

    Chromium151 misreports EOF of no-store Fetch streams as ERR_ABORTED and
    loses getResponseBody (microsoft/playwright#42742). Do not reinterpret that
    signal or reread the response: require the client's busy->validated ack,
    then independently check committed authority in the caller. No replay.
    """
    require(path.startswith("/api/v2/"), "fixed same-origin account action required")
    url, document, route = ORIGIN + path, observation.document, page.url
    requests, retired = [], False
    def started(request):
        nonlocal retired
        intended = request.url == url and request.method == method
        recovery = (request.url == ORIGIN + "/api/v1/auth/context"
                    or (accepted and request.url == ORIGIN + "/api/v2/auth/enrollment"))
        if not intended and not recovery:
            return
        try:
            if request.frame != page.main_frame or request.resource_type != "fetch":
                retired = True
                return
        except PlaywrightError:
            retired = True
            return
        if intended:
            if len(requests) < 2:
                requests.append(request)
            else:
                retired = True
        if recovery:
            retired = True
    def navigated(frame):
        nonlocal retired
        if frame == page.main_frame:
            retired = True
    page.on("request", started)
    page.on("framenavigated", navigated)
    try:
        locale = await page.locator("#locale").input_value()
        require(locale in ACK_COPY, "known account acceptance locale required")
        expected = ACK_COPY[locale][acknowledged]
        require(await page.evaluate(ACK_ARM, {"expected": expected, "accepted": accepted}),
                "account action requires an idle task state")
        async with page.expect_response(lambda response: response.request in requests) as waiting:
            await button.click()
        response = await waiting.value
        require(response.status == 200, "actual account action was not acknowledged by HTTP")
        await page.wait_for_function("""() => {
          const ack=window.__tabulaQaAck;
          return !!ack && (ack.complete || ack.retired || ack.overflow);
        }""", timeout=15000)
        ack = await page.evaluate("""() => {
          const {busy,complete,valid,retired,overflow}=window.__tabulaQaAck;
          return {busy,complete,valid,retired,overflow};
        }""")
        require(ack == {"busy": True, "complete": True, "valid": True,
                        "retired": False, "overflow": False},
                "actual UI action lacked a fresh validated acknowledgment")
        require(not retired and observation.document == document and page.url == route,
                "account action authority or route retired before acknowledgment")
        require(len(requests) == 1 and response.request == requests[0]
                and requests[0].redirected_from is None and requests[0].redirected_to is None,
                "actual UI action issued ambiguous or redirected requests")
        await expect(page.locator('.account > .account__state')).to_have_attribute('aria-busy', 'false')
        await expect(page.locator('.account > .account__state [role=status]')).to_have_text(expected)
        await expect(page.locator('.account [role=alert]')).to_have_count(0)
        return requests[0]
    finally:
        page.remove_listener("request", started)
        page.remove_listener("framenavigated", navigated)
        try:
            await page.evaluate("window.__tabulaQaAck?.cleanup()")
        except PlaywrightError:
            pass


def canonical_id(value):
    return (isinstance(value, str) and len(value) == 32 and any(ch != '0' for ch in value)
            and all(ch in '0123456789abcdef' for ch in value))


def posted_operation(request, observation):
    # Actual client input remains bounded and job-private. No CSRF/header/cookie
    # values are read, and no fabricated response or mutation retry is produced.
    raw = request.post_data
    require(isinstance(raw, str) and len(raw.encode('utf-8')) <= POST_LIMIT,
            "actual operation payload exceeded its bound")
    value = request.post_data_json
    require(isinstance(value, dict) and canonical_id(value.get('operation_id')),
            "actual operation identity invalid")
    operation = value['operation_id']
    require(len(observation.operation_ids) < 16 and operation not in observation.operation_ids,
            "explicit new intent reused an earlier operation")
    observation.operation_ids.add(operation)
    return value


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


async def enroll(page, context, observation, config_path, handle, name, locale):
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
    request = await ui_action(page, observation, "POST", "/api/v2/auth/register",
                              page.get_by_test_id("register-submit"), "ready", accepted=True)
    registration = posted_operation(request, observation)
    require(set(registration) == {"operation_id", "handle", "display_name"}
            and registration["handle"] == handle and registration["display_name"] == name,
            "actual registration did not submit the committed fields")
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


async def search(page, observation, handle, viewer):
    await page.locator("#friends-query").fill(handle)
    path = "/api/v2/social/search?q=" + handle
    await ui_action(page, observation, "GET", path, page.get_by_test_id("friends-search"), "ready")
    fresh = await api(page, path)
    require(fresh["status"] == 200 and fresh["value"]["version"] == 2
            and fresh["value"]["query"] == handle and fresh["value"]["viewer_id"] == viewer,
            "fresh permitted directory read failed")
    results = fresh["value"]["results"]
    require(len(results) <= 20, "directory projection exceeded its bound")
    expected = [{"user_id": row["identity"]["user_id"], "handle": row["identity"]["handle"]}
                for row in results]
    require(all(canonical_id(row["user_id"]) for row in expected)
            and len({row["user_id"] for row in expected}) == len(expected),
            "fresh directory identities invalid")
    rows = page.locator('.account__social-list').first.locator(':scope > li[data-user-id]')
    await expect(rows).to_have_count(len(expected))
    rendered = await rows.evaluate_all("""nodes => nodes.map(node => ({
      user_id:node.dataset.userId,handle:node.querySelector('a')?.textContent.trim()
    }))""")
    require(rendered == expected, "rendered directory differed from fresh permitted authority")
    return results


async def mutate_button(page, observation, test_id, viewer):
    """Return the actual fresh committed request projection, never a response double."""
    action = {"friend-send": "send", "friend-cancel": "cancel",
              "friend-decline": "decline", "friend-accept": "accept"}[test_id]
    button = page.get_by_test_id(test_id).first
    await expect(button).to_be_visible()
    row = button.locator('xpath=ancestor::li[1]')
    resource = await row.get_attribute('data-user-id' if action == 'send' else 'data-request-id')
    require(canonical_id(resource), "actual friend action resource invalid")
    before = await api(page, "/api/v2/social")
    require(before["status"] == 200 and before["value"]["version"] == 2
            and before["value"]["viewer_id"] == viewer, "fresh pre-action participant projection failed")
    prior = before["value"]["requests"]
    request = await ui_action(page, observation, "POST", "/api/v2/social/mutate", button, "saved")
    sent = posted_operation(request, observation)
    require(sent.get("action") == action, "actual friend action differed from selected control")
    if action == 'send':
        require(set(sent) == {"operation_id", "action", "target_user_id"}
                and sent['target_user_id'] == resource and resource != viewer,
                "actual send target differed from selected identity")
    else:
        matches = [entry for entry in prior if entry['request_id'] == resource]
        require(len(matches) == 1 and matches[0]['status'] == 'pending',
                "selected pending request lacked current authority")
        old = matches[0]
        require(set(sent) == {"operation_id", "action", "request_id", "expected_revision"}
                and sent['request_id'] == resource and sent['expected_revision'] == old['revision']
                and (old['sender']['user_id'] == viewer if action == 'cancel'
                     else old['recipient']['user_id'] == viewer),
                "actual friend transition resource, revision or actor differed")
    after = await api(page, "/api/v2/social")
    require(after["status"] == 200 and after["value"]["version"] == 2
            and after["value"]["viewer_id"] == viewer, "fresh committed participant projection failed")
    if action == 'send':
        prior_ids = {entry['request_id'] for entry in prior}
        committed = [entry for entry in after['value']['requests']
                     if entry['sender']['user_id'] == viewer and entry['recipient']['user_id'] == resource
                     and entry['status'] == 'pending' and entry['request_id'] not in prior_ids]
        require(len(committed) == 1 and committed[0]['revision'] == 1
                and canonical_id(committed[0]['request_id']),
                "new pending request absent from fresh committed authority")
    else:
        committed = [entry for entry in after['value']['requests'] if entry['request_id'] == resource]
        status = {'cancel': 'cancelled', 'decline': 'declined', 'accept': 'accepted'}[action]
        require(len(committed) == 1 and committed[0]['status'] == status
                and committed[0]['revision'] == old['revision'] + 1
                and committed[0]['sender']['user_id'] == old['sender']['user_id']
                and committed[0]['recipient']['user_id'] == old['recipient']['user_id'],
                "terminal request transition absent from fresh committed authority")
        if action == 'accept':
            peer = old['sender']['user_id']
            require(len([friend for friend in after['value']['friends']
                         if friend['identity']['user_id'] == peer]) == 1,
                    "accepted relationship absent from fresh friendship projection")
    return committed[0]


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
            alice = await enroll(first, contexts[0], a, config_path, "tabula_alice", "Đặng Mai 東京", "en")
            bob = await enroll(second, contexts[1], b, peer_path, "tabula_bob", "Nguyễn Bình", "vi")
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
            require(len(await search(first, a, "tabula_bob", alice["account_id"])) == 1, "permitted exact search missing")
            stage = "friends_send_first_request"
            pending = await mutate_button(first, a, "friend-send", alice["account_id"])
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
            await mutate_button(first, a, "friend-cancel", alice["account_id"])
            stage = "friends_search_after_cancel"
            await search(first, a, "tabula_bob", alice["account_id"])
            stage = "friends_send_after_cancel"
            await mutate_button(first, a, "friend-send", alice["account_id"])
            stage = "friends_decline_request"
            await mutate_button(second, b, "friend-decline", bob["account_id"])
            stage = "friends_search_after_decline"
            await search(first, a, "tabula_bob", alice["account_id"])
            stage = "friends_send_after_decline"
            await mutate_button(first, a, "friend-send", alice["account_id"])
            stage = "friends_accept_request"
            accepted = await mutate_button(second, b, "friend-accept", bob["account_id"])
            require(accepted["status"] == "accepted", "acceptance stayed local")
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
        if isinstance(error, CheckFailure):
            receipt["reason"] = str(error)
        receipt["sockets"] = [{"frames": len(observation.frames), "maximum_active": observation.maximum,
                                "errors": len(observation.errors),
                                "failed_social_reads": observation.failed_social_reads}
                               for observation in observations]
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
