"""Offline exit/receipt policy tests, not real browser or database evidence."""
import json
from pathlib import Path
import tempfile
import unittest

from finalize_evidence import AUDIT_PREFIXES, CONTINUITY_CASES, actual_continuity_receipts, finalize


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

    def successful_continuity(self):
        cases=[]
        for label in CONTINUITY_CASES:
            case={'case':label,'pass':True}
            if label.endswith('_at_held_actual_delivery'):
                case.update(native_body_bytes=0,nonempty_projected_capture=True,actual_inner_guard_error=True)
            if label in ('committed-crash','uncommitted-crash'):
                case.update(actual_sigkill_reaped=True,fresh_fenced_owner=True)
            cases.append(case)
        (self.artifacts/'continuity-result.json').write_text(json.dumps({'status':'pass','distinct_browser_processes_verified':True,'cases':cases}))
        (self.artifacts/'continuity-audits').mkdir()
        for label,expected in AUDIT_PREFIXES.items():
            (self.artifacts/'continuity-audits'/('continuity-audit-'+label+'.json')).write_text(json.dumps({'status':'pass','actual_postgres':True,'exact_runtime_recovery':True,'accepted_inputs':expected,'audit_client_output':False}))

    def test_only_all_actual_commands_and_nonempty_exact_fault_oracles_seal_pass(self):
        self.successful_continuity()
        self.assertEqual(finalize(self.artifacts, 0, 0, True, 0, 0, len(AUDIT_PREFIXES)), 0)
        self.assertTrue((self.artifacts / "result.txt").exists())
        self.assertTrue(json.loads((self.artifacts / "audit-result.json").read_text())["all_mandatory_checks_passed"])

    def test_main_game_alone_cannot_claim_continuity(self):
        self.assertEqual(finalize(self.artifacts,0,0,True),1)

    def test_missing_duplicate_or_failed_fault_partition_cannot_pass(self):
        self.successful_continuity()
        path=self.artifacts/'continuity-result.json';original=json.loads(path.read_text())
        for cases in ([],original['cases'][:-1],[original['cases'][0]]*len(CONTINUITY_CASES)):
            changed=dict(original,cases=cases);path.write_text(json.dumps(changed))
            self.assertFalse(actual_continuity_receipts(self.artifacts))
        original['cases'][0]['pass']=False;path.write_text(json.dumps(original))
        self.assertFalse(actual_continuity_receipts(self.artifacts))

    def test_partial_private_bytes_or_absent_guard_error_cannot_pass(self):
        self.successful_continuity();path=self.artifacts/'continuity-result.json';value=json.loads(path.read_text())
        selected=next(case for case in value['cases'] if case['case'].endswith('_at_held_actual_delivery'))
        selected['native_body_bytes']=1;path.write_text(json.dumps(value));self.assertFalse(actual_continuity_receipts(self.artifacts))
        selected['native_body_bytes']=0;selected['actual_inner_guard_error']=False;path.write_text(json.dumps(value));self.assertFalse(actual_continuity_receipts(self.artifacts))

    def test_unknown_oracle_count_wrong_prefix_and_missing_recovery_cannot_pass(self):
        self.successful_continuity()
        self.assertEqual(finalize(self.artifacts,0,0,True,0,0,0),1)
        path=next((self.artifacts/'continuity-audits').glob('*.json'));value=json.loads(path.read_text());value['accepted_inputs']=99;path.write_text(json.dumps(value))
        self.assertFalse(actual_continuity_receipts(self.artifacts))


if __name__ == "__main__":
    unittest.main()
