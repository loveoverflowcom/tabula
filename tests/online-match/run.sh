#!/usr/bin/env bash
# CI-only, job-lifetime actual browser + PostgreSQL acceptance. No live service.
set -euo pipefail
umask 077
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
test "${TABULA_ONLINE_MATCH_DISPOSABLE:-}" = 1 || {
    echo 'Explicit disposable online-match acceptance opt-in is required' >&2; exit 1;
}
test -n "${TABULA_ONLINE_MATCH_DATABASE_URL:-}" || {
    echo 'A disposable real PostgreSQL database is required; setup cannot be skipped' >&2; exit 1;
}
for tool in openssl certutil python3 cargo; do
    command -v "$tool" >/dev/null || { echo "Required acceptance tool is absent: $tool" >&2; exit 1; }
done
test -s apps/web/dist/index.html
test -s apps/web/dist/play/local/play.html
private=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/tabula-online-match.XXXXXXXX")
artifacts="$root/verification/online-match-artifacts"
mkdir -p "$artifacts"
fixture_pid=''
tls_pid=''
cleanup() {
    local result=$?
    trap - EXIT INT TERM
    if test -n "$tls_pid"; then kill "$tls_pid" 2>/dev/null || true; wait "$tls_pid" 2>/dev/null || true; fi
    if test -n "$fixture_pid"; then kill "$fixture_pid" 2>/dev/null || true; wait "$fixture_pid" 2>/dev/null || true; fi
    rm -rf -- "$private"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
# Each new browser gets its own temporary HOME/NSS trust store. No global trust,
# ignored TLS error, security-warning bypass or saved login information.
openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 1 \
    -subj '/CN=Tabula disposable browser acceptance CA' \
    -addext 'basicConstraints=critical,CA:TRUE' \
    -addext 'keyUsage=critical,keyCertSign,cRLSign' \
    -keyout "$private/ca.key" -out "$private/ca.pem" >/dev/null 2>&1
openssl req -new -newkey rsa:2048 -nodes -sha256 -subj '/CN=localhost' \
    -keyout "$private/tls.key" -out "$private/leaf.csr" >/dev/null 2>&1
cat > "$private/leaf.ext" <<'EOF'
subjectAltName=DNS:localhost,IP:127.0.0.1
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
EOF
openssl x509 -req -sha256 -days 1 -in "$private/leaf.csr" \
    -CA "$private/ca.pem" -CAkey "$private/ca.key" -CAcreateserial \
    -extfile "$private/leaf.ext" -out "$private/leaf.pem" >/dev/null 2>&1
cat "$private/leaf.pem" "$private/ca.pem" > "$private/tls.pem"
openssl verify -CAfile "$private/ca.pem" -verify_hostname localhost "$private/leaf.pem" >/dev/null
git rev-parse HEAD > "$artifacts/source-sha.txt"
git rev-parse HEAD^{tree} > "$artifacts/source-tree.txt"
export SQLX_OFFLINE=true
export RUST_LOG=off
fixture="$root/tests/online-match/target/debug/online-match-fixture"
test -x "$fixture" || { echo 'Compiled disposable fixture binary is required' >&2; exit 1; }
"$fixture" serve > "$private/fixture.log" 2>&1 &
fixture_pid=$!
python3 tests/online-match/tls_frontend.py --dist apps/web/dist \
    --cert "$private/tls.pem" --key "$private/tls.key" \
    --listen-port 9443 --upstream-port 3000 > "$private/tls.log" 2>&1 &
tls_pid=$!
python3 tests/online-match/browser_acceptance.py --private "$private" \
    --artifacts "$artifacts" --ca "$private/ca.pem" \
    --native-pid "$fixture_pid" --tls-pid "$tls_pid"
# Synthetic account identifiers stay in the private file, outside artifacts.
# Claim a new fence only after browser teardown; recover verifies full history.
"$fixture" audit "$private/audit-input.json" > "$artifacts/durable-verdict.json"
printf '%s\n' 'PASS: two independent actual Chromium processes completed rendered Chess through real durable authority' \
    > "$artifacts/result.txt"
