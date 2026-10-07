"""Offline fixture correctness tests, never actual-browser acceptance evidence."""
import unittest
import http.client
import json
from playwright.sync_api import TimeoutError as BrowserTimeout
from actual_response import ObservationFailure
from types import SimpleNamespace
from unittest import mock
from browser_acceptance import AcceptanceFailure, NEUTRAL_UNAVAILABLE, TERMINAL_STATUS, active_browser_diagnostics, api, board_square, denied, exception_class, fresh_grant_required, game_status_class, private_frame_keys, protected_endpoint_class, record_live_poll_denial, reattach_required, require, run, start_native_poll, enter_game, visible_game_facts


class BrowserHelperTests(unittest.TestCase):
    def attachment_page(self, completed_body=None, completed=True):
        """Event doubles distinguish headers, canceled delivery and finished body."""
        path = 'https://localhost:9443/api/v1/matches/' + 'a' * 32 + '/attach'
        failed, delivered = mock.Mock(status=200, url=path), mock.Mock(status=200, url=path)
        for response in (failed, delivered):
            response.header_value.side_effect = {"content-type": "application/json", "cache-control": "no-store", "content-length": "100"}.get
            request = mock.Mock(method='POST', url=path, failure=None, post_data='original-grant-body')
            request.response.return_value = response
            response.request = request
        for response in (failed, delivered):
            response.json.side_effect = AssertionError('observer must never fall back to CDP JSON')
            response.body.side_effect = AssertionError('observer must never fall back to CDP body')
        delivered.observed_body = completed_body or {'version': 2, 'seat': 1, 'frames': []}
        page = mock.Mock()
        page.locator.return_value.is_visible.return_value = True
        listeners, selections = {}, []
        page.on.side_effect = lambda name, callback: listeners.setdefault(name, []).append(callback)
        page.remove_listener.side_effect = lambda name, callback: listeners[name].remove(callback)

        class Selection:
            def __init__(self, event, predicate, timeout):
                self.event, self.predicate, self.timeout = event, predicate, timeout
                self.value = None
                selections.append(self)
            def __enter__(self):
                return self
            def __exit__(self, *args):
                if not args[0] and self.value is None:
                    raise BrowserTimeout('completed attachment did not arrive')

        page.expect_event.side_effect = lambda event, predicate, timeout: Selection(event, predicate, timeout)
        page.expect_response.side_effect = lambda predicate, timeout: Selection('response', predicate, timeout)
        def emit(event, value):
            for callback in list(listeners.get(event, [])):
                callback(value)
            for selection in selections:
                if selection.value is None and event == selection.event and selection.predicate(value):
                    selection.value = value
        def navigate():
            emit('response', failed)
            failed.request.failure = 'net::ERR_ABORTED'
            emit('requestfailed', failed.request)
            if completed:
                emit('response', delivered)
                emit('requestfinished', delivered.request)
        page.get_by_test_id.return_value.click.side_effect = navigate
        return page, failed, delivered, selections

    def test_attach_selection_requires_a_finished_body_after_canceled_200_headers(self):
        page, failed, delivered, selections = self.attachment_page()
        trace = []
        with mock.patch('browser_acceptance.actual_response_json', return_value=delivered.observed_body) as observed:
            attachment, body = enter_game(page, 'a' * 32, 1, trace)
        self.assertEqual(attachment, delivered.observed_body)
        self.assertEqual(body, 'original-grant-body')
        observed.assert_called_once_with(delivered)
        for response in (failed, delivered):
            response.json.assert_not_called()
            response.body.assert_not_called()
        self.assertEqual([(item.event, item.timeout) for item in selections], [('requestfinished', 60_000)])
        self.assertEqual((trace[0]['header_responses'], trace[0]['failed_requests'], trace[0]['completed_responses']), (2, 1, 1))
        self.assertEqual(trace[0]['attempts'][1]['failure_class'], 'navigation_aborted')
        self.assertTrue(trace[0]['typed_seat_matches'] and trace[0]['native_status_observed'] and trace[0]['actual_canvas_visible'])

    def test_canceled_attachment_headers_alone_cannot_satisfy_acceptance(self):
        page, failed, _, _ = self.attachment_page(completed=False)
        with mock.patch('browser_acceptance.actual_response_json') as observed:
            with self.assertRaises(BrowserTimeout):
                enter_game(page, 'a' * 32, 1)
        observed.assert_not_called()
        failed.json.assert_not_called()
        failed.body.assert_not_called()

    def test_completed_attachment_errors_cannot_be_skipped_for_another_attempt(self):
        for body in ({'version': 2, 'seat': 0}, {'version': 1, 'seat': 1}, {'version': 2, 'seat': 1, 'seed': []}):
            page, _, delivered, _ = self.attachment_page(body)
            with self.subTest(body=body), mock.patch('browser_acceptance.actual_response_json', return_value=body) as observed:
                with self.assertRaises(AcceptanceFailure):
                    enter_game(page, 'a' * 32, 1)
                observed.assert_called_once_with(delivered)
        for code in ('response_json_invalid', 'response_body_read_failed'):
            page, _, delivered, _ = self.attachment_page()
            with self.subTest(code=code), mock.patch('browser_acceptance.actual_response_json', side_effect=ObservationFailure(code)) as observed:
                with self.assertRaises(ObservationFailure) as failure:
                    enter_game(page, 'a' * 32, 1)
                self.assertEqual(failure.exception.code, code)
                observed.assert_called_once_with(delivered)
                delivered.json.assert_not_called()
                delivered.body.assert_not_called()

    def test_visible_focus_and_visibility_diagnostics_use_closed_classes(self):
        page = mock.Mock()
        values = {'revision': '0', 'seat': '1', 'status': None, 'connection': None,
                  'availability': 'available', 'focused': True, 'visibility': 'visible'}
        page.evaluate.return_value = values
        facts = visible_game_facts(page, 'black')
        self.assertTrue(facts['document_has_focus'])
        self.assertEqual(facts['visibility_class'], 'visible')
        values.update(focused='synthetic-private', visibility='synthetic-private')
        facts = visible_game_facts(page, 'black')
        self.assertIsNone(facts['document_has_focus'])
        self.assertEqual(facts['visibility_class'], 'not_reported')
        self.assertNotIn('synthetic-private', json.dumps(facts))

    def test_protected_http_diagnostics_discard_ids_queries_and_foreign_routes(self):
        self.assertEqual(protected_endpoint_class("https://localhost:9443/api/v1/matches/" + "a" * 32 + "/attach"), "attach")
        self.assertEqual(protected_endpoint_class("https://localhost:9443/api/v1/auth/context"), "context")
        for invalid in ("https://localhost:9443/api/v1/auth/context?token=synthetic-secret", "https://foreign.invalid/api/v1/auth/context", "https://localhost:9443/__fixture/enroll"):
            self.assertIsNone(protected_endpoint_class(invalid))

    def test_failure_snapshot_occurs_before_outer_browser_driver_teardown(self):
        events = []
        class OuterDriver:
            def __enter__(self):
                return self
            def __exit__(self, *args):
                events.append("driver_stopped")
        class Page:
            url = "https://localhost:9443/play/local/?match_id=public"
            def is_closed(self):
                return False
        page = Page()
        connection = mock.Mock()
        connection.is_connected.return_value = True
        context = SimpleNamespace(pages=[page], browser=connection)
        results = {}
        def visible_snapshot(*args):
            self.assertNotIn("driver_stopped", events)
            events.append("snapshot")
            return {"status_class": "white_turn"}
        with mock.patch("browser_acceptance.visible_game_facts", side_effect=visible_snapshot):
            with self.assertRaises(TypeError):
                with OuterDriver(), active_browser_diagnostics([context], results, {}):
                    raise TypeError("synthetic-secret must never enter diagnostics")
        self.assertEqual(events, ["snapshot", "driver_stopped"])
        self.assertEqual(results["failure_class"], "python_type_error")
        self.assertNotIn("synthetic-secret", str(results))
        self.assertTrue(results["failure_views"][0]["browser_connected"])

    def test_exception_class_never_echoes_arbitrary_text(self):
        self.assertEqual(exception_class(KeyError("synthetic-secret")), "python_key_error")
        self.assertEqual(exception_class(RuntimeError("synthetic-secret")), "other_error")

    def test_live_poll401_observation_does_not_read_an_intentionally_aborted_body(self):
        response = mock.Mock(status=401)
        response.json.side_effect = RuntimeError("synthetic body unavailable after transport abort")
        response.body.side_effect = RuntimeError("synthetic body unavailable after transport abort")
        progress = {}
        record_live_poll_denial(response, progress)
        self.assertEqual(progress["actual_live_poll_status"], 401)
        response.json.assert_not_called()
        response.body.assert_not_called()
        with self.assertRaises(AcceptanceFailure):
            record_live_poll_denial(mock.Mock(status=403), {})

    def test_terminal_accessibility_status_requires_the_exact_checkmate_reason(self):
        self.assertEqual(TERMINAL_STATUS, "Game over / Black wins / checkmate")
        self.assertEqual(game_status_class(TERMINAL_STATUS), "black_checkmate")
        for wrong in ("Game over / Black wins", "Game over / Black wins / resignation", "token=synthetic-secret"):
            self.assertEqual(game_status_class(wrong), "other_status")

    def test_neutral_concealment_requires_positive_error_and_no_projected_pixels_or_status(self):
        for required in ("onlineAvailability === 'unavailable'", "onlineSeat", "onlineRevision", "onlineStatus", "onlineConnection",
                         "!Object.hasOwn", "canvas.width === 0", "canvas.height === 0", "aria-hidden", "visibility === 'hidden'",
                         "#runtime-error", "#error-detail", "Moves are blocked", "node.textContent.trim() === ''"):
            self.assertIn(required, NEUTRAL_UNAVAILABLE)

    def test_native_byte_oracle_requires_disposable_ci_before_connecting(self):
        with mock.patch.dict("os.environ", {}, clear=True), mock.patch("browser_acceptance.http.client.HTTPConnection") as connect:
            with self.assertRaises(AcceptanceFailure):
                start_native_poll("1" * 32, "2" * 32, "synthetic-cookie", "synthetic-csrf")
            connect.assert_not_called()

    def native_observation(self, response):
        connection = mock.Mock()
        connection.getresponse.return_value = response
        with mock.patch.dict("os.environ", {"CI": "true", "TABULA_ONLINE_MATCH_DISPOSABLE": "1"}, clear=True), mock.patch("browser_acceptance.http.client.HTTPConnection", return_value=connection):
            thread, done, observed = start_native_poll("1" * 32, "2" * 32, "synthetic-cookie", "synthetic-csrf")
            thread.join(timeout=5)
        self.assertTrue(done.is_set())
        connection.close.assert_called_once()
        return observed

    def test_native_byte_oracle_counts_private_partial_data_on_transport_error(self):
        response = mock.Mock(status=200)
        response.getheader.side_effect = lambda name, default: {"Content-Type": "application/json", "Cache-Control": "no-store"}.get(name, default)
        response.read.side_effect = http.client.IncompleteRead(b"synthetic-private", 1)
        observed = self.native_observation(response)
        self.assertEqual(observed["body_bytes"], len(b"synthetic-private"))
        self.assertTrue(observed["body_error"])
        self.assertTrue(observed["json_content_type"] and observed["no_store"])

    def test_native_byte_oracle_records_zero_bytes_on_empty_partial_transport_error(self):
        response = mock.Mock(status=200)
        response.getheader.side_effect = lambda name, default: {"Content-Type": "application/json", "Cache-Control": "no-store"}.get(name, default)
        response.read.side_effect = http.client.IncompleteRead(b"", 1)
        observed = self.native_observation(response)
        self.assertEqual(observed["body_bytes"], 0)
        self.assertTrue(observed["body_error"])

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

    def test_stale_local_attachment_requires_exact_recovery_classification(self):
        reattach_required({'status':409,'body':{'code':'reattach_required'}},'recovery expected')
        for response in ({'status':403,'body':{'code':'match_unavailable'}},
                         {'status':409,'body':{'code':'busy'}},
                         {'status':409,'body':{'code':'reattach_required','frames':[{'body':'private'}]}}):
            with self.assertRaises(AcceptanceFailure):reattach_required(response,'recovery expected')

    def test_current_member_grant_rejection_requires_the_exact_sole_problem(self):
        fresh_grant_required({'status':409,'body':{'code':'fresh_grant_required'}},'grant rejection expected')

    def test_fresh_grant_denial_rejects_every_other_http_status(self):
        for status in (200,400,401,403,404,429,503):
            with self.subTest(status=status), self.assertRaises(AcceptanceFailure):
                fresh_grant_required({'status':status,'body':{'code':'fresh_grant_required'}},'grant rejection expected')

    def test_fresh_grant_denial_rejects_wrong_code_or_non_problem_body(self):
        for body in (None,[],{},'fresh_grant_required',{'code':'reattach_required'},
                     {'code':'match_unavailable'},{'code':'unauthenticated'}):
            with self.subTest(body=body), self.assertRaises(AcceptanceFailure):
                fresh_grant_required({'status':409,'body':body},'grant rejection expected')

    def test_fresh_grant_denial_rejects_even_empty_or_null_frame_extras(self):
        for frames in (None,[],[{'body':{'MatchUpdate':{'revision':0,'view':[1]}}}]):
            with self.subTest(frames=frames), self.assertRaises(AcceptanceFailure):
                fresh_grant_required({'status':409,'body':{'code':'fresh_grant_required','frames':frames}},'grant rejection expected')

    def test_fresh_grant_denial_rejects_attachment_and_scope_extras(self):
        for field in ('attachment_id','operation_scope','next_seq','seat'):
            with self.subTest(field=field), self.assertRaises(AcceptanceFailure):
                fresh_grant_required({'status':409,'body':{'code':'fresh_grant_required',field:None}},'grant rejection expected')

    def test_fresh_grant_denial_rejects_projection_and_arbitrary_extras(self):
        for field in ('view','events','revision','canonical_state','unexpected'):
            with self.subTest(field=field), self.assertRaises(AcceptanceFailure):
                fresh_grant_required({'status':409,'body':{'code':'fresh_grant_required',field:[]}},'grant rejection expected')

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
