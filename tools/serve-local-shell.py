#!/usr/bin/env python3
"""Serve the built shell and separate local play document for development.

Only implemented Leptos discovery routes receive SPA fallback. Missing gameplay
artifacts return HTTP 404, never a misleading successful shell response.
This is a loopback development server, not deployment infrastructure.
"""

import argparse
import functools
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit


def shell_route(path):
    """The bounded routes actually implemented by apps/web (ADR-0030)."""
    path = unquote(urlsplit(path).path)
    return path in ("/", "/games", "/games/") or (
        path.startswith("/games/") and len(path.split("/")) == 3
    )


class LocalShellHandler(SimpleHTTPRequestHandler):
    """Serve static runtime files before falling back to implemented shell paths."""

    def translate_path(self, path):
        resolved = super().translate_path(path)
        if not Path(resolved).exists() and shell_route(path):
            return str(Path(self.directory) / "index.html")
        return resolved

    def end_headers(self):
        # Only a verified full-content name is immutable. Entries, fixed-name
        # diagnostic copies and all errors must see the current build.
        self.send_header("Cache-Control", getattr(self, "_cache_control", "no-store"))
        if getattr(self, "_resource_etag", None):
            self.send_header("ETag", self._resource_etag)
        super().end_headers()

    def send_head(self):
        self._cache_control = "no-store"
        self._resource_etag = None
        parsed = urlsplit(self.path)
        match = re.fullmatch(r"(?:/play/local)?/resources/([0-9a-f]{64})\.(?:wasm|js|css|png|ttf)", parsed.path)
        file = Path(self.translate_path(self.path))
        if match and not parsed.query and file.is_file():
            # Refuse a corrupt immutable object instead of caching it for a
            # year. This local server does not claim CDN/deployment behavior.
            if file.stat().st_size > 64 * 1024 * 1024 or hashlib.sha256(file.read_bytes()).hexdigest() != match[1]:
                self.send_error(503, "Immutable resource content mismatch; rebuild local play")
                return None
            self._cache_control = "public, max-age=31536000, immutable"
            self._resource_etag = f'"sha256-{match[1]}"'
            validators = [value.strip().removeprefix("W/") for value in self.headers.get("If-None-Match", "").split(",")]
            if "*" in validators or self._resource_etag in validators:
                self.send_response(304)
                self.end_headers()
                return None
        return super().send_head()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8000)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    distribution = root / "apps/web/dist"
    for relative in ("index.html", "play/local/index.html", "play/local/tabula-game-client.wasm"):
        if not (distribution / relative).is_file():
            parser.error(f"missing {distribution / relative}; run just web-local-build first")
    handler = functools.partial(LocalShellHandler, directory=str(distribution))
    with ThreadingHTTPServer(("127.0.0.1", args.port), handler) as server:
        print(f"Tabula local discovery/play: http://127.0.0.1:{args.port}", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
