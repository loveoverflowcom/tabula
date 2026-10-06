"""Offline identity/cancellation contracts; no rendered acceptance is implied."""
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest import mock

import actual_response as observer

URL = "https://localhost:9443/api/v1/matches/" + "a" * 32 + "/attach"
DOCUMENT = "11111111-1111-4111-8111-111111111111"
NEW_DOCUMENT = "22222222-2222-4222-8222-222222222222"


class Events:
    def __init__(self): self.handlers = {}
    def on(self, name, callback): self.handlers.setdefault(name, []).append(callback)
    def emit(self, name, *args):
        for callback in self.handlers.get(name, []): callback(*args)


class Page(Events):
    def __init__(self, context):
        super().__init__(); self.context = context; self.main_frame = SimpleNamespace(page=self)
        self.document = DOCUMENT; self.disposed = False; self.fatal = None
        self.text = '{"seat":0,"frames":[]}'
        self.bind_error = self.read_error = None; self.reads = 0; self.binds = 0
    def evaluate(self, source, value=None):
        if "observer.bind" in source:
            self.binds += 1
            self.bound = value
            return {"ok":False,"error":self.bind_error} if self.bind_error else {"ok":True,"id":1,"document":self.document}
        if "identity()" in source:
            return {"document":self.document,"disposed":self.disposed,"fatal":self.fatal}
        if "observer.read" in source:
            self.reads += 1; self.last_read = value
            return {"ok":False,"error":self.read_error} if self.read_error else {"ok":True,"text":self.text}
        if "observer.retireBody" in source: return {"ok":True}
        raise AssertionError("unexpected helper evaluation")


class Context(Events):
    def __init__(self): super().__init__(); self.pages = []; self.scripts = []
    def add_init_script(self, *, script): self.scripts.append(script)


class Request:
    def __init__(self, page, body=b'{"version":2,"binding_id":"synthetic"}'):
        self.frame = page.main_frame; self.url = URL; self.method = "POST"; self.post_data_buffer = body
        self.timing = {"startTime": 1000.125}


class Response:
    def __init__(self, request):
        self.request = request; self.url = request.url; self.status = 200
        self.headers = {"content-type":"application/json","cache-control":"private, no-store","content-length":None}
    def header_value(self, name): return self.headers.get(name)
    def json(self): raise AssertionError("CDP JSON fallback must never be used")
    def body(self): raise AssertionError("CDP body fallback must never be used")


class ActualResponseTests(unittest.TestCase):
    def setUp(self):
        self.context = Context(); observer.install_actual_response_observer(self.context)
        self.page = Page(self.context); self.context.pages.append(self.page); self.context.emit("page", self.page)
        self.request = Request(self.page); self.context.emit("request", self.request)
        self.response = Response(self.request)
    def tearDown(self): self.context.emit("close")
    def failure(self, code, operation):
        with self.assertRaises(observer.ObservationFailure) as caught: operation()
        self.assertEqual(caught.exception.code, code)

    def test_installs_before_scripts_for_all_new_documents_without_routes(self):
        self.assertEqual(len(self.context.scripts), 1)
        self.assertIn("Reflect.apply(original, this, args)", self.context.scripts[0])
        self.assertIn("response.clone()", self.context.scripts[0])
        self.assertFalse(hasattr(self.context, "route"))

    def test_original_body_fingerprint_preserves_u128_bytes(self):
        raw = b'{"match_id":340282366920938463463374607431768211455}'
        request = Request(self.page, raw)
        self.assertEqual(observer.request_descriptor(request)["body_sha256"], hashlib.sha256(raw).hexdigest())
        self.assertEqual(request.post_data_buffer, raw)

    def test_exact_bound_response_uses_authentic_observation(self):
        self.assertEqual(observer.actual_response_json(self.response), {"seat":0,"frames":[]})
        self.assertEqual(self.page.last_read["owner"], self.page.bound["owner"])
        self.assertEqual(self.page.last_read["request_started_at"], 1000.125)

    def test_multiple_consumers_cache_specific_response_and_return_separate_values(self):
        first = observer.actual_response_json(self.response); first["seat"] = 99
        alias = Response(self.request); alias._impl_obj = self.response
        self.assertEqual(observer.actual_response_json(alias)["seat"], 0)
        self.assertEqual(self.page.reads, 1)

    def test_other_response_identity_never_uses_an_earlier_cached_success(self):
        observer.actual_response_json(self.response)
        other = Response(self.request); self.page.read_error = "response_body_unavailable"
        self.failure("response_body_unavailable", lambda: observer.actual_response_json(other))
        self.assertEqual(self.page.reads, 2)

    def test_reentrant_consumers_share_one_exact_response_and_charge_once(self):
        evaluate = self.page.evaluate; nested = []
        def reentrant(source, value=None):
            if "observer.read" in source and not nested:
                nested.append(None)
                nested[0] = observer.actual_response_json(self.response)
            return evaluate(source, value)
        with mock.patch.object(self.page, "evaluate", side_effect=reentrant):
            value = observer.actual_response_json(self.response)
        self.assertEqual(value, nested[0])
        state = observer._CONTEXTS[self.context]
        self.assertEqual(state.pages[self.page]["bytes"][DOCUMENT], len(self.page.text.encode()))

    def test_navigation_or_close_during_read_cannot_repopulate_retired_cache(self):
        for close_context in (False, True):
            context = Context(); observer.install_actual_response_observer(context)
            page = Page(context); context.emit("page", page)
            request = Request(page); context.emit("request", request); response = Response(request)
            evaluate = page.evaluate
            def interrupted(source, value=None):
                result = evaluate(source, value)
                if "observer.read" in source:
                    if close_context:
                        context.emit("close")
                    else:
                        page.document = NEW_DOCUMENT
                        page.emit("framenavigated", page.main_frame)
                return result
            with mock.patch.object(page, "evaluate", side_effect=interrupted):
                self.failure("document_mismatch", lambda: observer.actual_response_json(response))
            self.assertEqual(len(observer._CONTEXTS[context].responses), 0)
            context.emit("close")

    def test_missing_actual_request_event_cannot_bind_from_a_response_fallback(self):
        other = Response(Request(self.page))
        self.failure("request_not_observed", lambda: observer.actual_response_json(other))
        self.assertEqual(self.page.binds, 1)

    def test_duplicate_or_failed_binding_never_selects_successful_bytes(self):
        for code in ("ambiguous_request", "request_fingerprint_unavailable", "original_fetch_failed"):
            self.page.bind_error = code; request = Request(self.page); self.context.emit("request", request)
            self.failure(code, lambda: observer.actual_response_json(Response(request)))
        self.assertEqual(self.page.reads, 0)

    def test_wrong_original_method_url_or_body_fingerprint_fails(self):
        for field, value in (("method","GET"),("url",URL.replace("attach","command")),("post_data_buffer",b"different")):
            old = getattr(self.request, field); setattr(self.request, field, value)
            self.failure("request_document_mismatch", lambda: observer.actual_response_json(self.response))
            setattr(self.request, field, old)

    def test_request_size_and_foreign_query_or_fragment_paths_are_rejected(self):
        request = Request(self.page, b"x" * (observer.MAX_REQUEST + 1))
        self.failure("request_body_limit", lambda: observer.request_descriptor(request))
        for url in (URL + "?synthetic=private", URL + "#fragment", URL.replace("localhost","foreign.invalid")):
            self.assertFalse(observer.observed_endpoint(url))

    def test_response_metadata_stays_fixed_and_no_header_secret_is_retained(self):
        self.response.headers["set-cookie"] = "synthetic-secret"
        self.response.headers["content-type"] = "synthetic/secret"
        value = observer.response_metadata(self.response)
        self.assertEqual(value["content_type"], "other")
        self.assertNotIn("secret", json.dumps(value))

    def test_wrong_cached_status_or_metadata_cannot_reuse_a_body(self):
        observer.actual_response_json(self.response); self.response.status = 409
        self.failure("response_identity_mismatch", lambda: observer.actual_response_json(self.response))

    def test_invalid_announced_lengths_fail_without_echoing_the_value(self):
        for length in ("synthetic-secret", "1" * 50, str(observer.MAX_BODY + 1), "-1"):
            self.response.headers["content-length"] = length
            self.failure("response_body_limit", lambda: observer.actual_response_json(self.response))

    def test_metadata_or_old_native_document_time_mismatch_remains_failure(self):
        for code in ("response_metadata_mismatch", "request_document_mismatch"):
            self.page.read_error = code
            self.failure(code, lambda: observer.actual_response_json(self.response))

    def test_missing_nonfinite_or_boolean_native_start_time_fails(self):
        for time in (0, -1, float("nan"), float("inf"), True):
            self.request.timing["startTime"] = time
            self.failure("request_document_mismatch", lambda: observer.actual_response_json(self.response))

    def test_read_abort_truncation_deadline_and_limits_never_fall_back(self):
        for code in ("response_body_read_failed", "response_body_truncated", "body_read_deadline",
                     "response_body_limit", "total_body_limit", "document_disposed"):
            self.page.read_error = code
            self.failure(code, lambda: observer.actual_response_json(self.response))

    def test_cached_response_cannot_survive_pagehide_or_a_new_document(self):
        observer.actual_response_json(self.response); self.page.disposed = True
        self.failure("document_mismatch", lambda: observer.actual_response_json(self.response))
        self.page.disposed = False; self.page.document = "22222222-2222-4222-8222-222222222222"
        self.failure("document_mismatch", lambda: observer.actual_response_json(self.response))

    def test_navigation_close_and_context_disposal_clear_request_and_body_caches(self):
        for event in ("framenavigated", "close"):
            with self.subTest(event=event):
                request = Request(self.page); self.context.emit("request", request)
                response = Response(request); observer.actual_response_json(response)
                if event == "framenavigated":
                    self.page.document = NEW_DOCUMENT
                    self.page.emit(event, self.page.main_frame)
                else:
                    self.page.emit(event)
                self.failure("request_not_observed", lambda: observer.actual_response_json(response))
        self.context.emit("close")
        self.failure("request_not_observed", lambda: observer.actual_response_json(self.response))

    def test_same_document_history_hash_and_subframe_events_preserve_exact_cache_and_budget(self):
        observer.actual_response_json(self.response)
        state = observer._CONTEXTS[self.context]
        generation = state.pages[self.page]["generation"]
        budget = dict(state.pages[self.page]["bytes"])
        for navigation in ("pushState", "replaceState", "hash"):
            with self.subTest(navigation=navigation):
                self.page.emit("framenavigated", self.page.main_frame)
                self.assertEqual(observer.actual_response_json(self.response)["seat"], 0)
                self.assertEqual(state.pages[self.page]["generation"], generation)
                self.assertEqual(state.pages[self.page]["bytes"], budget)
        self.page.emit("framenavigated", SimpleNamespace(page=self.page))
        self.assertEqual(observer.actual_response_json(self.response)["seat"], 0)
        self.assertEqual(self.page.reads, 1)

    def test_new_document_navigation_clears_bodies_and_allows_fresh_actual_request(self):
        observer.actual_response_json(self.response)
        state = observer._CONTEXTS[self.context]
        generation = state.pages[self.page]["generation"]
        self.page.document = NEW_DOCUMENT
        self.page.emit("framenavigated", self.page.main_frame)
        self.assertEqual(state.pages[self.page]["generation"], generation + 1)
        self.assertEqual(state.pages[self.page]["bytes"], {})
        self.assertEqual(len(state.responses), 0)
        self.failure("request_not_observed", lambda: observer.actual_response_json(self.response))
        request = Request(self.page); self.context.emit("request", request)
        self.assertEqual(observer.actual_response_json(Response(request))["seat"], 0)
        self.assertEqual(set(state.pages[self.page]["bytes"]), {NEW_DOCUMENT})

    def test_unknown_disposed_or_failed_navigation_identity_retires_private_caches(self):
        for identity in (None, {}, {"document":DOCUMENT,"disposed":True,"fatal":None},
                         {"document":DOCUMENT,"disposed":False,"fatal":"body_read_deadline"},
                         {"document":"synthetic-secret","disposed":False,"fatal":None}, RuntimeError("synthetic-secret")):
            with self.subTest(identity_type=type(identity).__name__):
                context = Context(); observer.install_actual_response_observer(context)
                page = Page(context); context.emit("page", page)
                request = Request(page); context.emit("request", request)
                response = Response(request); observer.actual_response_json(response)
                with mock.patch.object(page, "evaluate", side_effect=identity if isinstance(identity, Exception) else None,
                                       return_value=identity):
                    page.emit("framenavigated", page.main_frame)
                self.failure("request_not_observed", lambda: observer.actual_response_json(response))
                state = observer._CONTEXTS[context]
                self.assertEqual(state.pages[page]["bytes"], {})
                self.assertIsNone(state.pages[page]["document"])
                self.assertEqual(len(state.responses), 0)
                context.emit("close")

    def test_initial_unknown_navigation_is_retired_then_actual_binding_sets_active_document(self):
        context = Context(); observer.install_actual_response_observer(context)
        page = Page(context); context.emit("page", page)
        state = observer._CONTEXTS[context]
        self.assertIsNone(state.pages[page]["document"])
        with mock.patch.object(page, "evaluate", return_value=None):
            page.emit("framenavigated", page.main_frame)
        self.assertEqual(state.pages[page]["generation"], 1)
        request = Request(page); context.emit("request", request)
        self.assertEqual(state.pages[page]["document"], DOCUMENT)
        self.assertEqual(observer.actual_response_json(Response(request))["seat"], 0)
        context.emit("close")

    def test_new_document_request_reentrant_during_navigation_is_not_retired_twice(self):
        observer.actual_response_json(self.response)
        self.page.document = NEW_DOCUMENT
        evaluate = self.page.evaluate; fresh = []
        def reentrant(source, value=None):
            result = evaluate(source, value)
            if "identity()" in source and not fresh:
                fresh.append(Request(self.page))
                self.context.emit("request", fresh[0])
            return result
        with mock.patch.object(self.page, "evaluate", side_effect=reentrant):
            self.page.emit("framenavigated", self.page.main_frame)
        self.failure("request_not_observed", lambda: observer.actual_response_json(self.response))
        self.assertEqual(observer.actual_response_json(Response(fresh[0]))["seat"], 0)
        state = observer._CONTEXTS[self.context]
        self.assertEqual(set(state.pages[self.page]["bytes"]), {NEW_DOCUMENT})

    def test_pending_old_binding_cannot_repopulate_after_unknown_epoch_retirement(self):
        evaluate = self.page.evaluate; interrupted = []
        def reentrant(source, value=None):
            result = evaluate(source, value)
            if "observer.bind" in source and not interrupted:
                interrupted.append(True)
                with mock.patch.object(self.page, "evaluate", return_value=None):
                    self.page.emit("framenavigated", self.page.main_frame)
            return result
        request = Request(self.page)
        with mock.patch.object(self.page, "evaluate", side_effect=reentrant):
            self.context.emit("request", request)
        self.failure("request_not_observed", lambda: observer.actual_response_json(Response(request)))
        state = observer._CONTEXTS[self.context]
        self.assertEqual(state.pages[self.page]["bytes"], {})
        self.assertEqual(len(state.requests), 0)

    def test_old_binding_resuming_after_fresh_document_binding_cannot_clear_fresh_cache(self):
        evaluate = self.page.evaluate; fresh = []
        state = observer._CONTEXTS[self.context]
        def reentrant(source, value=None):
            result = evaluate(source, value)
            if "observer.bind" in source and not fresh:
                self.page.document = NEW_DOCUMENT
                self.page.emit("framenavigated", self.page.main_frame)
                fresh.append(Response(Request(self.page)))
                self.context.emit("request", fresh[0].request)
                observer.actual_response_json(fresh[0])
            return result
        old = Request(self.page)
        with mock.patch.object(self.page, "evaluate", side_effect=reentrant):
            self.context.emit("request", old)
        self.failure("request_not_observed", lambda: observer.actual_response_json(Response(old)))
        self.assertEqual(observer.actual_response_json(fresh[0])["seat"], 0)
        self.assertEqual(len(state.responses), 1)
        self.assertEqual(state.pages[self.page]["generation"], 2)
        self.assertEqual(state.pages[self.page]["bytes"], {NEW_DOCUMENT:len(self.page.text.encode())})

    def test_body_budget_and_invalid_json_are_mandatory(self):
        for text in ("not-json", "NaN", "Infinity", '{"x":NaN}'):
            self.page.text = text
            self.failure("response_json_invalid", lambda: observer.actual_response_json(Response(self.request)))
        self.page.text = "x" * (observer.MAX_BODY + 1)
        self.failure("response_body_limit", lambda: observer.actual_response_json(Response(self.request)))

    def test_combined_python_cache_budget_cannot_be_released_by_prior_reads(self):
        self.page.text = '"' + "x" * (observer.MAX_BODY - 2) + '"'
        for _ in range(4): observer.actual_response_json(Response(self.request))
        self.failure("total_body_limit", lambda: observer.actual_response_json(Response(self.request)))

    def test_unknown_error_or_extra_observer_protocol_fields_are_rejected(self):
        self.failure("observer_protocol_invalid", lambda: observer._checked({"ok":False,"error":"synthetic-secret"}, {"text"}))
        self.failure("observer_protocol_invalid", lambda: observer._checked({"ok":True,"text":"{}","frames":[]}, {"text"}))
        self.assertNotIn("secret", str(observer.ObservationFailure("synthetic-secret")))

    def test_no_response_json_consumers_remain_in_the_two_actual_harnesses(self):
        for name in ("browser_acceptance.py", "continuity_acceptance.py"):
            source = Path(__file__).with_name(name).read_text()
            self.assertNotIn(".json()", source)
            self.assertIn("install_actual_response_observer", source)


if __name__ == "__main__": unittest.main()
