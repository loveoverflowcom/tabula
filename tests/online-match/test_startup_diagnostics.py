"""Offline closed-diagnostic checks, never actual browser/TLS acceptance proof."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from startup_diagnostics import NATIVE_FAILURES, closed_log_failure, http_status, navigation_failure, origin_class, process_phase


class StartupDiagnosticsTests(unittest.TestCase):
    def test_navigation_errors_are_exact_whitelisted_enums_without_raw_text(self):
        self.assertEqual(navigation_failure("net::ERR_CERT_AUTHORITY_INVALID"), "certificate_authority_invalid")
        self.assertEqual(navigation_failure("net::ERR_CONNECTION_REFUSED"), "connection_refused")
        for value in (None, "cookie=synthetic-secret", "net::ERR_CERT_AUTHORITY_INVALID https://secret.invalid/?token=secret"):
            self.assertEqual(navigation_failure(value), "other_navigation_failure")

    def test_origin_is_only_a_closed_class(self):
        expected = "https://localhost:9443"
        self.assertEqual([origin_class(value, expected) for value in (expected, None, "null", "https://secret.invalid")],
                         ["exact", "missing", "null", "foreign"])

    def test_http_status_domain_is_bounded_and_boolean_is_not_a_status(self):
        self.assertEqual(http_status(403), 403)
        for value in (None, 99, 600, True, "cookie=synthetic-secret"):
            self.assertIsNone(http_status(value))

    def test_process_probe_only_checks_signal_zero_and_never_serializes_pid(self):
        with mock.patch("startup_diagnostics.os.kill") as kill:
            self.assertEqual(process_phase(123), "alive")
            kill.assert_called_once_with(123, 0)
        for error, expected in ((ProcessLookupError(), "exited"), (PermissionError(), "unobservable")):
            with mock.patch("startup_diagnostics.os.kill", side_effect=error):
                self.assertEqual(process_phase(123), expected)
        for invalid in (None, -1, 0, True):
            self.assertEqual(process_phase(invalid), "not_configured")

    def test_log_reader_requires_complete_exact_owned_lines_without_dumping_data(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "private.log"
            path.write_text("cookie=synthetic-secret\nFAIL: strict composed migrations failed cookie=secret\n")
            result = closed_log_failure(path, NATIVE_FAILURES)
            self.assertEqual(result, "none_observed")
            self.assertNotIn("secret", json.dumps(result))
            path.write_text("FAIL: strict composed migrations failed\n")
            self.assertEqual(closed_log_failure(path, NATIVE_FAILURES), "composed_migration_failed")
            path.write_text("FAIL: strict composed migrations failed")
            self.assertEqual(closed_log_failure(path, NATIVE_FAILURES), "none_observed")
            path.write_bytes(b"x" * 8192 + b"\nFAIL: strict composed migrations failed\n")
            self.assertEqual(closed_log_failure(path, NATIVE_FAILURES), "none_observed")


if __name__ == "__main__":
    unittest.main()
