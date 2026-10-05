#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
test "${TABULA_KANIDM_DISPOSABLE:-}" = 1
test -s "${TABULA_KANIDM_TEST_CONFIG:?}"
artifacts="$root/verification/kanidm-oidc-artifacts"
# Fixed failure-schema/redaction tests are synthetic diagnostics, not OIDC proof.
cargo test -p tabula-auth --features postgres-acceptance --test kanidm_provider helper_failure
cargo test -p tabula-auth --features postgres-acceptance --test kanidm_provider \
    -- --list --ignored > "$artifacts/selected-tests.txt"
grep -Eq '^.+: test$' "$artifacts/selected-tests.txt"
cargo test -p tabula-auth --features postgres-acceptance --test kanidm_provider \
    -- --ignored --test-threads=1
