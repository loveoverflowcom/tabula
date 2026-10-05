"""Closed startup facts only: no raw errors, URLs, headers, bodies or log dumps."""
import os
from pathlib import Path

NAVIGATION_FAILURES = {
    "net::ERR_CERT_AUTHORITY_INVALID": "certificate_authority_invalid",
    "net::ERR_CERT_COMMON_NAME_INVALID": "certificate_hostname_invalid",
    "net::ERR_CERT_DATE_INVALID": "certificate_date_invalid",
    "net::ERR_CERT_INVALID": "certificate_invalid",
    "net::ERR_SSL_PROTOCOL_ERROR": "tls_protocol_error",
    "net::ERR_CONNECTION_REFUSED": "connection_refused",
    "net::ERR_CONNECTION_RESET": "connection_reset",
    "net::ERR_NAME_NOT_RESOLVED": "name_not_resolved",
    "net::ERR_TIMED_OUT": "navigation_timeout",
    "net::ERR_EMPTY_RESPONSE": "empty_response",
    "net::ERR_ABORTED": "navigation_aborted",
}
NATIVE_FAILURES = {
    "FAIL: explicit CI-only disposable fixture opt-in is required": "opt_in_absent",
    "FAIL: actual disposable PostgreSQL URL is required": "database_setup_absent",
    "FAIL: actual disposable PostgreSQL connection failed": "database_connection_failed",
    "FAIL: strict migration composition does not match the independently reviewed set": "migration_set_mismatch",
    "FAIL: strict composed migrations failed": "composed_migration_failed",
    "FAIL: canonical HTTPS session composition failed": "session_composition_failed",
    "FAIL: isolated current-authority match composition failed": "match_composition_failed",
    "FAIL: isolated fixture loopback bind failed": "native_bind_failed",
    "FAIL: isolated fixture listener failed": "native_listener_failed",
}


def navigation_failure(value: str | None) -> str:
    return NAVIGATION_FAILURES.get(value, "other_navigation_failure")


def origin_class(value: str | None, expected: str) -> str:
    if value is None:
        return "missing"
    if value == expected:
        return "exact"
    return "null" if value == "null" else "foreign"


def http_status(value: int | None) -> int | None:
    return value if isinstance(value, int) and not isinstance(value, bool) and 100 <= value <= 599 else None


def process_phase(pid: int | None) -> str:
    if not isinstance(pid, int) or isinstance(pid, bool) or pid <= 0:
        return "not_configured"
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return "exited"
    except (PermissionError, OSError):
        return "unobservable"
    return "alive"


def closed_log_failure(path: Path, allowed: dict[str, str]) -> str:
    try:
        with path.open("rb") as stream:
            raw = stream.read(8193)
        if len(raw) > 8192:
            raw = raw[:8192].rsplit(b"\n", 1)[0] + b"\n" if b"\n" in raw[:8192] else b""
        elif raw and not raw.endswith(b"\n"):
            raw = raw.rsplit(b"\n", 1)[0] + b"\n" if b"\n" in raw else b""
        lines = raw.decode("utf-8", errors="replace").splitlines()
    except OSError:
        return "log_unavailable"
    # Only complete exact source-owned fixed lines become enum labels.
    for line in lines:
        if line in allowed:
            return allowed[line]
    return "none_observed"


def process_diagnostics(private: Path, native_pid: int | None, tls_pid: int | None) -> dict:
    return {
        "native_process_phase": process_phase(native_pid),
        "native_failure": closed_log_failure(private / "fixture.log", NATIVE_FAILURES),
        "tls_process_phase": process_phase(tls_pid),
        "tls_failure": closed_log_failure(private / "tls.log", {"TLS frontend startup failed": "tls_startup_failed"}),
    }
