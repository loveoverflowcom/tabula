#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
test "${TABULA_KANIDM_DISPOSABLE:-}" = 1
test -s "${TABULA_KANIDM_TEST_CONFIG:?}"
artifacts="$root/verification/kanidm-oidc-artifacts"
cargo test -p tabula-auth --features postgres-acceptance --test kanidm_provider \
    -- --list --ignored > "$artifacts/selected-tests.txt"
grep -Eq '^.+: test$' "$artifacts/selected-tests.txt"
cargo test -p tabula-auth --features postgres-acceptance --test kanidm_provider \
    -- --ignored --test-threads=1
