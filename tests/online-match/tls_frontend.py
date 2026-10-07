#!/usr/bin/env python3
"""CI-only, disposable HTTPS frontend for isolated online-match acceptance.

This is fixture infrastructure, not a production listener or an auth authority
(ADR-0031). The caller supplies a certificate/key and configures verification
inside its disposable browser profile. This helper neither creates certificates,
installs trust, nor disables verification. Request data is never logged.
"""

import argparse
import http.client
import mimetypes
import os
from pathlib import Path
import re
import signal
import ssl
import sys
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import unquote, urlsplit


LOOPBACK = "127.0.0.1"
REQUEST_BODY_LIMIT = 1024 * 1024
RESPONSE_BODY_LIMIT = 16 * 1024 * 1024
STATIC_FILE_LIMIT = 128 * 1024 * 1024
IO_TIMEOUT = 15
COPY_CHUNK = 64 * 1024
PROXY_PREFIXES = ("/api/", "/__fixture/")
HOP_BY_HOP = frozenset({
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
    "proxy-connection", "te", "trailer", "transfer-encoding", "upgrade",
})
HEADER_NAME = re.compile(r"^[!#$%&'*+.^_`|~0-9A-Za-z-]+$")
BAD_ESCAPE = re.compile(r"%(?![0-9A-Fa-f]{2})")
ENCODED_SEPARATOR = re.compile(r"%(?:2f|5c)", re.IGNORECASE)
CONTENT_TYPES = {
    ".wasm": "application/wasm", ".js": "text/javascript",
    ".mjs": "text/javascript", ".html": "text/html; charset=utf-8",
    ".css": "text/css; charset=utf-8",
}


class RejectedRequest(Exception):
    """Carry only a fixed HTTP status, never request or credential text."""

    def __init__(self, status):
        super().__init__("Request rejected")
        self.status = status


class StopFrontend(Exception):
    """Unwind the server context on SIGINT/SIGTERM without deadlocking shutdown."""


def parse_target(target):
    """Accept a bounded origin-form target; return its unambiguous decoded path."""
    if (not target.startswith("/") or target.startswith("//")
            or len(target) > 8192 or "#" in target
            or any(ord(char) < 33 or ord(char) == 127 for char in target)):
        raise RejectedRequest(400)
    try:
        parsed = urlsplit(target)
        if parsed.scheme or parsed.netloc or BAD_ESCAPE.search(parsed.path):
            raise RejectedRequest(400)
        if ENCODED_SEPARATOR.search(parsed.path):
            raise RejectedRequest(400)
        path = unquote(parsed.path, encoding="utf-8", errors="strict")
    except (UnicodeError, ValueError):
        raise RejectedRequest(400) from None
    if ("\\" in path or "//" in path
            or any(ord(char) < 32 or ord(char) == 127 for char in path)
            or any(part in (".", "..") for part in path.split("/"))):
        raise RejectedRequest(400)
    return path


def is_proxy_target(target):
    """Route only the literal API/fixture prefixes, preserving the whole target."""
    parse_target(target)
    return urlsplit(target).path.startswith(PROXY_PREFIXES)


def filtered_headers(headers):
    """Remove hop-by-hop/framing headers without rewriting end-to-end values.

    A list rather than a dict preserves duplicate Cookie/Set-Cookie fields. In
    particular, Origin, CSRF and scoped grant headers remain opaque to this edge.
    """
    headers = list(headers)
    excluded = set(HOP_BY_HOP) | {"content-length"}
    for name, value in headers:
        if (not HEADER_NAME.fullmatch(name)
                or any(ord(char) < 32 and char != "\t" or ord(char) == 127
                       for char in value)):
            raise RejectedRequest(400)
        if name.lower() == "connection":
            excluded.update(token.strip().lower() for token in value.split(","))
    return [(name, value) for name, value in headers if name.lower() not in excluded]


def content_length(headers, limit, failure_status):
    """Reject ambiguous lengths before reading or forwarding a bounded body."""
    values = [value for name, value in headers if name.lower() == "content-length"]
    if not values:
        return None
    if len(values) != 1 or not re.fullmatch(r"[0-9]{1,10}", values[0]):
        raise RejectedRequest(failure_status)
    length = int(values[0])
    if length > limit:
        raise RejectedRequest(failure_status)
    return length


def request_body_length(headers):
    """Chunked requests are deliberately unsupported; the fixture uses Fetch."""
    headers = list(headers)
    if any(name.lower() == "transfer-encoding" for name, _ in headers):
        raise RejectedRequest(400)
    length = content_length(headers, 9999999999, 400) or 0
    if length > REQUEST_BODY_LIMIT:
        raise RejectedRequest(413)
    return length


def read_request_body(headers, stream):
    """Read exactly one bounded request, rejecting a truncated body."""
    length = request_body_length(headers)
    body = stream.read(length) if length else b""
    if len(body) != length:
        raise RejectedRequest(400)
    return body


def wants_html(headers):
    """SPA fallback requires an explicit, nonzero text/html Accept value."""
    for name, value in headers:
        if name.lower() != "accept":
            continue
        for entry in value.split(","):
            fields = [field.strip().lower() for field in entry.split(";")]
            if fields[0] != "text/html":
                continue
            quality = 1.0
            try:
                for field in fields[1:]:
                    if field.startswith("q="):
                        quality = float(field[2:])
            except ValueError:
                continue
            if 0 < quality <= 1:
                return True
    return False


def static_file(root, target, headers):
    """Resolve files/indexes inside dist, or an explicitly requested HTML route."""
    path = parse_target(target)
    parts = path.lstrip("/").split("/")
    if parts[0] in ("api", "__fixture", "ws") or any(part.startswith(".") for part in parts):
        raise RejectedRequest(404)
    try:
        candidate = (root / path.lstrip("/")).resolve()
        if not candidate.is_relative_to(root):
            raise RejectedRequest(403)
        if candidate.is_dir():
            candidate = (candidate / "index.html").resolve()
        if not candidate.is_relative_to(root):
            raise RejectedRequest(403)
        if candidate.is_file():
            return candidate
        # Registry IDs use reverse-domain spelling. /games/com.vendor.game is
        # an HTML shell route even though its final component contains dots.
        segments = path.strip("/").split("/")
        game_route = len(segments) == 2 and segments[0] == "games"
        if (game_route or not Path(path.rstrip("/")).suffix) and wants_html(headers):
            return root / "index.html"
    except (OSError, RuntimeError):
        raise RejectedRequest(404) from None
    raise RejectedRequest(404)


def read_upstream_body(response, method):
    """Buffer only a bounded fixture reply so HTTPS framing is exact."""
    if not 200 <= response.status <= 599:
        raise RejectedRequest(502)
    headers = list(response.getheaders())
    length = content_length(headers, RESPONSE_BODY_LIMIT, 502)
    if method == "HEAD" or response.status in (204, 304):
        return headers, b"", length
    body = response.read(RESPONSE_BODY_LIMIT + 1)
    if len(body) > RESPONSE_BODY_LIMIT or length is not None and len(body) != length:
        raise RejectedRequest(502)
    return headers, body, len(body)


class FixtureServer(ThreadingHTTPServer):
    """A loopback-only disposable threaded listener, with no exception/access log."""

    daemon_threads = True
    block_on_close = False

    def handle_error(self, request, client_address):
        # The stdlib default prints a traceback, which can include request data.
        pass


class FixtureHandler(BaseHTTPRequestHandler):
    """Serve static assets or forward opaque fixture traffic with bounded framing."""

    protocol_version = "HTTP/1.1"

    def setup(self):
        super().setup()
        self.connection.settimeout(IO_TIMEOUT)

    def log_message(self, format, *args):
        pass

    def send_error(self, code, message=None, explain=None):
        # Ignore arbitrary stdlib/parser/exception explanations. No request echo.
        status = code if code in HTTPStatus._value2member_map_ else 500
        body = (HTTPStatus(status).phrase + "\n").encode("ascii")
        self._reply(status, [("Content-Type", "text/plain; charset=utf-8")], body)

    def handle_expect_100(self):
        try:
            request_body_length(self.headers.raw_items())
        except RejectedRequest as error:
            self.send_error(error.status)
            return False
        self.send_response_only(100)
        self.end_headers()
        return True

    def _reply(self, status, headers, body, representation_length=None):
        clean = filtered_headers(headers)
        self.close_connection = True
        self._response_started = True
        self.send_response_only(status)
        for name, value in clean:
            self.send_header(name, value)
        if status not in (204, 304):
            length = len(body) if representation_length is None else representation_length
            self.send_header("Content-Length", str(length))
        self.send_header("Connection", "close")
        self.end_headers()
        if self.command != "HEAD" and body:
            self.wfile.write(body)

    def _proxy(self, headers):
        clean = filtered_headers(headers)
        body = read_request_body(headers, self.rfile)
        upstream_port = self._upstream_port()
        connection = http.client.HTTPConnection(LOOPBACK, upstream_port, timeout=IO_TIMEOUT)
        try:
            connection.putrequest(self.command, self.path, skip_host=True, skip_accept_encoding=True)
            if not any(name.lower() == "host" for name, _ in clean):
                connection.putheader("Host", f"{LOOPBACK}:{upstream_port}")
            for name, value in clean:
                connection.putheader(name, value)
            connection.putheader("Content-Length", str(len(body)))
            connection.putheader("Connection", "close")
            connection.endheaders(body)
            response = connection.getresponse()
            upstream_headers, response_body, length = read_upstream_body(response, self.command)
            try:
                filtered_headers(upstream_headers)
            except RejectedRequest:
                raise RejectedRequest(502) from None
            self._reply(response.status, upstream_headers, response_body, length)
        finally:
            connection.close()

    def _upstream_port(self):
        """One request-local upstream; subclasses may choose a fixed route owner."""
        return self.server.upstream_port

    def _serve_static(self, headers):
        if self.command not in ("GET", "HEAD"):
            raise RejectedRequest(405)
        if request_body_length(headers):
            raise RejectedRequest(400)
        file = static_file(self.server.dist, self.path, headers)
        with file.open("rb") as source:
            length = os.fstat(source.fileno()).st_size
            if length > STATIC_FILE_LIMIT:
                raise RejectedRequest(413)
            content_type = CONTENT_TYPES.get(file.suffix.lower()) or mimetypes.guess_type(str(file))[0] or "application/octet-stream"
            response_headers = [("Content-Type", content_type), ("Cache-Control", "no-store")]
            if content_type.startswith("text/html"):
                # Document-only: preserve exact Origin on same-origin non-CORS
                # POSTs, while suppressing cross-origin referrers. Never rewrite
                # the protected upstream/API no-referrer policy in _proxy.
                response_headers.append(("Referrer-Policy", "same-origin"))
            self._reply(200, response_headers, b"", length)
            if self.command == "HEAD":
                return
            remaining = length
            while remaining:
                chunk = source.read(min(remaining, COPY_CHUNK))
                if not chunk:
                    return  # Connection closes; never frame a second response.
                self.wfile.write(chunk)
                remaining -= len(chunk)

    def _dispatch(self):
        self._response_started = False
        try:
            headers = list(self.headers.raw_items())
            request_body_length(headers)
            if is_proxy_target(self.path):
                self._proxy(headers)
            else:
                self._serve_static(headers)
        except RejectedRequest as error:
            if not self._response_started:
                self.send_error(error.status)
        except (OSError, http.client.HTTPException):
            if not self._response_started:
                self.send_error(502)

    do_GET = do_HEAD = do_POST = do_PUT = do_PATCH = do_DELETE = do_OPTIONS = _dispatch


def validate_inputs(dist, cert, key):
    """Fail before binding if the shell assets or supplied TLS inputs are absent."""
    root = Path(dist).resolve(strict=True)
    if not root.is_dir() or not (root / "index.html").is_file():
        raise ValueError("Missing frontend assets")
    if not (root / "index.html").resolve().is_relative_to(root):
        raise ValueError("Invalid frontend assets")
    certificate = Path(cert).resolve(strict=True)
    private_key = Path(key).resolve(strict=True)
    if not certificate.is_file() or not private_key.is_file():
        raise ValueError("Missing TLS inputs")
    if private_key.is_relative_to(root):
        raise ValueError("TLS key must be outside public assets")
    return root, certificate, private_key


def stop_frontend(signum, frame):
    raise StopFrontend()


def serve(dist, cert, key, listen_port, upstream_port):
    """Run only in CI; signal unwinding always closes the listener and TLS socket."""
    if os.environ.get("CI", "").lower() not in ("true", "1"):
        raise ValueError("CI required")
    root, certificate, private_key = validate_inputs(dist, cert, key)
    if not 1 <= listen_port <= 65535 or not 1 <= upstream_port <= 65535 or listen_port == upstream_port:
        raise ValueError("Invalid fixture ports")
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(str(certificate), str(private_key))
    with FixtureServer((LOOPBACK, listen_port), FixtureHandler) as server:
        server.dist = root
        server.upstream_port = upstream_port
        server.socket = context.wrap_socket(server.socket, server_side=True)
        previous = {}
        try:
            for number in (signal.SIGINT, signal.SIGTERM):
                previous[number] = signal.signal(number, stop_frontend)
            server.serve_forever(poll_interval=0.2)
        except StopFrontend:
            pass
        finally:
            for number, handler in previous.items():
                signal.signal(number, handler)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", required=True, type=Path)
    parser.add_argument("--cert", required=True, type=Path)
    parser.add_argument("--key", required=True, type=Path)
    parser.add_argument("--listen-port", default=9443, type=int)
    parser.add_argument("--upstream-port", default=3000, type=int)
    args = parser.parse_args(argv)
    try:
        serve(args.dist, args.cert, args.key, args.listen_port, args.upstream_port)
    except (OSError, ValueError, ssl.SSLError):
        print("TLS frontend startup failed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
