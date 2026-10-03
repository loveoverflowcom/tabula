"""HTTP wrapper negatives; Rust separately tests fixture authority semantics."""
import importlib.util
import json
import threading
import unittest
import urllib.error
import urllib.request
from functools import partial
from http.server import ThreadingHTTPServer
from pathlib import Path
from tempfile import TemporaryDirectory

spec = importlib.util.spec_from_file_location("spike_serve", Path(__file__).with_name("serve.py"))
serve = importlib.util.module_from_spec(spec)
spec.loader.exec_module(serve)


class EchoTransport:
    def __init__(self):
        self.calls = []

    def request(self, value):
        self.calls.append(value)
        return {"ok": True}


class WrapperTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = TemporaryDirectory()
        cls.transport = EchoTransport()
        # Bind ephemeral port before deriving the exact origin enforced by Handler.
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), serve.Handler, bind_and_activate=True)
        cls.origin = f"http://127.0.0.1:{cls.server.server_port}"
        cls.server.RequestHandlerClass = partial(
            serve.Handler, authority=cls.transport, origin=cls.origin,
            directory=cls.directory.name,
        )
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)
        cls.directory.cleanup()

    def request(self, *, path="/authority", data=b'{"op":"init"}', origin=None,
                content_type="application/json", host=None):
        headers = {"Origin": origin if origin is not None else self.origin,
                   "Content-Type": content_type}
        if host:
            headers["Host"] = host
        request = urllib.request.Request(self.origin + path, data=data, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(request, timeout=3) as response:
                return response.status, json.loads(response.read())
        except urllib.error.HTTPError as error:
            return error.code, None

    def test_exact_origin_roundtrip(self):
        self.assertEqual(self.request(), (200, {"ok": True}))
        self.assertEqual(self.transport.calls[-1], {"op": "init"})

    def test_wrong_origin_never_reaches_authority(self):
        before = len(self.transport.calls)
        self.assertEqual(self.request(origin="https://attacker.invalid")[0], 403)
        self.assertEqual(len(self.transport.calls), before)

    def test_wrong_host_rejected(self):
        self.assertEqual(self.request(host="attacker.invalid")[0], 403)

    def test_non_json_rejected(self):
        self.assertEqual(self.request(content_type="text/plain")[0], 415)

    def test_malformed_json_rejected(self):
        self.assertEqual(self.request(data=b"{")[0], 400)

    def test_array_not_authority_request(self):
        self.assertEqual(self.request(data=b"[]")[0], 400)

    def test_non_finite_number_rejected(self):
        self.assertEqual(self.request(data=b'{"value":NaN}')[0], 400)

    def test_oversize_request_rejected(self):
        self.assertEqual(self.request(data=b" " * (serve.MAX_BODY + 1))[0], 413)

    def test_unknown_route_and_query_rejected(self):
        self.assertEqual(self.request(path="/other")[0], 404)
        self.assertEqual(self.request(path="/authority?token=forbidden")[0], 404)


if __name__ == "__main__":
    unittest.main()
