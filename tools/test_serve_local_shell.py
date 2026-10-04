"""Real loopback HTTP boundary tests; these do not execute a browser."""

import functools
from http.server import ThreadingHTTPServer
import importlib.util
from pathlib import Path
import tempfile
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import urlopen

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
                    for route in ("/", "/games", "/games/example?setup=1"):
                        with urlopen(origin + route) as response:
                            self.assertEqual(response.read(), b"shell")
                            self.assertEqual(response.headers["Cache-Control"], "no-store")
                    with urlopen(origin + "/play/local/?source=tabula") as response:
                        self.assertEqual(response.read(), b"game document")
                    with urlopen(origin + "/play/local/game.wasm") as response:
                        self.assertEqual(response.read(), b"\0asm")
                    for route in ("/play/local/missing.wasm", "/play/local/missing.js", "/login", "/games/example/extra"):
                        with self.assertRaises(HTTPError) as raised:
                            urlopen(origin + route)
                        self.assertEqual(raised.exception.code, 404)
                finally:
                    server.shutdown()
                    thread.join()

    def test_route_domain_is_bounded(self):
        for path in ("/games", "/games/id?setup=1", "/"):
            self.assertTrue(module.shell_route(path))
        for path in ("/play/local", "/games/id/extra", "/games/id%2Fextra", "/app.wasm"):
            self.assertFalse(module.shell_route(path))


if __name__ == "__main__":
    unittest.main()
