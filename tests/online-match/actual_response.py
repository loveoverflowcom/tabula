"""Volatile authentic-fetch observation, separate from rendered/durable oracles."""
from __future__ import annotations

import hashlib
import itertools
import json
import math
from pathlib import Path
import re
import weakref
from urllib.parse import urlsplit

MAX_BODY = 2_097_152
MAX_TOTAL = 8_388_608
MAX_REQUEST = 131_072
ERRORS = frozenset({
    "observer_missing", "observer_not_installed", "request_descriptor_failed",
    "request_body_unsupported", "request_body_limit", "request_body_read_failed",
    "request_fingerprint_deadline", "request_fingerprint_unavailable", "ambiguous_request",
    "request_not_observed", "request_owner_mismatch", "request_document_mismatch",
    "document_disposed", "document_mismatch", "record_count_limit", "total_body_limit",
    "original_fetch_failed", "response_redirected", "response_observation_failed",
    "response_observation_deadline", "response_not_observed", "response_metadata_mismatch",
    "response_body_limit", "response_body_truncated", "response_body_read_failed",
    "response_body_unavailable", "body_read_deadline", "response_json_invalid",
    "observer_protocol_invalid", "observer_evaluation_failed", "response_identity_mismatch",
})


class ObservationFailure(Exception):
    """A source-owned enum only; never an exception, URL, header or body excerpt."""
    def __init__(self, code: str):
        self.code = code if code in ERRORS else "observer_protocol_invalid"
        super().__init__("actual_response_" + self.code)


def observed_endpoint(url: str) -> bool:
    value = urlsplit(url)
    return (value.scheme == "https" and value.netloc == "localhost:9443"
            and not value.query and not value.fragment
            and (value.path in ("/api/v1/matches", "/api/v1/matches/join")
                 or re.fullmatch(r"/api/v1/matches/[0-9a-f]{32}/(attach|command)", value.path) is not None))


def request_descriptor(request) -> dict:
    body = request.post_data_buffer
    if body is None:
        body = b""
    if not isinstance(body, bytes) or len(body) > MAX_REQUEST:
        raise ObservationFailure("request_body_limit")
    if not observed_endpoint(request.url):
        raise ObservationFailure("request_descriptor_failed")
    return {"method": request.method, "url": request.url,
            "body_sha256": hashlib.sha256(body).hexdigest()}


def response_metadata(response) -> dict:
    media = (response.header_value("content-type") or "").split(";")[0].strip().lower()
    length = response.header_value("content-length")
    if length is not None and (not re.fullmatch(r"[0-9]{1,10}", length) or int(length) > MAX_BODY):
        raise ObservationFailure("response_body_limit")
    return {"status": response.status, "url": response.url,
            "content_type": media if media in ("application/json", "application/problem+json") else "other",
            "no_store": "no-store" in {value.strip().lower() for value in (response.header_value("cache-control") or "").split(",")},
            "content_length": length}


def _checked(result: object, keys: set[str]) -> dict:
    if not isinstance(result, dict) or type(result.get("ok")) is not bool:
        raise ObservationFailure("observer_protocol_invalid")
    if not result["ok"]:
        if set(result) != {"ok", "error"} or not isinstance(result["error"], str) or result["error"] not in ERRORS:
            raise ObservationFailure("observer_protocol_invalid")
        raise ObservationFailure(result["error"])
    if set(result) != keys | {"ok"}:
        raise ObservationFailure("observer_protocol_invalid")
    return result


def _key(value):
    # Distinct public wrappers around the same real protocol object share identity.
    return getattr(value, "_impl_obj", value)


def _document(identity):
    if (not isinstance(identity, dict) or identity.get("disposed") is not False
            or identity.get("fatal") is not None):
        return None
    document = identity.get("document")
    return document if isinstance(document, str) and re.fullmatch(
        r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", document) else None


class _ContextObservation:
    def __init__(self):
        self.requests = weakref.WeakKeyDictionary()
        self.responses = weakref.WeakKeyDictionary()
        self.pages = weakref.WeakKeyDictionary()
        self.owners = itertools.count(1)
        self.closed = False

    def watch_page(self, page):
        if page in self.pages:
            return
        self.pages[page] = {"generation": 0, "bytes": {}, "document": None, "closed": False}
        page.on("close", lambda *_: self.clear_page(page, closed=True))
        page.on("framenavigated", lambda frame: self.navigated_page(page)
                if frame == page.main_frame else None)

    def navigated_page(self, page):
        state = self.pages.get(page)
        if state is None or self.closed or state["closed"]:
            return
        generation = state["generation"]
        try:
            document = _document(page.evaluate("() => window.__tabulaActualFetchObserver?.identity()"))
        except Exception:
            document = None
        # Evaluation can dispatch another navigation or bind a new document's
        # actual Request. An older callback must not retire that newer epoch.
        if self.closed or state["closed"] or generation != state["generation"]:
            return
        # Playwright also emits framenavigated for pushState/replaceState/hash.
        # Retain private bodies only when the same live JS epoch proves ownership.
        if document is None or document != state["document"]:
            self.clear_page(page, document=document)

    def clear_page(self, page, *, document=None, closed=False):
        state = self.pages.get(page)
        if state is not None:
            state["generation"] += 1
            state["bytes"].clear()
            state["document"] = document
            state["closed"] |= closed
        for mapping in (self.requests, self.responses):
            for key, value in list(mapping.items()):
                if value.get("page") is page:
                    del mapping[key]

    def close(self, *_):
        self.closed = True
        self.requests.clear(); self.responses.clear(); self.pages.clear()

    def bind_request(self, request):
        if self.closed or not observed_endpoint(request.url):
            return
        key = _key(request)
        entry = self.requests.get(key)
        if entry is None:
            # This method is invoked only by the actual Request event, never as
            # a body-read fallback that could select a later identical request.
            try:
                page = request.frame.page
                self.watch_page(page)
                entry = {"page": page, "generation": self.pages[page]["generation"],
                         "document": self.pages[page]["document"],
                         "descriptor": request_descriptor(request), "owner": str(next(self.owners)),
                         "binding": None, "error": None}
                self.requests[key] = entry
            except Exception:
                self.requests[key] = {"error": "request_descriptor_failed"}
                return
        if entry.get("binding") is not None or entry.get("error") is not None:
            return
        try:
            binding = _checked(entry["page"].evaluate("""async ({expected,owner}) => {
                const observer=window.__tabulaActualFetchObserver;
                return observer?.version===1 ? await observer.bind(expected,owner)
                    : {ok:false,error:'observer_missing'};
            }""", {"expected": entry["descriptor"], "owner": entry["owner"]}), {"id", "document"})
            if type(binding["id"]) is not int or not 1 <= binding["id"] <= 64 or not isinstance(binding["document"], str) or len(binding["document"]) != 36:
                raise ObservationFailure("observer_protocol_invalid")
            page = entry["page"]
            state = self.pages.get(page)
            if self.closed or state is None or state["closed"]:
                raise ObservationFailure("document_mismatch")
            generation = state["generation"]
            document = _document(page.evaluate("() => window.__tabulaActualFetchObserver?.identity()"))
            if self.closed or state["closed"]:
                raise ObservationFailure("document_mismatch")
            if document != binding["document"]:
                # An old binding can resume after a fresh request established
                # the current document. Reject it without clearing that epoch.
                if generation == state["generation"] and (document is None or document != state["document"]):
                    self.clear_page(page, document=document)
                raise ObservationFailure("document_mismatch")
            if entry["generation"] != state["generation"] and (
                    state["document"] != document or entry["document"] == document):
                raise ObservationFailure("document_mismatch")
            if state["document"] != document:
                self.clear_page(page, document=document)
            # A fresh document's request can bind while its navigation callback
            # is evaluating identity. Adopt that verified epoch once, and keep
            # this exact event-owned entry rather than selecting another request.
            entry["generation"] = state["generation"]
            entry["document"] = document
            entry["binding"] = binding
            self.requests[key] = entry
        except ObservationFailure as error:
            entry["error"] = error.code
        except Exception:
            entry["error"] = "observer_evaluation_failed"

    def read(self, response):
        request = response.request
        entry = self.requests.get(_key(request))
        if self.closed or entry is None:
            raise ObservationFailure("request_not_observed")
        if entry.get("error") is not None:
            raise ObservationFailure(entry["error"])
        if entry["binding"] is None:
            # A reentrant Response event may arrive while its Request callback
            # is awaiting the same fingerprint. The existing owner is reused.
            self.bind_request(request)
        if entry.get("error") is not None or entry["binding"] is None:
            raise ObservationFailure(entry.get("error") or "request_not_observed")
        page = entry["page"]
        if (page not in self.pages or entry["generation"] != self.pages[page]["generation"]
                or request_descriptor(request) != entry["descriptor"]):
            raise ObservationFailure("request_document_mismatch")
        binding = entry["binding"]
        metadata = response_metadata(response)
        started = request.timing["startTime"]
        if not isinstance(started, (int, float)) or isinstance(started, bool) or not math.isfinite(started) or started <= 0:
            raise ObservationFailure("request_document_mismatch")
        try:
            identity = page.evaluate("() => window.__tabulaActualFetchObserver?.identity()")
        except Exception:
            raise ObservationFailure("observer_evaluation_failed") from None
        if (not isinstance(identity, dict) or identity.get("document") != binding["document"]
                or identity.get("disposed") is not False or identity.get("fatal") is not None):
            raise ObservationFailure("document_mismatch")
        key = _key(response)
        cached = self.responses.get(key)
        if cached is not None:
            if cached["owner"] != entry["owner"] or cached["metadata"] != metadata or cached["started"] != started:
                raise ObservationFailure("response_identity_mismatch")
            text = cached["text"]
        else:
            try:
                result = _checked(page.evaluate("""async value => {
                    const observer=window.__tabulaActualFetchObserver;
                    return observer?.version===1 ? await observer.read(value)
                        : {ok:false,error:'observer_missing'};
                }""", {"id": binding["id"], "document": binding["document"],
                         "owner": entry["owner"], "metadata": metadata,
                         "request_started_at": started}), {"text"})
            except ObservationFailure:
                raise
            except Exception:
                raise ObservationFailure("observer_evaluation_failed") from None
            text = result["text"]
            if not isinstance(text, str) or len(text.encode("utf-8")) > MAX_BODY:
                raise ObservationFailure("response_body_limit")
            if (self.closed or page not in self.pages or entry["generation"] != self.pages[page]["generation"]
                    or self.requests.get(_key(request)) is not entry):
                raise ObservationFailure("document_mismatch")
            # A reentrant consumer can finish this exact transfer while the
            # outer evaluate is pending. It must neither reselect nor charge twice.
            concurrent = self.responses.get(key)
            if concurrent is not None:
                if (concurrent["owner"] != entry["owner"] or concurrent["metadata"] != metadata
                        or concurrent["started"] != started or concurrent["text"] != text):
                    raise ObservationFailure("response_identity_mismatch")
            else:
                budget = self.pages[page]["bytes"]
                size = len(text.encode("utf-8"))
                budget[binding["document"]] = budget.get(binding["document"], 0) + size
                if budget[binding["document"]] > MAX_TOTAL:
                    raise ObservationFailure("total_body_limit")
                self.responses[key] = {"page": page, "owner": entry["owner"], "metadata": metadata,
                                       "started": started, "text": text}
            try:
                _checked(page.evaluate("""value => {
                    const observer=window.__tabulaActualFetchObserver;
                    return observer?.version===1 ? observer.retireBody(value)
                        : {ok:false,error:'observer_missing'};
                }""", {"id": binding["id"], "document": binding["document"], "owner": entry["owner"]}), set())
            except ObservationFailure:
                raise
            except Exception:
                raise ObservationFailure("observer_evaluation_failed") from None
            if self.closed or page not in self.pages or entry["generation"] != self.pages[page]["generation"]:
                raise ObservationFailure("document_mismatch")
        try:
            return json.loads(text, parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
        except (ValueError, UnicodeError):
            raise ObservationFailure("response_json_invalid") from None


_CONTEXTS = weakref.WeakKeyDictionary()


def install_actual_response_observer(context) -> None:
    """Install before tested documents/scripts, using only volatile observations."""
    if context in _CONTEXTS:
        raise ObservationFailure("observer_protocol_invalid")
    state = _ContextObservation()
    context.add_init_script(script=Path(__file__).with_name("actual_fetch_observer.js").read_text())
    _CONTEXTS[context] = state
    context.on("request", state.bind_request)
    context.on("page", state.watch_page)
    context.on("close", state.close)
    for page in context.pages:
        state.watch_page(page)


def actual_response_json(response):
    """Parse one exact real response; no CDP body lookup, fallback or fabricated reply."""
    try:
        context = response.request.frame.page.context
        state = _CONTEXTS.get(context)
        if state is None:
            raise ObservationFailure("observer_not_installed")
        return state.read(response)
    except ObservationFailure:
        raise
    except Exception:
        raise ObservationFailure("observer_evaluation_failed") from None
