#!/usr/bin/env python3
"""Loopback-only isolated fixture host; production networking remains gated.

The native Rust tool owns fixture authority. This process only serializes its
JSON-lines input/output and serves staged, public prototype assets.
"""
import argparse
import json
import subprocess
import threading
import time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

MAX_BODY = 65536


class Authority:
    def __init__(self, executable):
        self.process = subprocess.Popen(
            [str(executable), "--stdio"], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, text=True, bufsize=1,
        )
        self.lock = threading.Lock()

    def request(self, value):
        with self.lock:
            if self.process.poll() is not None:
                raise RuntimeError("authority stopped")
            self.process.stdin.write(json.dumps(value, allow_nan=False) + "\n")
            self.process.stdin.flush()
            # Rust processes bounded synchronous fixture inputs. No remote I/O.
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError("authority returned no response")
            return json.loads(line)

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, authority, origin, **kwargs):
        self.authority = authority
        self.origin = origin
        super().__init__(*args, **kwargs)

    def list_directory(self, path):
        self.send_error(403, "Directory listing disabled")
        return None

    def end_headers(self):
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        super().end_headers()

    def do_GET(self):
        if self.path == "/favicon.ico":
            self.send_response(204)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        super().do_GET()

    def do_POST(self):
        if urlsplit(self.path).path != "/authority" or urlsplit(self.path).query:
            self.send_error(404)
            return
        if self.headers.get("Origin") != self.origin:
            self.send_error(403, "Exact same origin required")
            return
        if self.headers.get("Host") != urlsplit(self.origin).netloc:
            self.send_error(403, "Exact host required")
            return
        if self.headers.get_content_type() != "application/json":
            self.send_error(415, "JSON required")
            return
        try:
            length = int(self.headers.get("Content-Length", "-1"))
            if not 0 < length <= MAX_BODY:
                self.send_error(413, "Bounded body required")
                return
            def reject_constant(_):
                raise ValueError("non-finite JSON")
            value = json.loads(self.rfile.read(length), parse_constant=reject_constant)
            if not isinstance(value, dict):
                raise ValueError("object required")
        except (ValueError, UnicodeDecodeError):
            self.send_error(400, "Malformed request")
            return
        started = time.perf_counter()
        try:
            result = self.authority.request(value)
        except (RuntimeError, OSError, ValueError):
            self.send_error(503, "Fixture authority unavailable")
            return
        payload = json.dumps(result, allow_nan=False).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(payload)
        print(json.dumps({"kind": "authority-request", "request_bytes": length,
                          "response_bytes": len(payload),
                          "wall_ms": (time.perf_counter() - started) * 1000}), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--authority", type=Path, required=True)
    parser.add_argument("--port", type=int, default=8060)
    args = parser.parse_args()
    directory = args.directory.resolve(strict=True)
    authority = Authority(args.authority.resolve(strict=True))
    origin = f"http://127.0.0.1:{args.port}"
    server = ThreadingHTTPServer(("127.0.0.1", args.port), partial(
        Handler, directory=str(directory), authority=authority, origin=origin,
    ))
    print(json.dumps({"kind": "host-ready", "origin": origin,
                      "scope": "isolated-local-fixture",
                      "authority_pid": authority.process.pid}), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        authority.close()


if __name__ == "__main__":
    main()
