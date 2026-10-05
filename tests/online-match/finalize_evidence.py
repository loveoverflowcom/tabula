"""Closed receipt/final exit policy, never substitute for either real execution."""
import argparse
import json
from pathlib import Path

PASS_MESSAGE = "PASS: two independent actual Chromium processes completed rendered Chess through real durable authority\n"


def finalize(artifacts: Path, browser_status: int, audit_status: int,
             audit_input_present: bool) -> int:
    if any(isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 255
           for value in (browser_status, audit_status)) or not isinstance(audit_input_present, bool):
        raise ValueError("invalid closed execution status")
    passed = browser_status == 0 and audit_status == 0 and audit_input_present
    receipt = {"version": 1, "browser_exit_status": browser_status,
               "durable_audit": "pass" if audit_input_present and audit_status == 0 else "fail" if audit_input_present else "not_run",
               "main_terminal_confirmed": audit_input_present,
               "all_mandatory_checks_passed": passed}
    (artifacts / "audit-result.json").write_text(json.dumps(receipt, indent=2) + "\n")
    result = artifacts / "result.txt"
    if passed:
        result.write_text(PASS_MESSAGE)
    else:
        result.unlink(missing_ok=True)
    return 0 if passed else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--browser-status", required=True, type=int)
    parser.add_argument("--audit-status", required=True, type=int)
    parser.add_argument("--audit-input-present", required=True, type=int, choices=(0, 1))
    args = parser.parse_args()
    return finalize(args.artifacts, args.browser_status, args.audit_status,
                    args.audit_input_present == 1)


if __name__ == "__main__":
    raise SystemExit(main())
