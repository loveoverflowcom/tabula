import importlib.util
import contextlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/online-match"))
sys.path.insert(0, str(ROOT / "tests/accounts"))

def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value

edge = module("local_dev_edge", Path(__file__).with_name("tls_frontend.py"))
runner = module("local_dev_runner", Path(__file__).with_name("run.py"))

class LocalDevHelpers(unittest.TestCase):
    def test_same_origin_context_and_callback_stay_with_preauth_owner(self):
        for path in ("/api/v1/auth/context", "/api/v1/auth/login",
                     "/api/v1/auth/oidc/callback?code=private&state=private",
                     "/api/v2/auth/enrollment/start", "/api/v2/auth/register"):
            handler = object.__new__(edge.ServiceHandler)
            handler.path = path
            self.assertEqual(handler._upstream_port(), 3001)
        for path in ("/api/v1/matches", "/api/v2/profiles/me", "/api/v2/social/search?q=peer"):
            handler = object.__new__(edge.ServiceHandler)
            handler.path = path
            self.assertEqual(handler._upstream_port(), 3002)

    def test_disposable_setup_cannot_inherit_operator_database_or_key(self):
        with patch.dict(os.environ, {"TABULA_AUTH_DATABASE_URL": "must-not-use",
                                     "TABULA_SERVER_CSRF_KEY": "must-not-use",
                                     "TABULA_KANIDM_DISPOSABLE": "1"}):
            environment = runner.service_environment()
        self.assertNotIn("TABULA_AUTH_DATABASE_URL", environment)
        self.assertNotIn("TABULA_SERVER_CSRF_KEY", environment)
        self.assertEqual(environment["TABULA_KANIDM_DISPOSABLE"], "1")

    def test_failed_browser_retains_completed_cases_without_private_diagnostics(self):
        private = "cookie-and-provider-private-value"
        def fail(_args, progress):
            progress.cases.append("actual_service_entrypoints_and_database_schema_readiness")
            progress.enter("first_browser_launch")
            raise runner.PlaywrightError(private)
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "receipt.json"
            output = io.StringIO()
            with patch.object(sys, "argv", ["run.py", "--receipt", str(receipt)]), \
                    patch.object(runner.subprocess, "check_output", return_value="public-source\n"), \
                    patch.object(runner, "build_provenance", return_value={}), \
                    patch.object(runner, "run", side_effect=fail), contextlib.redirect_stdout(output):
                self.assertEqual(runner.main(), 1)
            result = json.loads(receipt.read_text())
        self.assertEqual(result["cases"], ["actual_service_entrypoints_and_database_schema_readiness"])
        self.assertEqual(result["progress_stage"], "first_browser_launch")
        self.assertEqual(result["failure_exception"], "browser_error")
        self.assertNotIn(private, json.dumps(result) + output.getvalue())

    def test_diagnostic_stages_and_exception_types_are_closed(self):
        progress = runner.Progress()
        with self.assertRaises(ValueError):
            progress.enter("private-cookie-or-provider-diagnostic")
        self.assertEqual(progress.stage, "source_provenance")
        class PrivateError(Exception):
            def __str__(self):
                raise AssertionError("diagnostics must not read private exception text")
        self.assertEqual(runner.exception_kind(PrivateError()), "unexpected_error")
        error = runner.urllib.error.URLError(runner.ssl.SSLCertVerificationError("private certificate detail"))
        self.assertEqual(runner.exception_kind(error), "tls_certificate_error")

    def test_tls_readiness_uses_verified_loopback_certificate_and_browser_host(self):
        from unittest.mock import MagicMock
        import email.message
        private = Path("/private-test-only")
        context = runner.ssl.create_default_context()
        reply = MagicMock()
        reply.status = 200
        reply.geturl.return_value = "https://127.0.0.1:8444/"
        reply.headers = email.message.Message()
        reply.headers["Content-Type"] = "text/html"
        opener = MagicMock()
        opener.open.return_value.__enter__.return_value = reply
        child = MagicMock()
        child.poll.return_value = None
        with patch.object(runner.ssl, "create_default_context", return_value=context) as verify, \
                patch.object(runner.urllib.request, "build_opener", return_value=opener) as build:
            runner.Processes(private).tls_ready(child, private / "ca.pem")
        verify.assert_called_once_with(cafile=str(private / "ca.pem"))
        self.assertTrue(context.check_hostname)
        self.assertEqual(context.verify_mode, runner.ssl.CERT_REQUIRED)
        self.assertEqual(build.call_args.args[0].proxies, {})
        self.assertIs(build.call_args.args[1]._context, context)
        request = opener.open.call_args.args[0]
        self.assertEqual(request.get_method(), "HEAD")
        self.assertEqual(request.get_header("Host"), "app.localhost:8444")
        self.assertEqual(opener.open.call_args.kwargs["timeout"], 1)

if __name__ == "__main__":
    unittest.main()
