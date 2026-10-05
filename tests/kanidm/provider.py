#!/usr/bin/env python3
"""Disposable Kanidm 1.11.2 acceptance support; never a production admin tool.

The authorize command drives the provider's real server-rendered forms over
verified HTTPS. It does not render pixels or replace Tabula's OIDC verifier.
Secrets and callback URLs are returned only to the caller's captured pipe.
"""

import argparse
import errno
import hashlib
import hmac
import http.cookiejar
import json
import os
from pathlib import Path
import secrets
import socket
import ssl
import stat
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from html.parser import HTMLParser


VERSION = "1.11.2"
IMAGE_DIGEST = "sha256:d87475bf9c9cfd24872d8b25957c9fc13fc090ace09ddc37c3b25fe394b397ac"
BODY_LIMIT = 512 * 1024
ORIGIN = "https://localhost:8443"
CALLBACK = "https://app.localhost:8444/api/v1/auth/oidc/callback"
CLIENT_ID = "tabula_oidc_acceptance"
USERNAME = "tabula_oidc_person"
GROUP = "tabula_oidc_people"


class HarnessError(Exception):
    """A deliberately secret-free failure description."""

    def __init__(self, message, category="contract"):
        super().__init__(message)
        self.category = category


def require(condition, message, category="contract"):
    if not condition:
        raise HarnessError(message, category)


def transport_category(error):
    """Classify types/codes only, never publish exception text or request URLs."""
    reason = error.reason if isinstance(error, urllib.error.URLError) else error
    if isinstance(reason, ssl.SSLCertVerificationError):
        return "tls_certificate_verification"
    if isinstance(reason, ssl.SSLError):
        return "tls_handshake"
    if isinstance(reason, (TimeoutError, socket.timeout)):
        return "transport_timeout"
    if isinstance(reason, ConnectionRefusedError) or getattr(reason, "errno", None) == errno.ECONNREFUSED:
        return "connection_refused"
    return "network_transport"


def startup_categories(raw_output):
    """Allowlist fixed upstream error markers; raw output never leaves memory."""
    markers = {
        "configuration_parse": ("Configuration Parse Failure", "Unable to parse config version", "Unable to parse config from"),
        "configuration_missing_database": ("No db_path set in configuration",),
        "database_directory_unreadable": ("Unable to read metadata for database folder",),
        "tls_file_unreadable": ("Unable to read metadata for TLS chain file", "Unable to read metadata for TLS key file"),
        "ui_package_missing": ("Couldn't find htmx UI package path",),
        "listener_bind_failure": ("Failed to bind tcp listener",),
    }
    matched = sorted(category for category, patterns in markers.items() if any(marker in raw_output for marker in patterns))
    return matched or ["unclassified_startup_failure"]


def container_state(container):
    require(container.startswith("tabula-kanidm-"), "unexpected diagnostic container")
    process = subprocess.run(["docker", "inspect", "--format", "{{json .State}}", container],
                             capture_output=True, text=True, timeout=10, check=True)
    raw = json.loads(process.stdout)
    state = raw.get("Status")
    state = state if state in {"created", "running", "paused", "restarting", "removing", "exited", "dead"} else "unknown"
    code = raw.get("ExitCode")
    code = code if isinstance(code, int) and 0 <= code <= 255 else None
    return {"state": state, "exit_code": code}


def container_startup_categories(container):
    process = subprocess.run(["docker", "logs", "--tail", "50", container],
                             capture_output=True, text=True, timeout=10, check=False)
    return startup_categories((process.stdout + process.stderr)[:128 * 1024])


def public_diagnostic(path, value):
    # Only the closed fields/categories constructed by this module belong here.
    path.write_text(json.dumps(value, indent=2) + "\n")


def probe_ready(client):
    _, _, body = client.request("GET", "/status", authenticated=False)
    try:
        healthy = json.loads(body)
    except ValueError:
        raise HarnessError("provider status was invalid", "status_invalid") from None
    require(healthy is True, "provider status was not healthy", "status_unhealthy")
    # /status is added AFTER version_middleware in upstream. Check the version
    # on /robots.txt, which is registered BEFORE that layer, instead.
    _, headers, _ = client.request("GET", "/robots.txt", authenticated=False)
    require(headers.get("X-KANIDM-VERSION") == VERSION,
            "provider version did not match the pin", "version_mismatch")


def wait_for_provider(ca_path, container, artifacts):
    deadline = time.monotonic() + 90
    client = Client(ca_path)
    last_probe = "not_probed"
    while True:
        try:
            probe_ready(client)
            public_diagnostic(artifacts / "provider-startup.json", {
                "status": "ready", "https_status": "healthy", "version": "matched",
                "container": container_state(container),
            })
            print("Verified HTTPS Kanidm " + VERSION + " is ready")
            return
        except HarnessError as error:
            last_probe = error.category
        state = container_state(container)
        if state["state"] in {"exited", "dead"} or time.monotonic() >= deadline:
            public_diagnostic(artifacts / "provider-startup.json", {
                "status": "failed", "last_probe": last_probe, "container": state,
                "known_startup_categories": container_startup_categories(container),
            })
            raise HarnessError("verified HTTPS provider readiness failed; see sanitized startup evidence", "readiness_failed")
        time.sleep(1)


def totp_code(parameters, timestamp=None):
    """RFC 6238 with upstream-provided algorithm, seed, digits and time step."""
    algorithm = parameters.get("algo")
    digits = parameters.get("digits")
    step = parameters.get("step")
    secret = parameters.get("secret")
    require(algorithm in ("sha1", "sha256", "sha512") and digits in (6, 8)
            and isinstance(step, int) and 0 < step <= 120,
            "unsupported disposable TOTP parameters")
    require(isinstance(secret, list) and 16 <= len(secret) <= 128
            and all(isinstance(value, int) and 0 <= value <= 255 for value in secret),
            "invalid disposable TOTP seed")
    timestamp = time.time() if timestamp is None else timestamp
    require(timestamp >= 0, "invalid TOTP timestamp")
    counter = int(timestamp) // step
    digest = hmac.new(bytes(secret), counter.to_bytes(8, "big"), getattr(hashlib, algorithm)).digest()
    offset = digest[-1] & 0x0f
    truncated = int.from_bytes(digest[offset:offset + 4], "big") & 0x7fffffff
    return str(truncated % (10 ** digits)).zfill(digits)


def private_json(path, value):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as stream:
        json.dump(value, stream)


def read_config(path):
    mode = stat.S_IMODE(path.stat().st_mode)
    require(mode == 0o600 and path.is_file() and not path.is_symlink(), "config must be a private regular file")
    config = json.loads(path.read_text())
    require(config.get("provider_origin") == ORIGIN, "only the disposable loopback provider is supported")
    require(config.get("callback_url") == CALLBACK, "unexpected disposable callback")
    require(config.get("client_id") == CLIENT_ID, "unexpected disposable client")
    return config


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class Client:
    def __init__(self, ca_path):
        context = ssl.create_default_context(cafile=str(ca_path))
        self.opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}),
            urllib.request.HTTPSHandler(context=context),
            urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()),
            NoRedirect(),
        )
        self.token = None
        self.auth_session = None

    def request(self, method, path, *, data=None, form=None, expected=(200,), authenticated=True):
        url = urllib.parse.urljoin(ORIGIN, path)
        parsed = urllib.parse.urlsplit(url)
        require(parsed.scheme == "https" and parsed.netloc == "localhost:8443", "provider request escaped loopback origin")
        headers = {"Accept": "application/json, text/html", "User-Agent": "Tabula-disposable-acceptance/1"}
        body = None
        if data is not None:
            body = json.dumps(data).encode()
            headers["Content-Type"] = "application/json"
        if form is not None:
            body = urllib.parse.urlencode(form).encode()
            headers["Content-Type"] = "application/x-www-form-urlencoded"
            headers["Origin"] = ORIGIN
        if authenticated and self.token:
            headers["Authorization"] = "Bearer " + self.token
        if self.auth_session:
            headers["X-KANIDM-AUTH-SESSION-ID"] = self.auth_session
        request = urllib.request.Request(url, data=body, headers=headers, method=method)
        try:
            response = self.opener.open(request, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        except (urllib.error.URLError, TimeoutError, ssl.SSLError) as error:
            raise HarnessError("provider HTTPS transport failed", transport_category(error)) from None
        with response:
            require(response.status in expected, "provider request returned unexpected HTTP status " + str(response.status), "http_status")
            content = response.read(BODY_LIMIT + 1)
            require(len(content) <= BODY_LIMIT, "provider body exceeded acceptance bound")
            self.auth_session = response.headers.get("X-KANIDM-AUTH-SESSION-ID")
            return response.status, response.headers, content

    def json(self, method, path, data=None, authenticated=True):
        _, _, body = self.request(method, path, data=data, authenticated=authenticated)
        return json.loads(body)

    def login(self, username, password):
        result = self.json("POST", "/v1/auth", {"step": {"init2": {
            "username": username, "issue": "token", "privileged": True,
        }}}, authenticated=False)
        require("password" in result.get("state", {}).get("choose", []), "password auth mechanism was not offered")
        result = self.json("POST", "/v1/auth", {"step": {"begin": "password"}}, authenticated=False)
        require("password" in result.get("state", {}).get("continue", []), "password auth challenge was not offered")
        result = self.json("POST", "/v1/auth", {"step": {"cred": {"password": password}}}, authenticated=False)
        token = result.get("state", {}).get("success")
        require(isinstance(token, str) and bool(token), "provider login did not issue a token")
        self.token = token


class Forms(HTMLParser):
    """Select only known provider forms; never invent a login or consent route."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.forms = []
        self.current = None

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "form":
            require(self.current is None, "nested provider form")
            self.current = {"action": attrs.get("action", ""), "method": attrs.get("method", "get").lower(), "fields": {}}
        elif tag == "input" and self.current is not None and attrs.get("name"):
            name = attrs["name"]
            require(name not in self.current["fields"], "duplicate provider form input")
            if attrs.get("type", "text").lower() not in ("checkbox", "radio", "submit", "button"):
                self.current["fields"][name] = attrs.get("value", "")

    def handle_endtag(self, tag):
        if tag == "form" and self.current is not None:
            self.forms.append(self.current)
            self.current = None


def authorization_parameters(config, url):
    parsed = urllib.parse.urlsplit(url)
    require(parsed.scheme == "https" and parsed.netloc == "localhost:8443" and parsed.path == "/ui/oauth2", "unexpected authorization endpoint")
    pairs = urllib.parse.parse_qsl(parsed.query, keep_blank_values=True, strict_parsing=True)
    require(len({key for key, _ in pairs}) == len(pairs), "duplicate authorization parameter")
    query = dict(pairs)
    expected = {"client_id": config["client_id"], "redirect_uri": config["callback_url"],
                "response_type": "code", "scope": "openid", "code_challenge_method": "S256", "max_age": "0"}
    require(all(query.get(key) == value for key, value in expected.items()), "authorization contract changed")
    require(query.get("prompt") == "login", "fresh provider login was not requested")
    require(all(query.get(key) for key in ("state", "nonce", "code_challenge")), "authorization binding is incomplete")
    return query


def callback_result(config, url, query):
    target = urllib.parse.urlsplit(url)
    expected = urllib.parse.urlsplit(config["callback_url"])
    require((target.scheme, target.netloc, target.path) == (expected.scheme, expected.netloc, expected.path), "provider callback target changed")
    require(not target.fragment, "provider returned an unexpected callback fragment")
    pairs = urllib.parse.parse_qsl(target.query, keep_blank_values=True, strict_parsing=True)
    require(len({key for key, _ in pairs}) == len(pairs), "duplicate provider callback parameter")
    params = dict(pairs)
    require(params.get("state") == query["state"] and bool(params.get("code")), "provider callback was unsuccessful or unbound")
    require(set(params) <= {"state", "code", "iss"}, "unexpected provider callback fields")
    if "iss" in params:
        require(params["iss"] == config["issuer"], "provider callback issuer changed")
    return {"callback_url": url}


def authorize(config, authorization_url, consent_receipt=None):
    query = authorization_parameters(config, authorization_url)
    previous_consent = False
    if consent_receipt is not None and consent_receipt.exists():
        require(not consent_receipt.is_symlink() and stat.S_IMODE(consent_receipt.stat().st_mode) == 0o600,
                "consent receipt is not private")
        previous_consent = json.loads(consent_receipt.read_text()) == {"actual_provider_consent_observed": True}
    client = Client(config["ca_path"])
    current = authorization_url
    method, fields = "GET", None
    saw_login, saw_totp, saw_resume, saw_consent = False, False, False, False
    for _ in range(12):
        status, headers, body = client.request(method, current, form=fields, expected=(200, 302, 303), authenticated=False)
        if status in (302, 303):
            location = headers.get("Location")
            require(bool(location), "provider redirect had no location")
            next_url = urllib.parse.urljoin(current, location)
            if urllib.parse.urlsplit(next_url).netloc != "localhost:8443":
                require(saw_login and saw_totp and saw_resume and (saw_consent or previous_consent),
                        "provider did not execute fresh MFA login/resume and establish consent")
                result = callback_result(config, next_url, query)
                if saw_consent and consent_receipt is not None and not consent_receipt.exists():
                    private_json(consent_receipt, {"actual_provider_consent_observed": True})
                return result
            require(urllib.parse.urlsplit(next_url).path == "/ui/oauth2/resume", "unexpected provider redirect")
            saw_resume = True
            current, method, fields = next_url, "GET", None
            continue
        parser = Forms()
        parser.feed(body.decode("utf-8"))
        forms = [form for form in parser.forms if form["action"] in {
            "/ui/login/begin", "/ui/login/mech_choose", "/ui/login/totp", "/ui/login/pw", "/ui/oauth2/consent",
        }]
        require(len(forms) == 1 and forms[0]["method"] == "post", "provider did not offer one supported form")
        form = forms[0]
        fields = form["fields"].copy()
        action = form["action"]
        if action == "/ui/login/begin":
            fields.update(username=config["username"], password="", totp="")
        elif action == "/ui/login/mech_choose":
            fields = {"mech": "passwordmfa"}
        elif action == "/ui/login/totp":
            require(set(fields) == {"totp", "password"} and not saw_login and not saw_totp,
                    "unexpected provider TOTP form or ordering")
            fields.update(totp=totp_code(config["totp"]), password="")
            saw_totp = True
        elif action == "/ui/login/pw":
            require(saw_totp and set(fields) == {"password"} and not saw_login,
                    "password form preceded MFA or was ambiguous")
            fields["password"] = config["password"]
            saw_login = True
        else:
            require(saw_login and saw_totp and saw_resume and bool(fields.get("consent_token")), "provider consent preceded fresh MFA authentication")
            require(set(fields) == {"consent_token"}, "unexpected provider consent fields")
            saw_consent = True
        current, method = urllib.parse.urljoin(ORIGIN, action), "POST"
    raise HarnessError("provider flow exceeded the bounded number of steps")


def bootstrap(args):
    require(os.environ.get("TABULA_KANIDM_DISPOSABLE") == "1", "disposable runner scope was not set")
    require(args.container.startswith("tabula-kanidm-"), "unexpected provider container")
    process = subprocess.run(["docker", "inspect", "--format", '{{ index .Config.Labels "org.tabula.purpose" }}', args.container],
                             capture_output=True, text=True, timeout=20, check=True)
    require(process.stdout.strip() == "disposable-kanidm-acceptance", "provider container was not labeled disposable")
    process = subprocess.run(["docker", "exec", args.container, "/sbin/kanidmd", "-c", "/data/server.toml", "scripting", "recover-account", "idm_admin"],
                             capture_output=True, text=True, timeout=30, check=True)
    recovered = json.loads(process.stdout)
    require(recovered.get("status") == "ok" and isinstance(recovered.get("output"), str), "disposable bootstrap recovery failed")
    client = Client(args.ca)
    client.login("idm_admin", recovered["output"])
    client.json("POST", "/v1/person", {"attrs": {"name": [USERNAME], "displayname": ["Tabula acceptance person"]}})
    client.json("POST", "/v1/group", {"attrs": {"name": [GROUP]}})
    client.json("PUT", "/v1/group/" + GROUP + "/_attr/member", [USERNAME])
    password = secrets.token_urlsafe(36)
    session_token, _ = client.json("GET", "/v1/person/" + USERNAME + "/_credential/_update")
    client.json("POST", "/v1/credential/_update", [{"password": password}, session_token], authenticated=False)
    # Fresh Kanidm 1.11.2 enforces MFA for all persons. Satisfy that policy with
    # a provider-generated job-only TOTP instead of weakening its security settings.
    status = client.json("POST", "/v1/credential/_update", ["totpgenerate", session_token], authenticated=False)
    registration = status.get("mfaregstate")
    require(isinstance(registration, dict) and set(registration) == {"TotpCheck"},
            "provider did not offer the expected TOTP enrollment state")
    totp = registration["TotpCheck"]
    require(isinstance(totp, dict), "provider did not issue a disposable TOTP enrollment challenge")
    code = totp_code(totp)
    status = client.json("POST", "/v1/credential/_update", [{"totpverify": [int(code), "Tabula disposable acceptance"]}, session_token], authenticated=False)
    require(status.get("can_commit") is True, "test person credential did not satisfy upstream policy")
    client.json("POST", "/v1/credential/_commit", session_token, authenticated=False)
    client.json("POST", "/v1/oauth2/_basic", {"attrs": {
        "name": [CLIENT_ID], "displayname": ["Tabula disposable OIDC acceptance"],
        "oauth2_rs_origin_landing": ["https://app.localhost:8444"], "oauth2_strict_redirect_uri": ["true"],
    }})
    client.json("POST", "/v1/oauth2/" + CLIENT_ID + "/_attr/oauth2_rs_origin", [CALLBACK])
    client.json("POST", "/v1/oauth2/" + CLIENT_ID + "/_scopemap/" + GROUP, ["openid"])
    secret = client.json("GET", "/v1/oauth2/" + CLIENT_ID + "/_basic_secret")
    require(isinstance(secret, str) and bool(secret), "test client secret was not issued")
    person = client.json("GET", "/v1/person/" + USERNAME)
    subject = person["attrs"]["uuid"][0]
    issuer = ORIGIN + "/oauth2/openid/" + CLIENT_ID
    discovery = client.json("GET", issuer + "/.well-known/openid-configuration", authenticated=False)
    require(discovery.get("issuer") == issuer, "upstream issuer differs from configured issuer")
    require(discovery.get("authorization_endpoint") == ORIGIN + "/ui/oauth2", "upstream authorization endpoint changed")
    require(discovery.get("token_endpoint") == ORIGIN + "/oauth2/token", "upstream token endpoint changed")
    require(discovery.get("jwks_uri") == issuer + "/public_key.jwk", "upstream JWKS endpoint changed")
    require("ES256" in discovery.get("id_token_signing_alg_values_supported", []), "upstream ES256 is unavailable")
    require("S256" in discovery.get("code_challenge_methods_supported", []), "upstream S256 is unavailable")
    require("client_secret_basic" in discovery.get("token_endpoint_auth_methods_supported", []), "upstream basic client authentication is unavailable")
    keys = client.json("GET", discovery["jwks_uri"], authenticated=False)
    require(bool(keys.get("keys")), "upstream JWKS was empty")
    private_json(args.config, {"provider_origin": ORIGIN, "issuer": issuer, "client_id": CLIENT_ID,
                              "client_secret": secret, "callback_url": CALLBACK, "ca_path": str(args.ca.resolve()),
                              "admitted_subject": subject, "username": USERNAME, "password": password, "totp": totp})
    # These files contain public endpoints/keys only, not credentials or person data.
    (args.artifacts / "provider-discovery.json").write_text(json.dumps(discovery, indent=2) + "\n")
    (args.artifacts / "provider-jwks.json").write_text(json.dumps(keys, indent=2) + "\n")
    print("Disposable provider bootstrap, discovery, and JWKS passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    setup = commands.add_parser("bootstrap")
    setup.add_argument("--container", required=True)
    setup.add_argument("--ca", type=Path, required=True)
    setup.add_argument("--config", type=Path, required=True)
    setup.add_argument("--artifacts", type=Path, required=True)
    auth = commands.add_parser("authorize")
    auth.add_argument("--config", type=Path, required=True)
    wait = commands.add_parser("ready")
    wait.add_argument("--ca", type=Path, required=True)
    wait.add_argument("--container", required=True)
    wait.add_argument("--artifacts", type=Path, required=True)
    diagnose = commands.add_parser("diagnose-config")
    diagnose.add_argument("--input", type=Path, required=True)
    diagnose.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "bootstrap":
            bootstrap(args)
        elif args.command == "authorize":
            request = json.loads(sys.stdin.buffer.read(16 * 1024 + 1))
            require(set(request) == {"authorization_url"}, "unexpected helper request")
            result = authorize(read_config(args.config), request["authorization_url"],
                               args.config.with_name("consent-observed.json"))
            print(json.dumps(result))
        elif args.command == "ready":
            wait_for_provider(args.ca, args.container, args.artifacts)
        else:
            with args.input.open("r", errors="replace") as stream:
                raw = stream.read(128 * 1024)
            public_diagnostic(args.artifacts / "provider-startup.json", {
                "status": "failed", "stage": "config_validation",
                "known_startup_categories": startup_categories(raw),
            })
            print("Provider config validation failed; see sanitized startup evidence", file=sys.stderr)
    except HarnessError as error:
        print("Disposable Kanidm acceptance support failed: " + str(error), file=sys.stderr)
        return 1
    except (ValueError, KeyError, OSError, subprocess.SubprocessError, TypeError):
        # Upstream bodies, cookies, subprocess output, passwords, URLs and tokens
        # are deliberately excluded even when setup or authentication fails.
        print("Disposable Kanidm acceptance support failed (details redacted)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
