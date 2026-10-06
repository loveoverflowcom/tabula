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
rm -f -- "$artifacts/result.txt"
supervisor_pid=''
fixture_pid=''
tls_pid=''
cleanup() {
    local result=$?
    trap - EXIT INT TERM
    if test -n "$tls_pid"; then kill "$tls_pid" 2>/dev/null || true; wait "$tls_pid" 2>/dev/null || true; fi
    if test -n "$supervisor_pid"; then kill "$supervisor_pid" 2>/dev/null || true; wait "$supervisor_pid" 2>/dev/null || true; fi
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
python3 tests/online-match/process_supervisor.py --fixture "$fixture" --private "$private" > "$private/supervisor.log" 2>&1 &
supervisor_pid=$!
python3 - "$private" <<'PY'
import sys, time
from pathlib import Path
sys.path.insert(0, 'tests/online-match')
from process_supervisor import SupervisorClient
private=Path(sys.argv[1]);deadline=time.monotonic()+10
while time.monotonic()<deadline:
    try:
        if SupervisorClient(private/'process-supervisor.sock').status()['alive']:break
    except Exception:pass
    time.sleep(.05)
else:raise SystemExit('FAIL: disposable process supervisor did not start')
PY
fixture_pid=$(cat "$private/fixture.pid")
python3 tests/online-match/tls_frontend.py --dist apps/web/dist \
    --cert "$private/tls.pem" --key "$private/tls.key" \
    --listen-port 9443 --upstream-port 3000 > "$private/tls.log" 2>&1 &
tls_pid=$!
browser_status=0
python3 tests/online-match/browser_acceptance.py --private "$private" \
    --artifacts "$artifacts" --ca "$private/ca.pem" \
    --native-pid "$fixture_pid" --tls-pid "$tls_pid" || browser_status=$?
continuity_status=1
if test "$browser_status" = 0; then
    continuity_status=0
    python3 tests/online-match/continuity_acceptance.py --private "$private" \
        --artifacts "$artifacts" --ca "$private/ca.pem" || continuity_status=$?
fi
# Browser teardown alone cannot release the durable native ownership backend.
# Stop/reap the actual current child before any fresh-fence recovery audit.
python3 - "$private" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, 'tests/online-match')
from process_supervisor import SupervisorClient
reply=SupervisorClient(Path(sys.argv[1])/'process-supervisor.sock').stop()
if reply['alive']:raise SystemExit('FAIL: actual native server remained alive before audit')
PY
wait "$supervisor_pid"
supervisor_pid=''
# Synthetic account identifiers and expected input prefixes stay private.
# A later failure never prevents validation of the already observed main game.
audit_status=1
audit_input_present=0
if test -s "$private/audit-input.json"; then
    audit_input_present=1
    audit_status=0
    "$fixture" audit "$private/audit-input.json" > "$artifacts/durable-verdict.json" || audit_status=$?
fi
continuity_audit_status=0
continuity_audit_count=0
mkdir -p "$artifacts/continuity-audits"
shopt -s nullglob
for input in "$private"/continuity-audit-*.json; do
    label=$(basename "$input" .json)
    # Only fixed script-owned labels form filenames, never routing/account IDs.
    [[ "$label" =~ ^continuity-audit-[a-z-]+$ ]] || { echo 'FAIL: private audit label invalid'; exit 1; }
    "$fixture" audit "$input" > "$artifacts/continuity-audits/$label.json" || continuity_audit_status=$?
    continuity_audit_count=$((continuity_audit_count + 1))
done
python3 tests/online-match/finalize_evidence.py --artifacts "$artifacts" \
    --browser-status "$browser_status" --audit-status "$audit_status" \
    --audit-input-present "$audit_input_present" --continuity-status "$continuity_status" \
    --continuity-audit-status "$continuity_audit_status" --continuity-audit-count "$continuity_audit_count"
