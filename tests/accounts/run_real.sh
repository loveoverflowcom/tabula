#!/usr/bin/env bash
# Job-only real Kanidm + PostgreSQL + compiled shell HTTPS/WSS acceptance.
set -euo pipefail
umask 077
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
artifacts="$root/verification/account-social-artifacts"
mkdir -p "$artifacts"
# A setup failure must not leave a previous run's successful receipt behind.
rm -f -- "$artifacts/real-browser-receipt.json"
test "${TABULA_ACCOUNTS_DISPOSABLE:-}" = 1
test "${TABULA_KANIDM_DISPOSABLE:-}" = 1
test "${TABULA_KANIDM_SOCIAL:-}" = 1
test -s "${TABULA_KANIDM_TEST_CONFIG:?}"
for tool in openssl certutil python3; do
    command -v "$tool" >/dev/null || { echo "Required account acceptance helper absent: $tool" >&2; exit 1; }
done
test -s apps/web/dist/index.html
python3 tools/tests/check-loading-budgets.py --shell-dist apps/web/dist \
    --shell-profile account-social --write-receipt "$artifacts/real-browser-shell-budget.json"
private=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/tabula-accounts.XXXXXXXX")
native_pid=''
tls_pid=''
cleanup() {
    local result=$?
    trap - EXIT INT TERM
    if test -n "$tls_pid"; then kill "$tls_pid" 2>/dev/null || true; wait "$tls_pid" 2>/dev/null || true; fi
    if test -n "$native_pid"; then kill "$native_pid" 2>/dev/null || true; wait "$native_pid" 2>/dev/null || true; fi
    if test "$result" != 0 && test "${TABULA_ACCOUNTS_KEEP_PRIVATE_ON_FAILURE:-}" = 1; then
        printf 'Private disposable diagnostic directory retained: %s\n' "$private" >&2
    else
        rm -rf -- "$private"
    fi
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 1 \
    -subj '/CN=Tabula disposable account acceptance CA' \
    -addext 'basicConstraints=critical,CA:TRUE' \
    -addext 'keyUsage=critical,keyCertSign,cRLSign' \
    -keyout "$private/ca.key" -out "$private/ca.pem" >/dev/null 2>&1
openssl req -new -newkey rsa:2048 -nodes -sha256 -subj '/CN=app.localhost' \
    -keyout "$private/tls.key" -out "$private/leaf.csr" >/dev/null 2>&1
cat > "$private/leaf.ext" <<'EOF'
subjectAltName=DNS:app.localhost,DNS:localhost,IP:127.0.0.1
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
EOF
openssl x509 -req -sha256 -days 1 -in "$private/leaf.csr" \
    -CA "$private/ca.pem" -CAkey "$private/ca.key" -CAcreateserial \
    -extfile "$private/leaf.ext" -out "$private/leaf.pem" >/dev/null 2>&1
cat "$private/leaf.pem" "$private/ca.pem" > "$private/tls.pem"
openssl verify -CAfile "$private/ca.pem" -verify_hostname app.localhost "$private/leaf.pem" >/dev/null
git rev-parse HEAD > "$artifacts/source-sha.txt"
git rev-parse HEAD^{tree} > "$artifacts/source-tree.txt"
if test -n "$(git status --porcelain --untracked-files=normal)"; then
    printf '%s\n' 'working-tree (local run; HEAD/tree are base provenance)' > "$artifacts/source-state.txt"
else
    printf '%s\n' 'clean-checkout (HEAD/tree match acceptance source)' > "$artifacts/source-state.txt"
fi
export RUST_LOG=off
fixture="${CARGO_TARGET_DIR:-$root/target}/debug/examples/account_social_acceptance"
test -x "$fixture" || { echo 'Compiled actual account/social fixture required' >&2; exit 1; }
"$fixture" > "$private/native.log" 2>&1 &
native_pid=$!
python3 - "$native_pid" <<'PY'
import json, os, sys, time, urllib.request
deadline=time.monotonic()+20
while time.monotonic()<deadline:
    try:
        os.kill(int(sys.argv[1]),0)
        with urllib.request.urlopen('http://127.0.0.1:3001/api/v1/auth/context',timeout=1) as response:
            context=json.load(response)
            assert context['disposition']=='signed_out' and context['capabilities']['register']
        break
    except (OSError, ValueError, AssertionError):
        time.sleep(.1)
else:
    raise SystemExit('FAIL: actual account authority did not become ready')
PY
python3 tests/accounts/tls_frontend.py --dist apps/web/dist \
    --cert "$private/tls.pem" --key "$private/tls.key" > "$private/tls.log" 2>&1 &
tls_pid=$!
python3 - "$tls_pid" "$private/ca.pem" <<'PY'
import os, ssl, sys, time, urllib.request
context=ssl.create_default_context(cafile=sys.argv[2])
deadline=time.monotonic()+20
while time.monotonic()<deadline:
    try:
        os.kill(int(sys.argv[1]),0)
        with urllib.request.urlopen('https://localhost:8444/',context=context,timeout=1) as response:
            assert response.status==200 and b'<html' in response.read(65536).lower()
        os.kill(int(sys.argv[1]),0)
        break
    except (OSError, ValueError, AssertionError):
        time.sleep(.1)
else:
    raise SystemExit('FAIL: task-CA verified account TLS edge did not become ready')
PY
python3 tests/accounts/real_browser_acceptance.py --private "$private" \
    --artifacts "$artifacts" --ca "$private/ca.pem"
