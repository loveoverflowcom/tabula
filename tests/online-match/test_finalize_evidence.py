"""Offline exit/receipt policy tests, not real browser or database evidence."""
import json
from pathlib import Path
import tempfile
import unittest

from finalize_evidence import finalize


class FinalizeEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.artifacts = Path(self.temporary.name)

    def test_later_browser_failure_stays_fail_even_with_successful_real_audit(self):
        (self.artifacts / "result.txt").write_text("stale PASS")
        self.assertEqual(finalize(self.artifacts, 1, 0, True), 1)
        receipt = json.loads((self.artifacts / "audit-result.json").read_text())
        self.assertEqual(receipt["durable_audit"], "pass")
        self.assertFalse(receipt["all_mandatory_checks_passed"])
        self.assertFalse((self.artifacts / "result.txt").exists())

    def test_missing_confirmed_terminal_input_is_not_a_skipped_green_audit(self):
        self.assertEqual(finalize(self.artifacts, 0, 0, False), 1)
        self.assertEqual(json.loads((self.artifacts / "audit-result.json").read_text())["durable_audit"], "not_run")
        self.assertFalse((self.artifacts / "result.txt").exists())

    def test_audit_failure_cannot_pass_when_browser_completed(self):
        self.assertEqual(finalize(self.artifacts, 0, 1, True), 1)
        self.assertFalse((self.artifacts / "result.txt").exists())

    def test_only_both_actual_commands_success_with_confirmed_input_seal_pass(self):
        self.assertEqual(finalize(self.artifacts, 0, 0, True), 0)
        self.assertTrue((self.artifacts / "result.txt").exists())
        self.assertTrue(json.loads((self.artifacts / "audit-result.json").read_text())["all_mandatory_checks_passed"])


if __name__ == "__main__":
    unittest.main()
