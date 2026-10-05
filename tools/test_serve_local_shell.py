"""Real loopback HTTP boundary tests; these do not execute a browser."""

import functools
import hashlib
from http.server import ThreadingHTTPServer
import importlib.util
from pathlib import Path
import tempfile
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

spec = importlib.util.spec_from_file_location("local_shell", Path(__file__).with_name("serve-local-shell.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ServerTests(unittest.TestCase):
    def test_shell_fallback_and_missing_runtime_are_distinct(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "index.html").write_text("shell")
            (root / "play/local").mkdir(parents=True)
            (root / "play/local/index.html").write_text("game document")
            (root / "play/local/game.wasm").write_bytes(b"\0asm")
            handler = functools.partial(module.LocalShellHandler, directory=directory)
            with ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
                thread = threading.Thread(target=server.serve_forever, daemon=True)
                thread.start()
                origin = f"http://127.0.0.1:{server.server_port}"
                try:
                    for route in ("/", "/games", "/games/example?setup=1", "/account", "/me", "/login", "/register", "/friends", "/u/example"):
                        with urlopen(origin + route) as response:
                            self.assertEqual(response.read(), b"shell")
                            self.assertEqual(response.headers["Cache-Control"], "no-store")
                    with urlopen(origin + "/play/local/?source=tabula") as response:
                        self.assertEqual(response.read(), b"game document")
                    with urlopen(origin + "/play/local/game.wasm") as response:
                        self.assertEqual(response.read(), b"\0asm")
                    for route in ("/play/local/missing.wasm", "/play/local/missing.js", "/api/v1/auth/context", "/api/v1/me", "/games/example/extra", "/u/example/extra", "/account/extra"):
                        with self.assertRaises(HTTPError) as raised:
                            urlopen(origin + route)
                        self.assertEqual(raised.exception.code, 404)
                finally:
                    server.shutdown()
                    thread.join()

    def test_route_domain_is_bounded(self):
        for path in ("/games", "/games/id?setup=1", "/", "/account", "/me", "/login", "/register", "/friends", "/u/example"):
            self.assertTrue(module.shell_route(path))
        for path in ("/play/local", "/games/id/extra", "/games/id%2Fextra", "/app.wasm", "/api/v1/me", "/account/extra", "/u/", "/u/example%2Fextra", "/u/example/extra"):
            self.assertFalse(module.shell_route(path))

    def test_only_exact_verified_content_names_are_immutable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            resource_dir = root / "play/local/resources"
            resource_dir.mkdir(parents=True)
            payload = b"\0asm\x01\0\0\0"
            digest = hashlib.sha256(payload).hexdigest()
            resource = resource_dir / f"{digest}.wasm"
            resource.write_bytes(payload)
            (root / "index.html").write_text("current shell")
            handler = functools.partial(module.LocalShellHandler, directory=directory)
            with ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
                thread = threading.Thread(target=server.serve_forever, daemon=True)
                thread.start()
                origin = f"http://127.0.0.1:{server.server_port}"
                url = origin + f"/play/local/resources/{digest}.wasm"
                try:
                    with urlopen(url) as response:
                        self.assertEqual(response.read(), payload)
                        self.assertEqual(response.headers["Cache-Control"], "public, max-age=31536000, immutable")
                        self.assertEqual(response.headers["Content-Type"], "application/wasm")
                        etag = response.headers["ETag"]
                    with self.assertRaises(HTTPError) as unchanged:
                        urlopen(Request(url, headers={"If-None-Match": etag}))
                    self.assertEqual(unchanged.exception.code, 304)
                    self.assertEqual(unchanged.exception.headers["ETag"], etag)
                    with urlopen(url + "?diagnostic=1") as response:
                        self.assertEqual(response.headers["Cache-Control"], "no-store")
                    resource.write_bytes(b"corrupt!")
                    with self.assertRaises(HTTPError) as corrupt:
                        urlopen(url)
                    self.assertEqual(corrupt.exception.code, 503)
                    self.assertEqual(corrupt.exception.headers["Cache-Control"], "no-store")
                    with self.assertRaises(HTTPError) as missing:
                        urlopen(origin + "/play/local/resources/" + "0" * 64 + ".wasm")
                    self.assertEqual(missing.exception.code, 404)
                    self.assertEqual(missing.exception.headers["Cache-Control"], "no-store")
                    (root / "index.html").write_text("new shell and new resource names")
                    with urlopen(origin + "/games/example?setup=1") as response:
                        self.assertEqual(response.read(), b"new shell and new resource names")
                        self.assertEqual(response.headers["Cache-Control"], "no-store")
                finally:
                    server.shutdown()
                    thread.join()


if __name__ == "__main__":
    unittest.main()
