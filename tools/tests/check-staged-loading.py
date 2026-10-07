#!/usr/bin/env python3
"""Exercise the real staged HTTP/SRI/manifest boundary, without a browser.

This fetches the explicitly declared startup resources. It does not execute
CSS font selection, Macroquad, browser storage, or a browser request waterfall.
"""

import argparse
import base64
import functools
import hashlib
from html.parser import HTMLParser
from http.server import ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import threading
import tomllib
from urllib.parse import urljoin
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("local_shell", ROOT / "tools/serve-local-shell.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class References(HTMLParser):
    def __init__(self):
        super().__init__()
        self.files = []
        self.images = []

    def handle_starttag(self, tag, attributes):
        fields = dict(attributes)
        if tag == "script" and fields.get("src"):
            self.files.append((fields["src"], fields.get("integrity")))
        if tag == "link" and fields.get("rel") == "stylesheet":
            self.files.append((fields["href"], fields.get("integrity")))
        if tag == "img":
            self.images.append(fields.get("src"))


def check(distribution):
    handler = functools.partial(module.LocalShellHandler, directory=str(distribution))
    with ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        origin = f"http://127.0.0.1:{server.server_port}"
        entry = origin + "/play/local/"
        requests = []

        def get(url):
            with urlopen(url) as response:
                payload = response.read()
                requests.append({"path": url.removeprefix(origin), "bytes": len(payload), "cache_control": response.headers["Cache-Control"]})
                return payload

        try:
            document = get(entry)
            assert requests[-1]["cache_control"] == "no-store"
            references = References()
            references.feed(document.decode())
            assert not references.images, "first-board loader must not fetch cover artwork"
            assert len(references.files) == 9, "expected two styles and seven pinned scripts, including host bridge and direct transport"
            manifest = None
            for relative, integrity in references.files:
                assert relative.startswith("resources/"), f"mutable host dependency: {relative}"
                payload = get(urljoin(entry, relative))
                assert requests[-1]["cache_control"] == "public, max-age=31536000, immutable"
                expected = "sha256-" + base64.b64encode(hashlib.sha256(payload).digest()).decode()
                assert integrity == expected, f"SRI mismatch: {relative}"
                if payload.startswith(b"window.TabulaResourceManifest="):
                    manifest = json.loads(payload.decode().removeprefix("window.TabulaResourceManifest=").removesuffix(";\n"))
            pack = tomllib.loads((ROOT / "games/chess/assets/fixture.pack.toml").read_text())
            runtime_aliases = {"tabula-game-client.wasm", "assets/OpenSans-Regular.ttf", "assets/OpenSans-Semibold.ttf", "assets/NotoSerif-Bold.ttf"}
            expected_aliases = runtime_aliases | {file["path"] for file in pack["files"]}
            assert manifest and manifest["schema"] == 1
            assert set(manifest["files"]) == expected_aliases, "staged runtime payloads must match the declared pack and fonts"
            critical = [file for file in pack["files"] if file["priority"] == "critical" and file["density"] == 1]
            assert len(critical) == 1
            aliases = ["tabula-game-client.wasm", "assets/OpenSans-Regular.ttf", "assets/OpenSans-Semibold.ttf", "assets/NotoSerif-Bold.ttf", critical[0]["path"]]
            for alias in aliases:
                resource = manifest["files"][alias]
                payload = get(urljoin(entry, resource["url"]))
                assert len(payload) == resource["bytes"] and hashlib.sha256(payload).hexdigest() == resource["sha256"]
                assert requests[-1]["cache_control"] == "public, max-age=31536000, immutable"
            assert len(requests) == 15
            return {
                "evidence": "real staged HTTP/SRI/manifest smoke, not a browser waterfall or cache timing",
                "first_board_runtime_aliases": aliases,
                "explicit_request_count_including_static_host": len(requests),
                "body_bytes": sum(request["bytes"] for request in requests),
                "requests": requests,
            }
        finally:
            server.shutdown()
            thread.join()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shell-dist", type=Path, required=True)
    parser.add_argument("--write-receipt", type=Path)
    args = parser.parse_args()
    result = check(args.shell_dist)
    encoded = json.dumps(result, indent=2) + "\n"
    if args.write_receipt:
        args.write_receipt.write_text(encoded)
    print(encoded, end="")
    print("PASS: real staged HTTP resource wiring")


if __name__ == "__main__":
    main()
