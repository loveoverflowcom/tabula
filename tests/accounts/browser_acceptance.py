#!/usr/bin/env python3
"""Rendered account-shell acceptance with explicit synthetic HTTP doubles.

ADR-0036/ADR-0044: this exercises the built Leptos/WASM UI, not provider authentication,
PostgreSQL, secure transport, assistive technology, or production authority.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import struct
import asyncio
from collections import Counter, defaultdict
from contextlib import contextmanager
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import re
from pathlib import Path
import shutil
import ssl
import subprocess
import tempfile
import time
import threading
from urllib.parse import urlsplit
from http.cookies import SimpleCookie

from playwright.async_api import async_playwright, expect


ACCOUNT_A = "00000000000000000000000000000001"
ACCOUNT_B = "00000000000000000000000000000002"
CSRF = "A" * 43
CONTEXT = "/api/v1/auth/context"
PROFILE = "/api/v1/me"
REFRESH = "/api/v1/auth/refresh"
LOGOUT = "/api/v1/auth/logout"
LOGIN = "/api/v1/auth/login"
WIDTHS = (320, 390, 768, 1440)
THEMES = {
    "light": ("light", "no-preference"),
    "dark": ("dark", "no-preference"),
    "hc-light": ("light", "more"),
    "hc-dark": ("dark", "more"),
}
COPY = {
    "en": {
        "check": "Check session", "cancel": "Cancel", "logout": "Sign out",
        "refresh": "Refresh session", "retry": "Retry sign-out",
        "continue": "Continue with Kanidm", "signed_out": "You're signed out.", "account": "Account",
        "headings": {"/login": "Sign in", "/me": "Your profile",
                     "/register": "Create account", "/friends": "Friends"},
    },
    "vi": {
        "check": "Kiểm tra phiên đăng nhập", "cancel": "Hủy", "logout": "Đăng xuất",
        "refresh": "Làm mới phiên đăng nhập", "retry": "Thử đăng xuất lại",
        "continue": "Tiếp tục với Kanidm", "signed_out": "Bạn chưa đăng nhập.", "account": "Tài khoản",
        "headings": {"/login": "Đăng nhập", "/me": "Hồ sơ của bạn",
                     "/register": "Tạo tài khoản", "/friends": "Bạn bè"},
    },
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


ACTIVE_DOUBLES = {}

class SocketDouble:
    """Real browser WSS framing, synthetic app snapshots only; no account authority."""
    def __init__(self, connection, loop):
        self.connection,self.loop=connection,loop
        self.lock=threading.Lock()
        self.message=None
        self.closed=None
    def on_message(self,callback): self.message=callback
    def on_close(self,callback): self.closed=callback
    def send(self,message):
        payload=message.encode("utf-8")
        require(len(payload)<=512*1024,"synthetic social frame too large")
        header=bytes((0x81,len(payload))) if len(payload)<126 else bytes((0x81,126))+struct.pack("!H",len(payload)) if len(payload)<65536 else bytes((0x81,127))+struct.pack("!Q",len(payload))
        with self.lock: self.connection.sendall(header+payload)
    def read(self,stream):
        code,reason=None,None
        try:
            while True:
                header=stream.read(2)
                if len(header)!=2: break
                opcode=header[0]&15;length=header[1]&127
                require(header[1]&128,"browser synthetic WS frame unmasked")
                if length==126: length=struct.unpack("!H",stream.read(2))[0]
                elif length==127: length=struct.unpack("!Q",stream.read(8))[0]
                require(length<=512*1024,"incoming synthetic frame too large")
                mask=stream.read(4);payload=stream.read(length)
                require(len(mask)==4 and len(payload)==length,"truncated synthetic frame")
                payload=bytes(value^mask[index%4] for index,value in enumerate(payload))
                if opcode==8:
                    if len(payload)>=2: code=struct.unpack("!H",payload[:2])[0];reason=payload[2:].decode("utf-8")
                    with self.lock: self.connection.sendall(bytes((0x88,len(payload)))+payload)
                    break
                require(opcode==1,"unexpected synthetic client opcode")
                self.loop.call_soon_threadsafe(self.message,payload.decode("utf-8"))
        finally:
            if self.closed and not self.loop.is_closed(): self.loop.call_soon_threadsafe(self.closed,code,reason)

async def install_double(context,double,base):
    identifier=f"{id(double):x}"
    double.loop=asyncio.get_running_loop()
    ACTIVE_DOUBLES[identifier]=double
    await context.add_cookies([{"name":"tabula_ui_fixture","value":identifier,"url":base,"secure":True,"sameSite":"Strict"}])
    await context.route("**/api/**",double.handle)


@contextmanager
def static_https(dist: Path):
    """Disposable test certificate; browser trust is bypassed for this fixture."""
    dist = dist.resolve()
    require((dist / "index.html").is_file(), f"built shell missing: {dist}/index.html")
    require(any(dist.glob("*.wasm")), f"built WASM bundle missing: {dist}")
    with tempfile.TemporaryDirectory(prefix="tabula-account-ui-") as temporary:
        cert, key = Path(temporary) / "cert.pem", Path(temporary) / "key.pem"
        subprocess.run([
            "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
            "-keyout", str(key), "-out", str(cert), "-days", "1",
            "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1",
        ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        class Handler(SimpleHTTPRequestHandler):
            def __init__(self, *args, **kwargs):
                super().__init__(*args, directory=str(dist), **kwargs)

            def do_GET(self):
                path = urlsplit(self.path).path
                if path == "/api/v2/lobby/ws":
                    cookies=SimpleCookie(self.headers.get("Cookie",""))
                    marker=cookies.get("tabula_ui_fixture")
                    double=ACTIVE_DOUBLES.get(marker.value if marker else "")
                    if not double or self.headers.get("Sec-WebSocket-Protocol") != "tabula-social.v2.json":
                        self.send_error(403);return
                    key=self.headers.get("Sec-WebSocket-Key","")
                    accept=base64.b64encode(hashlib.sha1((key+"258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
                    self.send_response(101)
                    self.send_header("Upgrade","websocket");self.send_header("Connection","Upgrade")
                    self.send_header("Sec-WebSocket-Accept",accept);self.send_header("Sec-WebSocket-Protocol","tabula-social.v2.json")
                    self.end_headers();self.wfile.flush()
                    bridge=SocketDouble(self.connection,double.loop)
                    asyncio.run_coroutine_threadsafe(double.websocket(bridge),double.loop).result(timeout=5)
                    bridge.read(self.rfile)
                    self.close_connection=True
                    return
                if path in ("/account", "/me", "/login", "/register", "/friends", "/games") or path.startswith("/u/"):
                    self.path = "/index.html"
                super().do_GET()

            def log_message(self, *_args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.load_cert_chain(cert, key)
        server.socket = tls.wrap_socket(server.socket, server_side=True)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield f"https://127.0.0.1:{server.server_port}"
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


class HttpDouble:
    """Versioned response doubles, with observable held-request gates."""

    def __init__(self):
        self.disposition = "signed_out"
        self.account = ACCOUNT_A
        self.counts = Counter()
        self.held = {}
        self.entered = defaultdict(asyncio.Event)
        self.fail_logout = False
        self.errors = []
        self.completed = Counter()
        self.full = False
        self.enrollment = "ready"
        self.name = "Đặng Mai 東京 — Hồ sơ"
        self.visibility = "private"
        self.revision = 1
        self.profile_conflict = False
        self.profile_read_fail = False
        self.methods = Counter()
        self.unknown_mutation = False
        self.operations = {}
        self.sockets = set()
        self.socket_maximum = 0
        self.socket_opened = 0
        self.scope_counter = 20
        self.requests = []
        self.pause_stream = False
        self.ws_commands = Counter()
        self.emitters = []
        self.hold_resync = False
        self.delayed_resync = []
        self.mutation_payloads = []
        now = int(time.time() * 1000)
        for index, state in enumerate(("pending", "pending", "declined", "expired", "cancelled")):
            incoming = index == 0
            peer = self.identity(f"{3+index:032x}", f"peer_{index}")
            self.requests.append({"request_id":f"{100+index:032x}",
                "sender":peer if incoming else self.identity(ACCOUNT_A, "alice_fixture"),
                "recipient":self.identity(ACCOUNT_A, "alice_fixture") if incoming else peer,
                "status":state,"revision":1,"created_at_ms":now-1000,
                "expires_at_ms":now+86400000,"updated_at_ms":now-1000})

    def hold(self, path: str) -> None:
        require(path not in self.held, f"duplicate fixture hold: {path}")
        self.held[path] = asyncio.Event()
        self.entered[path] = asyncio.Event()

    async def wait_for_held(self, path: str) -> None:
        await asyncio.wait_for(self.entered[path].wait(), timeout=10)

    async def release(self, path: str) -> None:
        gate = self.held.pop(path)
        gate.set()
        # Drain the deliberately late HTTP response attempt before assertions.
        baseline = self.completed[path]
        for _ in range(100):
            if self.completed[path] > baseline:
                return
            await asyncio.sleep(0.01)
        raise AssertionError(f"held response did not drain: {path}")

    def context_body(self) -> dict:
        authenticated = self.disposition == "authenticated"
        unavailable = self.disposition == "unavailable"
        return {
            "version": 1, "disposition": self.disposition,
            "account_id": self.account if authenticated else None,
            "csrf_token": None if unavailable else CSRF,
            "capabilities": {"login": not authenticated and not unavailable,
                             "register": self.full and not authenticated and not unavailable, "friends": self.full and authenticated,
                             "read_self_profile": authenticated},
        }

    @staticmethod
    def identity(account, handle):
        return {"user_id":account,"handle":handle,"display_name":"Nguyễn 東京 — " + "漢" * 40}

    def v2_profile(self, other=False, handle="alice_fixture"):
        profile = {"version":2,"account_id":ACCOUNT_B if other else self.account,
            "handle":handle,"display_name":self.name,"as_of_ms":int(time.time()*1000)}
        if not other:
            profile.update(visibility=self.visibility, revision=self.revision)
        return profile

    def snapshot(self, scope, revision):
        now = int(time.time()*1000)
        return {"version":2,"viewer_id":self.account,"scope_id":scope,"revision":revision,
            "generated_at_ms":now,"friends":[{"identity":self.identity(ACCOUNT_B,"peer_fixture"),
                "presence":{"state":"online","as_of_ms":now}}],"requests":self.requests}

    async def websocket(self, socket):
        require(self.full, "legacy closed social capability opened a socket")
        self.sockets.add(socket)
        self.socket_maximum = max(self.socket_maximum, len(self.sockets))
        self.socket_opened += 1
        state = {"scope":None,"revision":0}
        def emit(new_scope=False):
            if new_scope:
                self.scope_counter += 1
                state.update(scope=f"{self.scope_counter:032x}", revision=0)
            state["revision"] += 1
            socket.send(json.dumps({"type":"snapshot","snapshot":self.snapshot(state["scope"],state["revision"])}))
        self.emitters.append(emit)
        def receive(message):
            parsed=json.loads(message)
            self.ws_commands[parsed.get("type")] += 1
            require(parsed.get("type") in ("hello","resync"), "unknown synthetic social command")
            if parsed["type"] == "hello":
                require(parsed.get("version") == 2, "social hello version mismatch")
            if parsed["type"] == "resync" and self.hold_resync: self.delayed_resync.append(emit)
            else: emit(True)
        socket.on_message(receive)
        socket.on_close(lambda *_: self.sockets.discard(socket))
        async def pulse():
            while socket in self.sockets:
                await asyncio.sleep(1)
                if socket in self.sockets and state["scope"] and not self.pause_stream:
                    try: emit()
                    except Exception: self.sockets.discard(socket)
        asyncio.create_task(pulse())

    async def handle(self, route) -> None:
        request = route.request
        path = urlsplit(request.url).path
        self.counts[path] += 1
        self.methods[request.method+" "+path] += 1
        # Capture at request admission: releasing after fixture mode changes must
        # deliver the old answer, rather than accidentally manufacture a fresh one.
        body, status = None, 204
        try:
            if path in (CONTEXT, PROFILE):
                require(request.method == "GET", f"read used mutation method: {path}")
                body = self.context_body() if path == CONTEXT else {
                    "version": 1, "account_id": self.account,
                }
                status = 503 if path == CONTEXT and self.disposition == "unavailable" else 200
            elif path in (LOGOUT, REFRESH, LOGIN):
                require(request.method == "POST", f"mutation used read method: {path}")
                require(request.headers.get("x-tabula-csrf") == CSRF,
                        f"synthetic mutation omitted its current document token: {path}")
                if path == LOGOUT and self.fail_logout:
                    status, body = 503, {"version": 1, "status": 503,
                                         "title": "Unavailable", "code": "unavailable"}
                elif path == LOGIN:
                    # No external provider is contacted; this explicit test error
                    # exercises the failed-start presentation only.
                    status, body = 503, {"version": 1, "status": 503,
                                         "title": "Unavailable", "code": "unavailable"}
            elif self.full and path.startswith("/api/v2/"):
                status = 200
                payload = request.post_data_json if request.post_data else None
                if request.method in ("POST","PATCH"):
                    require(request.headers.get("x-tabula-csrf") == CSRF, "v2 current control omitted")
                if path == "/api/v2/auth/enrollment":
                    ready = self.enrollment == "ready"
                    body = {"version":2,"disposition":self.enrollment,"operation_id":f"{500:032x}" if ready else None,
                        "csrf_token":CSRF if ready else None,"field_policy":{"handle_min_length":3,"handle_max_length":32,
                            "display_name_max_chars":64,"display_name_max_bytes":256} if ready else None,"agreement":None}
                elif path == "/api/v2/auth/enrollment/start":
                    status,body = 503,{"version":1,"status":503,"title":"Unavailable","code":"unavailable"}
                elif path == "/api/v2/auth/register":
                    require(request.method == "POST" and payload["operation_id"] == f"{500:032x}", "registration operation mismatch")
                    require("password" not in payload and "terms" not in payload, "invented enrollment fields")
                    self.enrollment="accepted_without_session"
                    body={"version":2,"disposition":self.enrollment}
                elif path == "/api/v2/profiles/me":
                    if request.method == "PATCH":
                        if self.profile_conflict:
                            status,body=409,{"version":1,"status":409,"title":"Conflict","code":"revision_conflict"}
                        else:
                            require(payload["expected_revision"] == self.revision, "profile CAS lost")
                            self.name=payload["display_name"];self.visibility=payload["visibility"];self.revision+=1
                            status,body=204,None
                    elif self.profile_read_fail:
                        self.profile_read_fail=False
                        status,body=503,{"version":1,"status":503,"title":"Unavailable","code":"unavailable"}
                    else: body=self.v2_profile()
                elif path.startswith("/api/v2/profiles/by-handle/"):
                    body=self.v2_profile(True,path.rsplit("/",1)[1])
                elif path == "/api/v2/social/search":
                    from urllib.parse import parse_qs
                    query=parse_qs(urlsplit(request.url).query).get("q",[""])[0]
                    body={"version":2,"viewer_id":self.account,"query":query,"results":[{
                        "identity":self.identity(ACCOUNT_B,"peer_fixture"),"relationship":"none","request":None}] if query else []}
                elif path == "/api/v2/social/mutate":
                    self.mutation_payloads.append(payload)
                    require(payload["operation_id"] not in ("", "0"*32), "invalid operation id")
                    if self.unknown_mutation:
                        status,body=503,{"version":1,"status":503,"title":"Unavailable","code":"unavailable"}
                    else:
                        if payload["action"] == "send":
                            now=int(time.time()*1000)
                            item={"request_id":f"{900:032x}","sender":self.identity(self.account,"alice_fixture"),
                                "recipient":self.identity(ACCOUNT_B,"peer_fixture"),"status":"pending","revision":1,
                                "created_at_ms":now,"expires_at_ms":now+86400000,"updated_at_ms":now}
                            self.requests=[row for row in self.requests if row["request_id"] != item["request_id"]]+[item]
                        else:
                            item=next(row for row in self.requests if row["request_id"] == payload["request_id"])
                            require(item["revision"] == payload["expected_revision"], "request CAS lost")
                            item.update(status={"accept":"accepted","decline":"declined","cancel":"cancelled"}[payload["action"]],revision=item["revision"]+1,updated_at_ms=int(time.time()*1000))
                        body={"version":2,"viewer_id":self.account,"operation_id":payload["operation_id"],"duplicate":False,"request":item}
                else: raise AssertionError(f"unexpected full API request: {path}")
            elif path == "/api/v2/profiles/me":
                require(request.method == "GET", "legacy profile probe mutated v2")
                status, body = 503, {"version":1,"status":503,"title":"Unavailable","code":"unavailable"}
            else:
                raise AssertionError(f"unexpected account API request: {path}")
            gate = self.held.get(path)
            if gate:
                self.entered[path].set()
                await asyncio.wait_for(gate.wait(), timeout=15)
            try:
                await route.fulfill(status=status, headers={
                    "Cache-Control": "no-store", "Content-Type": "application/json",
                }, body="" if body is None else json.dumps(body))
            except Exception as error:
                # Cancelling Fetch or retiring a route closes the HTTP request.
                # Only that observed transport outcome is permitted here.
                require("closed" in str(error).lower() or "invalid interception" in str(error).lower(),
                        f"unexpected late-response fixture error: {error}")
        except Exception as error:
            self.errors.append(str(error))
            try:
                await route.abort()
            except Exception:
                pass
        finally:
            self.completed[path] += 1


async def idle(page) -> None:
    await expect(page.locator(".account__state").first).to_have_attribute("aria-busy", "false")


async def hidden_private(page) -> None:
    await expect(page.locator("[data-account-private]:visible")).to_have_count(0)
    await no_identity(page)


async def no_identity(page) -> None:
    text = await page.locator("main").inner_text()
    require(ACCOUNT_A not in text and ACCOUNT_B not in text, "private identity remains rendered")
    await no_live_identity(page)


async def no_live_identity(page) -> None:
    for live in await page.locator('[role="status"], [role="alert"]').all_inner_texts():
        require(ACCOUNT_A not in live and ACCOUNT_B not in live, "private identity entered a live region")


async def visible_profile(page, identity=ACCOUNT_A) -> None:
    await idle(page)
    await expect(page.locator(".account__profile[data-account-private]:visible")).to_have_count(1)
    await expect(page.locator(".account__id")).to_have_text(identity)
    await no_live_identity(page)


async def render_check(page, locale: str, theme: str, route: str, state: str, full=False) -> dict:
    await expect(page.locator("#account-title")).to_have_text(COPY[locale]["headings"].get(route, "Profile" if locale == "en" else "Hồ sơ"))
    await expect(page.locator("html")).to_have_attribute("lang", locale)
    await expect(page.locator("html")).to_have_attribute("data-theme", theme)
    if state == "authenticated" and route == "/me":
        await visible_profile(page)
    elif state == "authenticated" and route == "/login":
        await no_identity(page)
        await expect(page.locator('main a[href="/me"]')).to_have_count(1)
    elif not full:
        await hidden_private(page)
    require(full or await page.locator("main input, main textarea, main [contenteditable]").count() == 0,
            "account route collects unavailable credentials or profile data")
    principal = page.locator("main .btn--principal")
    await expect(principal).to_have_count(1)
    require(await principal.is_enabled(), "task principal is unavailable")
    geometry = await page.evaluate(r"""() => {
      const root = document.documentElement;
      const task = document.querySelector('main .account');
      const controls = [...task.querySelectorAll('a,button,input,select')]
        .filter(e => e.getClientRects().length);
      const target = parseFloat(getComputedStyle(root).getPropertyValue('--sys-density-min-target'));
      const probe = document.createElement('span');
      probe.style.color = 'var(--sys-color-primary)';
      document.body.append(probe);
      const semanticPrimary = getComputedStyle(probe).color;
      probe.remove();
      const rgba = value => {
        if (!/^rgba?\(/.test(value)) throw Error('Unsupported computed color: ' + value);
        const channels = value.match(/[\d.]+/g).map(Number);
        return [...channels.slice(0,3).map(n => n / 255), channels[3] ?? 1];
      };
      const blend = (front, back) => front.slice(0,3)
        .map((n,i) => n * front[3] + back[i] * (1 - front[3]));
      const luminance = color => color.map(n => n <= .04045 ? n / 12.92
        : ((n + .055) / 1.055) ** 2.4)
        .reduce((sum,n,i) => sum + n * [.2126,.7152,.0722][i], 0);
      const contrast = control => {
        const layers = [];
        for (let el = control; el; el = el.parentElement)
          layers.push(rgba(getComputedStyle(el).backgroundColor));
        const background = layers.reverse().reduce((back, front) => blend(front, back), [1,1,1]);
        const foreground = blend(rgba(getComputedStyle(control).color), background);
        const values = [luminance(foreground), luminance(background)].sort((a,b) => b-a);
        return (values[0] + .05) / (values[1] + .05);
      };
      return {overflow: root.scrollWidth > innerWidth + 1,
        taskWidth: task.getBoundingClientRect().width,
        target, semanticPrimary, controls: controls.map(e => ({text: e.textContent.trim(),
          width: e.getBoundingClientRect().width, height: e.getBoundingClientRect().height,
          contrast: contrast(e), textButton: e.classList.contains('btn--text'),
          color: getComputedStyle(e).color}))};
    }""")
    require(not geometry["overflow"], f"horizontal overflow: {route}/{locale}/{theme}")
    require(0 < geometry["taskWidth"] <= 761, "task lost its bounded readable column")
    require(geometry["target"] > 0 and bool(geometry["controls"]), "target geometry check was empty")
    require(any(control["textButton"] for control in geometry["controls"]), "text-button selection was empty")
    for control in geometry["controls"]:
        require(control["height"] + 1 >= geometry["target"],
                f"control below generated target height: {control['text']}")
        require(control["contrast"] >= 4.5,
                f"account control text contrast below 4.5: {control['text']} ({control['contrast']:.2f})")
        if control["textButton"]:
            require(control["color"] == geometry["semanticPrimary"],
                    f"account text button lost semantic primary color: {control['text']}")
    if route in ("/register", "/friends"):
        require(await page.locator('main a[href="/login"]').count() == 1,
                "unavailable task has no fixed sign-in escape")
    require(await page.locator('main a[href="/games"]').count() >= 1,
            "local browsing escape is missing")
    return {"route": route, "state": state, "locale": locale, "theme": theme, "pass": True}


async def interactions(page, double: HttpDouble, base: str, locale: str) -> list[str]:
    copy = COPY[locale]
    main = page.locator("main")
    cases = []
    await page.goto(base + "/login")
    await page.locator("#locale").select_option(locale)
    await idle(page)
    double.hold(LOGOUT)
    await main.get_by_role("button", name=copy["continue"], exact=True).click()
    await double.wait_for_held(LOGOUT)
    await hidden_private(page)
    await expect(main.get_by_role("button", name=copy["continue"], exact=True)).to_have_count(0)
    await main.get_by_role("button", name=copy["cancel"], exact=True).click()
    await idle(page)
    await double.release(LOGOUT)
    require(double.counts[LOGIN] == 0, "cancelled preauth preparation started a provider request")
    await expect(page).to_have_url(base + "/login")
    cases.append("cancelled_login_preparation_never_starts_provider_request")

    await main.get_by_role("button", name=copy["check"], exact=True).click()
    await idle(page)
    await main.get_by_role("button", name=copy["continue"], exact=True).click()
    await idle(page)
    await hidden_private(page)
    require(double.counts[LOGIN] == 1, "explicit failed-start HTTP branch was not reached")
    await expect(main.get_by_role("alert")).to_be_visible()
    await expect(page).to_have_url(base + "/login")
    cases.append("synthetic_login_start_error_collects_no_credentials_or_identity")

    double.disposition = "authenticated"
    await page.goto(base + "/me")
    await page.locator("#locale").select_option(locale)
    await visible_profile(page)

    # Local confirmation must focus Cancel, send no mutation on Cancel/Escape,
    # and restore its still-live invoker rather than an unrelated heading.
    invoker = main.get_by_role("button", name=copy["logout"], exact=True)
    baseline = double.counts[LOGOUT]
    for action in ("cancel", "escape"):
        await invoker.click()
        prompt = page.locator("#logout-confirmation")
        await expect(prompt).to_be_visible()
        cancel = prompt.get_by_role("button", name=copy["cancel"], exact=True)
        await expect(cancel).to_be_focused()
        if action == "cancel":
            await cancel.press("Enter")
        else:
            await cancel.press("Escape")
        await expect(prompt).to_have_count(0)
        await expect(invoker).to_be_focused()
        await visible_profile(page)
        require(double.counts[LOGOUT] == baseline, "local confirmation cancel sent a mutation")
    cases.append("logout_cancel_escape_restore_focus_without_mutation")

    # A pending recheck retires private pixels and rejects a duplicate native
    # activation. Cancellation focuses the persistent successor; an old answer
    # released afterwards must never restore the old profile.
    double.hold(CONTEXT)
    await main.get_by_role("button", name=copy["check"], exact=True).click()
    await double.wait_for_held(CONTEXT)
    await hidden_private(page)
    check = main.get_by_role("button", name=copy["check"], exact=True)
    await expect(check).to_be_disabled()
    baseline = double.counts[CONTEXT]
    await check.evaluate("button => button.click()")
    require(double.counts[CONTEXT] == baseline, "duplicate pending check dispatched")
    await main.get_by_role("button", name=copy["cancel"], exact=True).click()
    await idle(page)
    await expect(page.locator("#account-title")).to_be_focused()
    await double.release(CONTEXT)
    await hidden_private(page)
    require(double.counts[PROFILE] >= 1, "authenticated branch was never reached")
    cases.append("pending_recheck_duplicate_cancel_and_late_context_retire_private")

    await main.get_by_role("button", name=copy["check"], exact=True).click()
    await visible_profile(page)
    before_context, before_profile = double.counts[CONTEXT], double.counts[PROFILE]
    double.hold(REFRESH)
    await main.get_by_role("button", name=copy["refresh"], exact=True).click()
    await double.wait_for_held(REFRESH)
    await hidden_private(page)
    await expect(page.locator("#account-title")).to_be_focused()
    await double.release(REFRESH)
    await visible_profile(page)
    require(double.counts[CONTEXT] > before_context and double.counts[PROFILE] > before_profile,
            "refresh restored identity without fresh context and profile requests")
    cases.append("refresh_retires_profile_until_fresh_context_and_profile")

    # The real browser offline event clears rendered identity synchronously.
    # Online recovery uses fresh doubles carrying another synthetic subject.
    await main.get_by_role("button", name=copy["logout"], exact=True).click()
    await expect(page.locator("#logout-confirmation").get_by_role(
        "button", name=copy["cancel"], exact=True)).to_be_focused()
    before_context, before_profile = double.counts[CONTEXT], double.counts[PROFILE]
    await page.context.set_offline(True)
    await hidden_private(page)
    await expect(page.locator("#account-title")).to_be_focused()
    await expect(page.locator("#logout-confirmation")).to_have_count(0)
    double.account = ACCOUNT_B
    await page.context.set_offline(False)
    await visible_profile(page, ACCOUNT_B)
    require(double.counts[CONTEXT] > before_context and double.counts[PROFILE] > before_profile,
            "online recovery restored identity without fresh requests")
    cases.append("offline_confirmation_focus_retirement_and_fresh_online_recovery")

    # An account-independent escape remains usable and keeps its focus when a
    # private task retires; lifecycle cleanup must not indiscriminately refocus.
    escape = main.locator('a[href="/games"]')
    await escape.focus()
    await page.context.set_offline(True)
    await hidden_private(page)
    await expect(escape).to_be_focused()
    await page.context.set_offline(False)
    await visible_profile(page, ACCOUNT_B)
    await expect(escape).to_be_focused()
    cases.append("offline_preserves_focus_on_account_independent_escape")

    await page.goto(base + "/login")
    await page.locator("#locale").select_option(locale)
    await idle(page)
    await no_identity(page)
    await expect(main.locator(".account__id")).to_have_count(0)
    profile_link = main.locator('a[href="/me"]')
    await expect(profile_link).to_have_count(1)
    await profile_link.focus()
    await page.context.set_offline(True)
    await hidden_private(page)
    await expect(page.locator("#account-title")).to_be_focused()
    await page.context.set_offline(False)
    await idle(page)
    await expect(profile_link).to_be_visible()
    await no_identity(page)
    cases.append("offline_signed_in_login_link_focus_moves_to_persistent_heading")
    before_context, before_profile = double.counts[CONTEXT], double.counts[PROFILE]
    await profile_link.click()
    await expect(page).to_have_url(base + "/me")
    await expect(page.locator("#account-title")).to_have_text(copy["headings"]["/me"])
    await visible_profile(page, ACCOUNT_B)
    require(double.counts[CONTEXT] > before_context and double.counts[PROFILE] > before_profile,
            "profile route reused the prior task's authority instead of rechecking")
    cases.append("signed_in_login_links_to_profile_without_rendering_identity")

    # Traverse the actual shell links between different route owners, rather
    # than full-document goto (which cannot detect disposed closure rebuilds).
    async def current_route(route: str) -> None:
        await expect(page).to_have_url(base + route)
        title = copy["account"] if route == "/account" else copy["headings"][route]
        await expect(page.locator("#account-title")).to_have_text(title)
        if route in ("/account", "/me"):
            await visible_profile(page, ACCOUNT_B)
        elif route == "/login":
            await idle(page)
            await no_identity(page)
            await expect(main.locator('a[href="/me"]')).to_be_visible()
        else:
            await hidden_private(page)
            await expect(main.locator('a[href="/login"]')).to_be_visible()

    await page.locator('.bottom-nav a[href="/account"]').click()
    await current_route("/account")
    await main.locator('a[href="/register"]').click()
    await current_route("/register")
    await main.locator('a[href="/login"]').click()
    await current_route("/login")
    await main.locator('a[href="/me"]').click()
    await current_route("/me")
    cases.append("account_register_login_profile_links_replace_route_owned_views")

    before_context, before_profile = double.counts[CONTEXT], double.counts[PROFILE]
    await page.go_back()
    await current_route("/login")
    await page.go_forward()
    await current_route("/me")
    require(double.counts[CONTEXT] > before_context and double.counts[PROFILE] > before_profile,
            "client history restored profile without fresh route authority")
    cases.append("client_history_back_forward_rechecks_login_and_profile")

    await main.locator('a[href="/friends"]').click()
    await current_route("/friends")
    await main.locator('a[href="/account"]').click()
    await current_route("/account")
    await main.locator('a[href="/friends"]').click()
    await current_route("/friends")
    await main.locator('a[href="/login"]').click()
    await current_route("/login")
    await main.locator('a[href="/me"]').click()
    await current_route("/me")
    cases.append("friends_back_account_and_login_links_keep_explicit_route_tasks")

    # Failed logout does not report signed-out authority or restore identity.
    # Its explicit retry succeeds only when the synthetic server acknowledges.
    double.fail_logout = True
    await main.get_by_role("button", name=copy["logout"], exact=True).click()
    await page.locator("#logout-confirmation").get_by_role(
        "button", name=copy["logout"], exact=True).click()
    await idle(page)
    await hidden_private(page)
    retry = main.get_by_role("button", name=copy["retry"], exact=True)
    await expect(retry).to_be_visible()
    require(copy["signed_out"] not in await main.inner_text(), "failure asserted confirmed sign-out")
    double.fail_logout = False
    await retry.click()
    await idle(page)
    await hidden_private(page)
    await expect(retry).to_have_count(0)
    await expect(main.get_by_role("status")).to_have_text(copy["signed_out"])
    cases.append("uncertain_logout_keeps_profile_hidden_until_acknowledged_retry")

    # Navigate away while a profile is pending, then deliver that old profile.
    # This exercises a disposed route owner instead of only operation Cancel.
    double.hold(PROFILE)
    await page.goto(base + "/me")
    await page.locator("#locale").select_option(locale)
    await double.wait_for_held(PROFILE)
    await main.locator('a[href="/games"]').click()
    await expect(page).to_have_url(base + "/games")
    await double.release(PROFILE)
    await hidden_private(page)
    cases.append("late_profile_after_route_exit_never_restores_private_output")
    require(not double.errors, f"HTTP fixture errors: {double.errors}")
    return cases


async def v2_interactions(page,double,base,locale):
    cases=[]
    await page.goto(base+"/register");await page.locator("#locale").select_option(locale)
    handle,name=page.locator("#register-handle"),page.locator("#register-display-name")
    await expect(handle).to_be_visible()
    session=await page.context.new_cdp_session(page)
    tree=await session.send("Accessibility.getFullAXTree")
    names={node.get("name",{}).get("value") for node in tree["nodes"] if not node.get("ignored")}
    require(("Handle" if locale=="en" else "Tên tài khoản") in names and ("Display name" if locale=="en" else "Tên hiển thị") in names,"native account fields missing AX names")
    await handle.fill("INVALID");await name.fill("Đặng 東京")
    await page.get_by_test_id("register-submit").click()
    await expect(page.locator("#registration-error")).to_be_visible()
    require(double.counts["/api/v2/auth/register"]==0,"invalid native form dispatched")
    await handle.fill("alice_fixture")
    await name.dispatch_event("compositionstart",{"data":"東京"})
    await page.get_by_test_id("register-submit").click()
    require(double.counts["/api/v2/auth/register"]==0,"composition dispatched enrollment")
    await name.dispatch_event("compositionend",{"data":"東京"})
    double.hold("/api/v2/auth/register")
    await name.press("Enter");await double.wait_for_held("/api/v2/auth/register")
    await expect(page.get_by_test_id("register-submit")).to_be_disabled()
    await page.get_by_test_id("register-submit").evaluate("button=>button.click()")
    require(double.counts["/api/v2/auth/register"]==1,"duplicate pending registration")
    await double.release("/api/v2/auth/register")
    await expect(page.get_by_test_id("registration-accepted")).to_be_visible()
    await expect(handle).to_have_count(0)
    require(double.disposition=="signed_out","registration incorrectly authenticated fixture")
    for reconciled in (False, True):
        if reconciled:
            before_enrollment = double.counts["/api/v2/auth/enrollment"]
            await page.reload();await page.locator("#locale").select_option(locale)
            await expect(page.get_by_test_id("registration-accepted")).to_be_visible()
            require(double.counts["/api/v2/auth/enrollment"]>before_enrollment,"accepted enrollment was not freshly read")
        await expect(page.locator("main .account__state [role=status]")).to_have_text(
            "Account information is ready." if locale=="en" else "Thông tin tài khoản đã sẵn sàng.")
        primary = page.locator("main .btn--principal")
        await expect(primary).to_have_count(1)
        await expect(primary).to_have_attribute("href","/login")
        await expect(primary).to_be_enabled()
        await expect(page.get_by_test_id("enrollment-start")).to_have_count(0)
    cases.extend(["native_registration_labels_ax_validation_composition","keyboard_pending_duplicate_registration_explicit_signedout_acceptance"])
    double.disposition="authenticated"
    await page.goto(base+"/me");await page.locator("#locale").select_option(locale)
    await expect(page.get_by_test_id("profile-edit")).to_be_visible();await page.get_by_test_id("profile-edit").click()
    await page.locator("#profile-display-name").fill("Nguyễn 東京 — đổi")
    await page.locator("#profile-visibility").select_option("friends")
    double.hold("/api/v2/profiles/me")
    await page.get_by_test_id("profile-save").click();await double.wait_for_held("/api/v2/profiles/me")
    await page.get_by_test_id("profile-save").evaluate("button=>button.click()")
    require(double.counts["/api/v2/profiles/me"]==2,"duplicate pending profile PATCH")
    await double.release("/api/v2/profiles/me")
    await expect(page.get_by_test_id("profile-edit")).to_be_visible()
    require(double.counts["/api/v2/profiles/me"]==3 and double.visibility=="friends","204 save did not read fresh self profile")
    cases.append("profile_cas_pending_duplicate_204_scoped_refetch")
    await page.get_by_test_id("profile-edit").click();await page.locator("#profile-display-name").fill("Conflict draft")
    double.profile_conflict=True
    await page.get_by_test_id("profile-save").click()
    await expect(page.get_by_test_id("profile-save")).to_be_disabled()
    require(double.counts["/api/v2/profiles/me"]==4,"profile conflict case not selected")
    double.profile_conflict=False
    await page.get_by_role("button",name="Refetch current profile" if locale=="en" else "Tải lại hồ sơ hiện tại",exact=True).click()
    await expect(page.get_by_test_id("profile-save")).to_be_enabled()
    await expect(page.locator("#profile-display-name")).to_have_value("Conflict draft")
    cases.append("profile_conflict_requires_explicit_refetch_preserves_draft")
    double.profile_read_fail=True
    await page.get_by_test_id("profile-save").click()
    retry=page.get_by_role("button",name="Retry the same request" if locale=="en" else "Thử lại cùng yêu cầu",exact=True)
    await expect(retry).to_be_visible()
    await expect(page.get_by_test_id("profile-edit")).to_have_count(0)
    writes=double.methods["PATCH /api/v2/profiles/me"]
    await retry.click();await expect(page.get_by_test_id("profile-edit")).to_be_visible()
    require(double.methods["PATCH /api/v2/profiles/me"]==writes,"acknowledged204 recovery reissued mutation")
    cases.append("acknowledged_profile_save_failed_read_exposes_get_only_recovery")
    await page.locator('main a[href="/friends"]').first.click()
    await expect(page.locator("#friends-query")).to_be_visible()
    await expect(page.get_by_test_id("friend-accept")).to_be_visible()
    opened=double.socket_opened
    await page.locator('main a[href="/me"]').first.click();await expect(page.get_by_test_id("profile-edit")).to_be_visible()
    await page.locator('main a[href="/friends"]').first.click();await expect(page.get_by_test_id("friend-accept")).to_be_visible()
    require(double.socket_maximum==1 and double.socket_opened==opened,"route navigation replaced shared social stream")
    cases.append("single_social_owner_and_fresh_scope_across_profile_routes")
    await page.locator("#friends-query").fill("peer_fixture");await page.get_by_test_id("friends-search").click()
    await expect(page.get_by_test_id("friend-send")).to_be_visible()
    double.hold("/api/v2/social/mutate")
    await page.get_by_test_id("friend-send").click();await double.wait_for_held("/api/v2/social/mutate")
    await page.get_by_test_id("friend-send").evaluate("button=>button.click()")
    require(double.counts["/api/v2/social/mutate"]==1,"duplicate pending friend request")
    await double.release("/api/v2/social/mutate")
    await expect(page.locator('[data-request-id="'+f'{900:032x}'+'"]')).to_be_visible()
    cases.append("friend_send_pending_duplicate_and_acknowledged_snapshot")
    # Freshness timeout must remove the live Online label while retaining permitted timestamps.
    double.pause_stream=True
    await expect(page.locator(".account__social-list").get_by_text("Online" if locale=="en" else "Trực tuyến",exact=True)).to_have_count(0,timeout=7000)
    await expect(page.locator(".account__social-list").get_by_text("Presence is stale" if locale=="en" else "Trạng thái đã cũ",exact=True)).to_be_visible()
    double.pause_stream=False
    await expect(page.locator(".account__social-list").get_by_text("Online" if locale=="en" else "Trực tuyến",exact=True)).to_be_visible()
    cases.append("social_five_second_freshness_becomes_stale_without_green_presence")
    # One outstanding resync under the prior route must not satisfy a new route ticket.
    double.hold_resync=True
    before_resync=double.ws_commands["resync"]
    await page.locator('main a[href="/me"]').first.click();await expect(page.get_by_test_id("profile-edit")).to_be_visible()
    for _ in range(100):
        if double.delayed_resync: break
        await asyncio.sleep(.01)
    require(bool(double.delayed_resync),"late-scope admission was not held")
    await page.locator('main a[href="/friends"]').first.click();await expect(page.locator("#friends-query")).to_be_visible()
    await expect(page.get_by_test_id("friend-accept")).to_have_count(0)
    double.hold_resync=False
    double.delayed_resync.pop(0)(True)
    await expect(page.get_by_test_id("friend-accept")).to_be_visible()
    require(double.ws_commands["resync"] >= before_resync+2,"prior-route scoped reply satisfied the new ticket")
    cases.append("late_serialized_resync_reply_is_discarded_before_new_route_scope")
    await page.locator("#friends-query").fill("peer_fixture");await page.get_by_test_id("friends-search").click()
    await expect(page.get_by_test_id("friend-send")).to_be_visible()
    double.unknown_mutation=True
    await page.get_by_test_id("friend-send").click()
    retry=page.get_by_role("button",name="Retry the same request" if locale=="en" else "Thử lại cùng yêu cầu",exact=True)
    await expect(retry).to_be_visible()
    await expect(page.locator("#friends-query")).to_be_disabled()
    original=double.mutation_payloads[-1]
    double.unknown_mutation=False
    await retry.click()
    await expect(page.locator("#friends-query")).to_be_enabled()
    await expect(page.locator(".account__state").get_by_role("status")).to_have_text("Your changes were saved." if locale=="en" else "Đã lưu thay đổi.")
    await expect(retry).to_have_count(0)
    require(double.mutation_payloads[-1]==original,"ambiguous friend write retry changed operation or intent")
    cases.append("ambiguous_friend_write_locks_new_intent_and_retries_identical_operation")
    await page.context.set_offline(True);await hidden_private(page)
    await expect(page.locator("#account-title")).to_be_focused()
    before=double.counts[CONTEXT]
    await page.context.set_offline(False);await expect(page.locator("#friends-query")).to_be_visible()
    await expect(page.get_by_test_id("friend-accept")).to_be_visible()
    require(double.counts[CONTEXT]>before,"social recovery lacked fresh context")
    cases.append("social_offline_private_retirement_and_fresh_authority_recovery")
    # A route-exited search cannot restore its old result list.
    double.hold("/api/v2/social/search")
    await page.locator("#friends-query").fill("peer_fixture");await page.get_by_test_id("friends-search").click()
    await double.wait_for_held("/api/v2/social/search")
    await page.locator('main a[href="/me"]').first.click();await expect(page.get_by_test_id("profile-edit")).to_be_visible()
    await double.release("/api/v2/social/search")
    await expect(page.get_by_test_id("friend-send")).to_have_count(0)
    cases.append("retired_search_response_cannot_restore_previous_route_list")
    return cases


async def run(base: str, args) -> dict:
    receipt = {
        "evidence_kind": "interaction-tested",
        "authority": "synthetic version-1/version-2 HTTP and snapshot WebSocket doubles; real rendered Leptos/WASM shell",
        "transport": "disposable self-signed HTTPS with browser trust bypass; no secure-transport claim",
        "excluded": ["real provider login", "password manager", "PostgreSQL",
                     "production session authority", "assistive technology", "BFCache"],
        "selection": "interactions_only" if args.only_interactions else "full_matrix_and_interactions",
        "render_cases": [], "interaction_cases": [], "v2_render_cases": [], "v2_interaction_cases": [], "page_errors": [],
    }
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(executable_path=args.chrome, headless=True)
        try:
            for width in (() if args.only_interactions else WIDTHS):
                for theme, (scheme, contrast) in THEMES.items():
                    for locale in COPY:
                        context = await browser.new_context(
                            ignore_https_errors=True, viewport={"width": width, "height": 900},
                            color_scheme=scheme, contrast=contrast, reduced_motion="reduce",
                            locale=locale,
                        )
                        double = HttpDouble()
                        await install_double(context,double,base)
                        page = await context.new_page()
                        page.on("pageerror", lambda error: receipt["page_errors"].append(str(error)))
                        try:
                            scenarios = [(route, "unavailable" if route in ("/register", "/friends")
                                          else "signed_out") for route in COPY[locale]["headings"]]
                            scenarios += [("/me", "authenticated"), ("/login", "authenticated")]
                            for route, state in scenarios:
                                double.disposition = state
                                await page.goto(base + route)
                                await page.locator("#locale").select_option(locale)
                                if route in ("/login", "/me"):
                                    await idle(page)
                                case = await render_check(page, locale, theme, route, state)
                                case["viewport"] = width
                                receipt["render_cases"].append(case)
                                if args.screenshots:
                                    await page.screenshot(path=str(args.screenshots /
                                        f"{route[1:]}-{state}-{width}-{theme}-{locale}.png"), full_page=True)
                            require(not double.errors, f"HTTP fixture errors: {double.errors}")
                        finally:
                            await context.close()
                        print(f"PASS render width={width} theme={theme} locale={locale}", flush=True)

            for width in (() if args.only_interactions else WIDTHS):
                for theme, (scheme, contrast) in THEMES.items():
                    for locale in COPY:
                        context = await browser.new_context(ignore_https_errors=True, viewport={"width":width,"height":900}, color_scheme=scheme, contrast=contrast, locale=locale, reduced_motion="reduce")
                        double=HttpDouble();double.full=True;double.name="Đặng 東京 — " + "漢"*40
                        await install_double(context,double,base)
                        page=await context.new_page()
                        page.on("pageerror",lambda error:receipt["page_errors"].append(str(error)))
                        try:
                            for route, state in (("/register","ready_enrollment"),("/me","ready_profile"),("/me","editing_profile"),("/u/peer_fixture","permitted_other"),("/friends","ready_social")):
                                double.disposition="signed_out" if route=="/register" else "authenticated"
                                await page.goto(base+route)
                                await page.locator("#locale").select_option(locale)
                                if route=="/register": await expect(page.locator("#register-handle")).to_be_visible()
                                elif route=="/me":
                                    await expect(page.get_by_test_id("profile-edit")).to_be_visible()
                                    if state=="editing_profile":
                                        await page.get_by_test_id("profile-edit").click()
                                        await expect(page.locator("#profile-display-name")).to_have_value(double.name)
                                elif route=="/friends":
                                    await expect(page.locator("#friends-query")).to_be_visible()
                                    await expect(page.get_by_test_id("friend-accept")).to_be_visible()
                                    stamp=page.locator("time").first
                                    await expect(stamp).to_have_attribute("datetime",re.compile(r"^\d{4}-\d{2}-\d{2}T"))
                                    require(not re.fullmatch(r"\d{13,}",await stamp.inner_text()),"presence timestamp remained raw epoch milliseconds")
                                    await page.locator("#friends-query").fill("peer_fixture")
                                    await page.get_by_test_id("friends-search").click()
                                    await expect(page.get_by_test_id("friend-send")).to_be_visible()
                                else: await expect(page.locator(".account__profile")).to_contain_text(double.name)
                                case=await render_check(page,locale,theme,route,state,True);case["viewport"]=width
                                receipt["v2_render_cases"].append(case)
                                if args.screenshots: await page.screenshot(path=str(args.screenshots/f"v2-{state}-{width}-{theme}-{locale}.png"),full_page=True)
                            require(not double.errors,f"v2 fixture errors: {double.errors}")
                            require(double.socket_maximum == 1,"v2 shell socket owner was duplicated")
                        finally:
                            for page in context.pages:
                                if not page.is_closed(): await page.evaluate("window.dispatchEvent(new Event('pagehide'))")
                            await asyncio.sleep(.1)
                            await context.close()
                        print(f"PASS v2 render width={width} theme={theme} locale={locale}",flush=True)

            for locale in COPY:
                context = await browser.new_context(
                    ignore_https_errors=True, viewport={"width": 390, "height": 900},
                    color_scheme="light", reduced_motion="reduce", locale=locale,
                )
                double = HttpDouble()
                await install_double(context,double,base)
                page = await context.new_page()
                page.on("pageerror", lambda error: receipt["page_errors"].append(str(error)))
                try:
                    cases = await interactions(page, double, base, locale)
                    receipt["interaction_cases"].extend(
                        {"case": case, "locale": locale, "viewport": 390,
                         "theme": "light", "pass": True} for case in cases)
                except Exception:
                    print(json.dumps({"url": page.url, "api_counts": double.counts,
                                      "fixture_errors": double.errors,
                                      "page_errors": receipt["page_errors"],
                                      "main": (await page.locator("main").inner_html())[:8000]}, indent=2),
                          flush=True)
                    if args.screenshots:
                        await page.screenshot(path=str(args.screenshots /
                            f"interaction-failure-{locale}.png"), full_page=True)
                    raise
                finally:
                    await context.close()
                print(f"PASS interactions locale={locale} cases={len(cases)}", flush=True)
            for locale in COPY:
                context=await browser.new_context(ignore_https_errors=True,viewport={"width":390,"height":900},locale=locale,reduced_motion="reduce")
                double=HttpDouble();double.full=True
                await install_double(context,double,base)
                page=await context.new_page();page.on("pageerror",lambda error:receipt["page_errors"].append(str(error)))
                try:
                    cases=await v2_interactions(page,double,base,locale)
                    receipt["v2_interaction_cases"].extend({"case":case,"locale":locale,"pass":True} for case in cases)
                    require(not double.errors,f"v2 interaction fixture errors: {double.errors}")
                except Exception:
                    print(json.dumps({"url":page.url,"api_counts":double.counts,"fixture_errors":double.errors,"page_errors":receipt["page_errors"],"main":(await page.locator("main").inner_html())[:8000]},indent=2),flush=True)
                    if args.screenshots: await page.screenshot(path=str(args.screenshots/f"v2-interaction-failure-{locale}.png"),full_page=True)
                    raise
                finally:
                    for page in context.pages:
                        if not page.is_closed(): await page.evaluate("window.dispatchEvent(new Event('pagehide'))")
                    await asyncio.sleep(.1)
                    await context.close()
                print(f"PASS v2 interactions locale={locale} cases={len(cases)}",flush=True)
        finally:
            await browser.close()
    require(len(receipt["render_cases"]) == (0 if args.only_interactions else 192),
            "render matrix selection is empty or incomplete")
    require(len(receipt["interaction_cases"]) == 28, "interaction selection is empty or incomplete")
    require(len(receipt["v2_render_cases"]) == (0 if args.only_interactions else 160), "v2 render selection is empty or incomplete")
    require(len(receipt["v2_interaction_cases"]) == 24, "v2 interaction selection is empty or incomplete")
    require(not receipt["page_errors"], f"uncaught browser errors: {receipt['page_errors']}")
    receipt["status"] = "PASS"
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--web-dist", type=Path, required=True, help="Trunk-built dist served by disposable synthetic HTTPS/WSS fixture")
    parser.add_argument("--chrome", default=shutil.which("google-chrome"),
                        help="installed Chrome executable; otherwise Playwright's installed Chromium")
    parser.add_argument("--receipt", type=Path, required=True, help="write bounded execution receipt")
    parser.add_argument("--screenshots", type=Path, help="optional screenshot directory; capture is not inspection")
    parser.add_argument("--only-interactions", action="store_true",
                        help="focused52-case interaction run; omits the rendered matrix explicitly")
    args = parser.parse_args()
    if args.screenshots:
        args.screenshots.mkdir(parents=True, exist_ok=True)
    with static_https(args.web_dist) as base:
        receipt = asyncio.run(run(base, args))
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"PASS {len(receipt['render_cases'])} rendered route cases, 28 interactions, "
          f"{len(receipt['v2_render_cases'])} v2 rendered cases, 24 v2 interactions, 0 page errors; {args.receipt}")


if __name__ == "__main__":
    main()
