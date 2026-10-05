"""Offline fixture regressions only: no listener, browser, or TLS bypass is used."""

import importlib.util
import io
import os
from pathlib import Path
import signal
import ssl
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from email.message import Message
from types import SimpleNamespace
from unittest import mock


SPEC = importlib.util.spec_from_file_location("online_match_tls_frontend", Path(__file__).with_name("tls_frontend.py"))
frontend = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(frontend)


class MemoryHandler(frontend.FixtureHandler):
    """Exercise real handler decisions with memory streams and no socket/server."""

    def __init__(self, root, target="/", method="GET", headers=(), body=b""):
        self.server = SimpleNamespace(dist=root, upstream_port=3000)
        self.path = target
        self.command = method
        self.headers = Message()
        for name, value in headers:
            self.headers[name] = value
        self.rfile = io.BytesIO(body)
        self.wfile = io.BytesIO()
        self.status = None
        self.sent_headers = []
        self.responses = []
        self.close_connection = False

    def send_response_only(self, code, message=None):
        self.status = code
        self.responses.append(code)

    def send_header(self, name, value):
        self.sent_headers.append((name, value))

    def end_headers(self):
        pass

    def values(self, name):
        return [value for key, value in self.sent_headers if key.lower() == name.lower()]


class TlsFrontendTests(unittest.TestCase):
    def test_only_static_html_documents_get_same_origin_referrer_policy(self):
        (self.root / "client.js").write_text("synthetic-script")
        for path, expected in (("/", ["same-origin"]), ("/client.js", [])):
            handler = self.handler(path)
            handler._dispatch()
            self.assertEqual(handler.status, 200)
            self.assertEqual(handler.values("Referrer-Policy"), expected)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.private = Path(self.temporary.name).resolve()
        self.root = self.private / "dist"
        self.root.mkdir()
        (self.root / "index.html").write_bytes(b"<html>synthetic shell</html>")

    def handler(self, target="/", method="GET", headers=(), body=b""):
        return MemoryHandler(self.root, target, method, headers, body)

    def response(self, status=200, headers=(), body=b"{}"):
        response = mock.Mock()
        response.status = status
        response.getheaders.return_value = list(headers)
        response.read.return_value = body
        return response

    def assert_rejected(self, status, function, *args):
        with self.assertRaises(frontend.RejectedRequest) as error:
            function(*args)
        self.assertEqual(error.exception.status, status)
        self.assertEqual(str(error.exception), "Request rejected")

    def test_only_exact_proxy_prefixes_are_routed(self):
        for target in ("/api/v1/matches", "/api/?query=%2Fopaque", "/__fixture/session?query=1"):
            with self.subTest(target=target):
                self.assertTrue(frontend.is_proxy_target(target))
        for target in ("/api", "/api-other/v1", "/__fixture", "/__fixture-extra/session", "/ws", "/games/chess"):
            with self.subTest(target=target):
                self.assertFalse(frontend.is_proxy_target(target))

    def test_hostile_targets_are_rejected_before_proxy_or_static_resolution(self):
        targets = ("https://other.invalid/api/x", "//other.invalid/api/x", "/../secret",
                   "/a/%2e%2E/secret", "/a/./secret", "/api%2Fx", "/api/%5cx",
                   "/a\\secret", "/a//b", "/%00secret", "/%7Fsecret", "/%FF",
                   "/%Q0", "/%", "/file#fragment", "/file\r\nCookie: synthetic", "/" + "a" * 8192)
        for target in targets:
            with self.subTest(target=target):
                self.assert_rejected(400, frontend.parse_target, target)

    def test_encoded_names_and_opaque_query_are_supported(self):
        (self.root / "hello world.js").write_bytes(b"synthetic javascript")
        self.assertEqual(frontend.static_file(self.root, "/hello%20world.js?opaque=%00", []), self.root / "hello world.js")

    def test_hop_headers_and_connection_nominations_are_filtered_case_insensitively(self):
        headers = [("Connection", "Keep-Alive, X-Hop"), ("connection", "X-Other"),
                   ("Keep-Alive", "timeout=2"), ("X-Hop", "synthetic"), ("x-other", "synthetic"),
                   ("Transfer-Encoding", "chunked"), ("TE", "trailers"), ("Trailer", "x-end"),
                   ("Upgrade", "websocket"), ("Proxy-Authorization", "synthetic"),
                   ("Proxy-Authenticate", "synthetic"), ("Proxy-Connection", "close"),
                   ("Content-Length", "17"), ("Content-Type", "application/json")]
        self.assertEqual(frontend.filtered_headers(headers), [("Content-Type", "application/json")])

    def test_auth_headers_and_duplicate_cookies_are_opaque_and_preserved(self):
        headers = [("Cookie", "synthetic_session=one; synthetic_other=two"), ("Cookie", "synthetic_extra=three"),
                   ("Origin", "https://localhost:9443"), ("X-CSRF-Token", "synthetic-csrf"),
                   ("Authorization", "Bearer synthetic-grant"), ("X-Match-Grant", "synthetic-grant"),
                   ("Set-Cookie", "synthetic_one=one; Secure; HttpOnly; Path=/"),
                   ("Set-Cookie", "synthetic_two=two; Secure; HttpOnly; Path=/")]
        self.assertEqual(frontend.filtered_headers(headers), headers)

    def test_invalid_header_names_or_values_cannot_be_forwarded(self):
        for header in (("bad name", "synthetic"), ("X-Value", "synthetic\r\nCookie: synthetic"),
                       ("X-Value", "synthetic\x00"), ("X-Value", "synthetic\x7f")):
            with self.subTest(header=header):
                self.assert_rejected(400, frontend.filtered_headers, [header])

    def test_request_body_zero_exact_limit_and_truncation(self):
        self.assertEqual(frontend.read_request_body([], io.BytesIO()), b"")
        body = b"x" * frontend.REQUEST_BODY_LIMIT
        self.assertEqual(frontend.read_request_body([("Content-Length", str(len(body)))], io.BytesIO(body)), body)
        self.assert_rejected(400, frontend.read_request_body, [("Content-Length", "3")], io.BytesIO(b"xx"))

    def test_oversized_requests_fail_before_any_body_read(self):
        stream = mock.Mock()
        self.assert_rejected(413, frontend.read_request_body,
                             [("Content-Length", str(frontend.REQUEST_BODY_LIMIT + 1))], stream)
        stream.read.assert_not_called()

    def test_ambiguous_or_noncanonical_request_lengths_are_rejected(self):
        for headers in ([("Content-Length", "1"), ("content-length", "1")],
                        [("Content-Length", "1, 1")], [("Content-Length", "-1")],
                        [("Content-Length", "+1")], [("Content-Length", " 1")],
                        [("Content-Length", "")], [("Content-Length", "9" * 11)],
                        [("Transfer-Encoding", "chunked")], [("Transfer-Encoding", "")]):
            with self.subTest(headers=headers):
                self.assert_rejected(400, frontend.request_body_length, headers)

    def test_spa_fallback_requires_explicit_html_accept_and_extensionless_route(self):
        self.assertEqual(frontend.static_file(self.root, "/games/chess?mode=online", [("Accept", "text/html, */*;q=0.8")]),
                         self.root / "index.html")
        self.assertEqual(frontend.static_file(self.root, "/games/com.synthetic.game", [("Accept", "text/html")]),
                         self.root / "index.html")
        self.assert_rejected(404, frontend.static_file, self.root, "/games/com.synthetic.game", [])
        for headers in ([], [("Accept", "*/*")], [("Accept", "application/json")],
                        [("Accept", "text/html;q=0")], [("Accept", "text/html;q=bogus")],
                        [("Accept", "text/html;q=nan")]):
            with self.subTest(headers=headers):
                self.assert_rejected(404, frontend.static_file, self.root, "/games/chess", headers)
        for target in ("/missing.wasm", "/missing.js", "/missing.png", "/.env", "/nested/.private"):
            with self.subTest(target=target):
                self.assert_rejected(404, frontend.static_file, self.root, target, [("Accept", "text/html")])

    def test_reserved_roots_never_receive_spa_fallback(self):
        for target in ("/api", "/api/missing", "/__fixture", "/__fixture/missing", "/ws"):
            with self.subTest(target=target):
                self.assert_rejected(404, frontend.static_file, self.root, target, [("Accept", "text/html")])

    def test_directory_index_serves_the_actual_game_document(self):
        game = self.root / "play" / "synthetic-game"
        game.mkdir(parents=True)
        (game / "index.html").write_bytes(b"synthetic game document")
        self.assertEqual(frontend.static_file(self.root, "/play/synthetic-game/", []), game / "index.html")

    def test_symlink_escape_and_directory_index_escape_are_rejected(self):
        with tempfile.TemporaryDirectory() as outside:
            external = Path(outside)
            (external / "secret.js").write_bytes(b"synthetic outside data")
            (self.root / "linked.js").symlink_to(external / "secret.js")
            (self.root / "linked").symlink_to(external, target_is_directory=True)
            nested = self.root / "nested"
            nested.mkdir()
            (nested / "index.html").symlink_to(external / "secret.js")
            for target in ("/linked.js", "/linked/secret.js", "/nested/"):
                with self.subTest(target=target):
                    self.assert_rejected(403, frontend.static_file, self.root, target, [])

    def test_static_binary_mime_and_exact_length_are_sent(self):
        payload = b"\x00asm\x01\x00\x00\x00synthetic wasm"
        (self.root / "game.wasm").write_bytes(payload)
        handler = self.handler("/game.wasm")
        handler._dispatch()
        self.assertEqual(handler.status, 200)
        self.assertEqual(handler.values("Content-Type"), ["application/wasm"])
        self.assertEqual(handler.values("Content-Length"), [str(len(payload))])
        self.assertEqual(handler.wfile.getvalue(), payload)
        self.assertEqual(handler.values("Connection"), ["close"])
        self.assertTrue(handler.close_connection)

    def test_static_head_has_representation_length_and_no_body(self):
        handler = self.handler(method="HEAD")
        handler._dispatch()
        self.assertEqual(handler.status, 200)
        self.assertEqual(handler.values("Content-Length"), [str((self.root / "index.html").stat().st_size)])
        self.assertEqual(handler.wfile.getvalue(), b"")

    def test_large_static_files_are_rejected_before_a_success_response(self):
        handler = self.handler()
        with mock.patch.object(frontend, "STATIC_FILE_LIMIT", 2):
            handler._dispatch()
        self.assertEqual(handler.responses, [413])

    def test_static_post_and_static_get_body_are_rejected(self):
        handler = self.handler(method="POST")
        handler._dispatch()
        self.assertEqual(handler.status, 405)
        handler = self.handler(headers=[("Content-Length", "1")], body=b"x")
        handler._dispatch()
        self.assertEqual(handler.status, 400)

    def test_proxy_forwards_method_query_body_and_auth_headers_unchanged(self):
        body = b'{"synthetic_command":"move"}'
        headers = [("Host", "localhost:9443"), ("Cookie", "synthetic_session=one"),
                   ("Origin", "https://localhost:9443"), ("X-CSRF-Token", "synthetic-csrf"),
                   ("Authorization", "Bearer synthetic-grant"), ("Content-Type", "application/json"),
                   ("Content-Length", str(len(body))), ("Connection", "X-Hop"), ("X-Hop", "synthetic")]
        handler = self.handler("/api/v1/matches?opaque=%2F%25", "POST", headers, body)
        reply = self.response(headers=[("Content-Length", "2"), ("Content-Type", "application/json")])
        connection = mock.Mock()
        connection.getresponse.return_value = reply
        with mock.patch.object(frontend.http.client, "HTTPConnection", return_value=connection) as factory:
            handler._dispatch()
        factory.assert_called_once_with("127.0.0.1", 3000, timeout=frontend.IO_TIMEOUT)
        connection.putrequest.assert_called_once_with("POST", handler.path, skip_host=True, skip_accept_encoding=True)
        for name, value in headers[:6]:
            self.assertIn(mock.call(name, value), connection.putheader.call_args_list)
        self.assertNotIn(mock.call("X-Hop", "synthetic"), connection.putheader.call_args_list)
        self.assertEqual([call.args for call in connection.putheader.call_args_list if call.args[0].lower() == "content-length"],
                         [("Content-Length", str(len(body)))])
        connection.endheaders.assert_called_once_with(body)
        connection.getresponse.assert_called_once_with()
        connection.close.assert_called_once_with()
        self.assertEqual(handler.wfile.getvalue(), b"{}")
        self.assertEqual(handler.values("Content-Length"), ["2"])

    def test_proxy_keeps_duplicate_set_cookie_headers_and_removes_hop_headers(self):
        cookies = [("Set-Cookie", "synthetic_one=one; Secure; HttpOnly; SameSite=Strict"),
                   ("Set-Cookie", "synthetic_two=two; Secure; HttpOnly; SameSite=Strict")]
        reply = self.response(status=201, headers=cookies + [("Connection", "X-Hop"), ("X-Hop", "synthetic"),
                                                           ("Transfer-Encoding", "chunked")], body=b"synthetic response")
        connection = mock.Mock()
        connection.getresponse.return_value = reply
        handler = self.handler("/__fixture/session", "POST")
        with mock.patch.object(frontend.http.client, "HTTPConnection", return_value=connection):
            handler._dispatch()
        self.assertEqual(handler.status, 201)
        self.assertEqual(handler.values("Set-Cookie"), [value for _, value in cookies])
        self.assertEqual(handler.values("Content-Length"), [str(len(b"synthetic response"))])
        self.assertEqual(handler.values("Transfer-Encoding"), [])
        self.assertEqual(handler.values("X-Hop"), [])

    def test_proxy_failures_are_sanitized_and_connection_is_closed(self):
        connection = mock.Mock()
        connection.getresponse.side_effect = OSError("synthetic cookie=private grant=private body=private")
        handler = self.handler("/api/private?synthetic=private", headers=[("Cookie", "synthetic=private")])
        capture = io.StringIO()
        with mock.patch.object(frontend.http.client, "HTTPConnection", return_value=connection), redirect_stderr(capture), redirect_stdout(capture):
            handler._dispatch()
        self.assertEqual(handler.status, 502)
        self.assertEqual(handler.wfile.getvalue(), b"Bad Gateway\n")
        self.assertEqual(capture.getvalue(), "")
        connection.close.assert_called_once_with()

    def test_oversized_request_cannot_contact_upstream(self):
        handler = self.handler("/api/v1/matches", "POST", [("Content-Length", str(frontend.REQUEST_BODY_LIMIT + 1))])
        with mock.patch.object(frontend.http.client, "HTTPConnection") as factory:
            handler._dispatch()
        self.assertEqual(handler.status, 413)
        factory.assert_not_called()

    def test_upstream_body_bound_is_checked_with_and_without_declared_length(self):
        with mock.patch.object(frontend, "RESPONSE_BODY_LIMIT", 4):
            reply = self.response(headers=[("Content-Length", "5")], body=b"xxxxx")
            self.assert_rejected(502, frontend.read_upstream_body, reply, "GET")
            reply.read.assert_not_called()
            reply = self.response(body=b"xxxxx")
            self.assert_rejected(502, frontend.read_upstream_body, reply, "GET")
            reply.read.assert_called_once_with(5)
            reply = self.response(body=b"xxxx")
            self.assertEqual(frontend.read_upstream_body(reply, "GET"), ([], b"xxxx", 4))

    def test_upstream_truncation_duplicate_lengths_and_upgrade_are_rejected(self):
        for reply in (self.response(headers=[("Content-Length", "3")], body=b"xx"),
                      self.response(headers=[("Content-Length", "2"), ("content-length", "2")]),
                      self.response(status=101, body=b"")):
            self.assert_rejected(502, frontend.read_upstream_body, reply, "GET")

    def test_upstream_head_204_and_304_do_not_read_a_body(self):
        for status, method in ((200, "HEAD"), (204, "GET"), (304, "GET")):
            with self.subTest(status=status, method=method):
                reply = self.response(status=status, headers=[("Content-Length", "31")])
                self.assertEqual(frontend.read_upstream_body(reply, method), ([("Content-Length", "31")], b"", 31))
                reply.read.assert_not_called()
                handler = self.handler(method=method)
                handler._reply(status, [("Content-Length", "31")], b"", 31)
                self.assertEqual(handler.values("Content-Length"), ["31"] if status == 200 else [])
                self.assertEqual(handler.wfile.getvalue(), b"")

    def test_malformed_upstream_header_is_gateway_failure(self):
        connection = mock.Mock()
        connection.getresponse.return_value = self.response(headers=[("X-Invalid", "synthetic\r\nCookie: private")])
        handler = self.handler("/api/v1/matches")
        with mock.patch.object(frontend.http.client, "HTTPConnection", return_value=connection):
            handler._dispatch()
        self.assertEqual(handler.responses, [502])
        self.assertEqual(handler.wfile.getvalue(), b"Bad Gateway\n")

    def test_expect_continue_rejects_oversized_body_before_continue(self):
        handler = self.handler(headers=[("Content-Length", str(frontend.REQUEST_BODY_LIMIT + 1))])
        self.assertFalse(handler.handle_expect_100())
        self.assertEqual(handler.responses, [413])
        handler = self.handler(headers=[("Content-Length", "1")])
        self.assertTrue(handler.handle_expect_100())
        self.assertEqual(handler.responses, [100])

    def test_logging_and_error_responses_never_echo_arbitrary_request_data(self):
        handler = self.handler("/synthetic?grant=private", headers=[("Cookie", "synthetic=private")], body=b"synthetic private body")
        capture = io.StringIO()
        with redirect_stderr(capture), redirect_stdout(capture):
            handler.log_message("%s", "synthetic private cookie/grant/body")
            frontend.FixtureServer.handle_error(None, "synthetic private request", ("127.0.0.1", 1))
            handler.send_error(400, "synthetic private cookie/grant/body", "synthetic private explanation")
        self.assertEqual(capture.getvalue(), "")
        self.assertEqual(handler.wfile.getvalue(), b"Bad Request\n")

    def test_disconnected_static_client_cannot_get_a_second_response(self):
        handler = self.handler()
        handler.wfile = mock.Mock()
        handler.wfile.write.side_effect = BrokenPipeError("synthetic private request")
        handler._dispatch()
        self.assertEqual(handler.responses, [200])
        self.assertTrue(handler.close_connection)

    def test_inputs_require_assets_and_both_supplied_tls_files(self):
        certificate = self.private / "synthetic-cert.pem"
        key = self.private / "synthetic-key.pem"
        certificate.write_text("synthetic, not a certificate")
        key.write_text("synthetic, not a key")
        self.assertEqual(frontend.validate_inputs(self.root, certificate, key), (self.root, certificate, key))
        for dist, cert, private_key in ((self.root / "missing", certificate, key),
                                       (self.root, self.root / "missing", key),
                                       (self.root, certificate, self.root / "missing")):
            with self.subTest(dist=dist, cert=cert, key=private_key):
                with self.assertRaises((OSError, ValueError)):
                    frontend.validate_inputs(dist, cert, private_key)
        (self.root / "index.html").unlink()
        with self.assertRaises(ValueError):
            frontend.validate_inputs(self.root, certificate, key)

    def test_tls_private_key_cannot_be_inside_public_static_assets(self):
        certificate = self.private / "synthetic-cert.pem"
        key = self.root / "synthetic-key.pem"
        certificate.write_text("synthetic, not a certificate")
        key.write_text("synthetic, not a key")
        with self.assertRaises(ValueError):
            frontend.validate_inputs(self.root, certificate, key)

    def test_shell_index_symlink_outside_dist_is_invalid_at_startup(self):
        with tempfile.TemporaryDirectory() as outside:
            path = Path(outside) / "synthetic.html"
            path.write_text("synthetic")
            (self.root / "index.html").unlink()
            (self.root / "index.html").symlink_to(path)
            with self.assertRaises(ValueError):
                frontend.validate_inputs(self.root, path, path)

    def test_non_ci_startup_is_rejected_without_opening_a_listener(self):
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch.object(frontend, "FixtureServer") as server:
            with self.assertRaises(ValueError):
                frontend.serve(self.root, "synthetic-cert", "synthetic-key", 9443, 3000)
        server.assert_not_called()

    def test_ci_setup_uses_supplied_certificate_tls_server_context_and_loopback(self):
        certificate = self.root / "synthetic-cert.pem"
        key = self.root / "synthetic-key.pem"
        context = mock.Mock()
        listener = mock.MagicMock()
        listener.__enter__.return_value = listener
        listener.serve_forever.side_effect = frontend.StopFrontend()
        original_socket = listener.socket
        with mock.patch.dict(os.environ, {"CI": "true"}), \
             mock.patch.object(frontend, "validate_inputs", return_value=(self.root, certificate, key)), \
             mock.patch.object(frontend.ssl, "SSLContext", return_value=context) as context_factory, \
             mock.patch.object(frontend, "FixtureServer", return_value=listener) as server_factory, \
             mock.patch.object(frontend.signal, "signal", return_value="synthetic-old-handler") as signals:
            frontend.serve(self.root, certificate, key, 9443, 3000)
        context_factory.assert_called_once_with(ssl.PROTOCOL_TLS_SERVER)
        self.assertEqual(context.minimum_version, ssl.TLSVersion.TLSv1_2)
        context.load_cert_chain.assert_called_once_with(str(certificate), str(key))
        context.wrap_socket.assert_called_once_with(original_socket, server_side=True)
        server_factory.assert_called_once_with(("127.0.0.1", 9443), frontend.FixtureHandler)
        listener.serve_forever.assert_called_once_with(poll_interval=0.2)
        listener.__exit__.assert_called_once_with(None, None, None)
        self.assertEqual(signals.call_args_list, [mock.call(signal.SIGINT, frontend.stop_frontend),
                                                mock.call(signal.SIGTERM, frontend.stop_frontend),
                                                mock.call(signal.SIGINT, "synthetic-old-handler"),
                                                mock.call(signal.SIGTERM, "synthetic-old-handler")])

    def test_missing_assets_or_bad_certificate_fails_before_binding(self):
        with mock.patch.dict(os.environ, {"CI": "true"}), mock.patch.object(frontend, "FixtureServer") as server:
            with self.assertRaises(OSError):
                frontend.serve(self.root, self.root / "missing.pem", self.root / "missing.key", 9443, 3000)
        server.assert_not_called()
        context = mock.Mock()
        context.load_cert_chain.side_effect = ssl.SSLError("synthetic private certificate detail")
        with mock.patch.dict(os.environ, {"CI": "true"}), \
             mock.patch.object(frontend, "validate_inputs", return_value=(self.root, self.root / "cert", self.root / "key")), \
             mock.patch.object(frontend.ssl, "SSLContext", return_value=context), \
             mock.patch.object(frontend, "FixtureServer") as server:
            with self.assertRaises(ssl.SSLError):
                frontend.serve(self.root, "cert", "key", 9443, 3000)
        server.assert_not_called()

    def test_invalid_or_colliding_ports_fail_before_binding(self):
        for listen, upstream in ((0, 3000), (65536, 3000), (9443, 0), (9443, 65536), (3000, 3000)):
            with self.subTest(listen=listen, upstream=upstream), \
                 mock.patch.dict(os.environ, {"CI": "true"}), \
                 mock.patch.object(frontend, "validate_inputs", return_value=(self.root, self.root / "cert", self.root / "key")), \
                 mock.patch.object(frontend, "FixtureServer") as server:
                with self.assertRaises(ValueError):
                    frontend.serve(self.root, "cert", "key", listen, upstream)
                server.assert_not_called()

    def test_signal_requests_context_cleanup_without_synchronous_shutdown(self):
        with self.assertRaises(frontend.StopFrontend):
            frontend.stop_frontend(signal.SIGTERM, None)
        self.assertTrue(frontend.FixtureServer.daemon_threads)
        self.assertFalse(frontend.FixtureServer.block_on_close)

    def test_cli_defaults_and_sanitized_startup_failure(self):
        argv = ["--dist", "synthetic-dist", "--cert", "synthetic-cert", "--key", "synthetic-key"]
        with mock.patch.object(frontend, "serve") as serve:
            self.assertEqual(frontend.main(argv), 0)
        serve.assert_called_once_with(Path("synthetic-dist"), Path("synthetic-cert"), Path("synthetic-key"), 9443, 3000)
        capture = io.StringIO()
        with mock.patch.object(frontend, "serve", side_effect=ValueError("synthetic private certificate/key/body")), redirect_stderr(capture):
            self.assertEqual(frontend.main(argv + ["--listen-port", "9555", "--upstream-port", "3111"]), 1)
        self.assertEqual(capture.getvalue(), "TLS frontend startup failed\n")


if __name__ == "__main__":
    unittest.main()
