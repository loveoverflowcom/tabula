#!/usr/bin/env bash
set -euo pipefail
test "${TABULA_KANIDM_DISPOSABLE:-}" = 1
test -s "${TABULA_KANIDM_TEST_CONFIG:?}"
artifacts=verification/kanidm-oidc-artifacts
case=real_kanidm_
cargo test -p tabula-auth --features accounts-acceptance --test kanidm_provider "$case" \
    -- --list --ignored > "$artifacts/selected-enrollment-tests.txt"
python3 - "$artifacts/selected-enrollment-tests.txt" <<'PY'
from pathlib import Path
import sys
selected=[line for line in Path(sys.argv[1]).read_text().splitlines() if line.endswith(': test')]
expected={
    'real_kanidm_invited_web_login_lifecycle_and_stale_epoch: test',
    'real_kanidm_legacy_profile_completion_captured_epoch_and_preservation: test',
    'real_kanidm_unmapped_enrollment_receipt_login_profile_and_policy_fences: test',
}
assert set(selected)==expected and len(selected)==3, 'Exact nonempty provider/account regression selection required'
PY
cargo test -p tabula-auth --features accounts-acceptance --test kanidm_provider "$case" \
    -- --ignored --test-threads=1
