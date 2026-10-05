"""Offline fixture correctness tests, never actual-browser acceptance evidence."""
import unittest
from types import SimpleNamespace
from unittest import mock
from browser_acceptance import AcceptanceFailure, api, board_square, denied, private_frame_keys, require, run


class BrowserHelperTests(unittest.TestCase):
    def test_browser_launch_requires_exact_disposable_ci_opt_in_before_any_setup(self):
        for environment in ({}, {"CI": "true"}, {"TABULA_ONLINE_MATCH_DISPOSABLE": "1"}):
            with self.subTest(environment=environment), mock.patch.dict("os.environ", environment, clear=True):
                with self.assertRaisesRegex(AcceptanceFailure, "CI-only disposable browser"):
                    run(SimpleNamespace(private="absent", artifacts="absent", ca="absent"))

    def test_exact_duplicate_body_retains_u128_bytes_instead_of_js_number_roundtrip(self):
        class FakePage:
            def evaluate(self, source, arguments):
                self.source = source
                self.arguments = arguments
                return {"status": 200, "body": {"frames": []}}
        page = FakePage()
        body = '{"command":{"match_id":340282366920938463463374607431768211455}}'
        api(page, "/api/v1/matches/fixture/command", body, "transient fixture token")
        self.assertEqual(page.arguments["body"], body)
        self.assertIn("typeof body === 'string' ? body", page.source)

    def test_nested_canonical_fact_is_detected_without_logging_its_value(self):
        for key in ("seed", "ledger", "state_hash", "logical_ms", "input_index"):
            self.assertTrue(private_frame_keys({"frames": [{"body": {key: "never printed"}}]}))

    def test_approved_visible_revision_and_projected_bytes_are_not_canonical_facts(self):
        self.assertFalse(private_frame_keys({"version": 1, "frames": [
            {"body": {"MatchUpdate": {"revision": 2, "view": [1, 2], "events": []}}},
            {"body": {"Ack": {"seq": 1}}}]}))

    def test_denial_with_frames_never_passes(self):
        with self.assertRaisesRegex(AcceptanceFailure, "released gameplay frames"):
            denied({"status": 403, "body": {"frames": [{"body": "private"}]}}, {403}, "denied")

    def test_expected_denial_without_frames_passes(self):
        denied({"status": 401, "body": {"code": "unauthenticated"}}, {401}, "denied")

    def test_wrong_error_class_and_failed_setup_do_not_green_skip(self):
        with self.assertRaises(AcceptanceFailure):
            denied({"status": 503, "body": None}, {401}, "wrong class")
        with self.assertRaises(AcceptanceFailure):
            require(False, "setup absent")

    def test_white_and_flipped_black_coordinates_are_mirrored(self):
        self.assertEqual(board_square(1100, 794, "a1", False), board_square(1100, 794, "h8", True))
        f2, f3 = board_square(1100, 794, "f2", False), board_square(1100, 794, "f3", False)
        self.assertEqual(f2[0], f3[0])
        self.assertGreater(f2[1], f3[1])
        e7, e5 = board_square(1100, 794, "e7", True), board_square(1100, 794, "e5", True)
        self.assertEqual(e7[0], e5[0])
        self.assertGreater(e7[1], e5[1])

    def test_invalid_square_fails_before_any_ui_action(self):
        for square in ("a0", "i2", "f99", ""):
            with self.assertRaises(AcceptanceFailure):
                board_square(1100, 794, square, False)


if __name__ == "__main__":
    unittest.main()
