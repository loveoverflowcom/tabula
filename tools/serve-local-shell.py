#!/usr/bin/env python3
"""Serve the built shell and separate local play document for development.

Only implemented Leptos discovery routes receive SPA fallback. Missing gameplay
artifacts return HTTP 404, never a misleading successful shell response.
This is a loopback development server, not deployment infrastructure.
"""

import argparse
import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
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
        # Development builds replace independently bundled files in place.
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


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
