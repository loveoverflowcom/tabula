#!/usr/bin/env bash
# Run an acceptance command with a job-lifetime real Kanidm, never a live IDP.
set -euo pipefail
umask 077

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
artifacts="$root/verification/kanidm-oidc-artifacts"
mkdir -p "$artifacts"
private=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/tabula-kanidm.XXXXXXXX")
container="tabula-kanidm-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}-$$"
image='docker.io/kanidm/server@sha256:d87475bf9c9cfd24872d8b25957c9fc13fc090ace09ddc37c3b25fe394b397ac'

cleanup() {
    local result=$?
    trap - EXIT INT TERM
    docker rm --force "$container" >/dev/null 2>&1 || true
    rm -rf -- "$private"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

for tool in docker openssl python3; do
    command -v "$tool" >/dev/null || { echo "Required disposable acceptance tool is absent: $tool" >&2; exit 1; }
done
test "$#" -gt 0 || { echo 'An acceptance command is required' >&2; exit 1; }
docker info >/dev/null
docker pull --platform linux/amd64 "$image"
version=$(docker run --rm --platform linux/amd64 "$image" /sbin/kanidmd version)
test "$version" = 'kanidmd 1.11.2' || { echo 'Pinned provider has an unexpected version' >&2; exit 1; }
printf '%s\n' "$version" > "$artifacts/provider-version.txt"
docker image inspect "$image" --format '{{json .RepoDigests}}' > "$artifacts/provider-image-digests.json"
printf '%s\n' "$image" > "$artifacts/provider-image.txt"
git -C "$root" rev-parse HEAD > "$artifacts/tabula-sha.txt"

mkdir "$private/data"
# Trust is client-scoped. No global CA installation, insecure TLS switch,
# persistent credential store, hosted account, or external OAuth grant.
openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 1 \
    -subj '/CN=Tabula disposable Kanidm acceptance CA' \
    -addext 'basicConstraints=critical,CA:TRUE' \
    -addext 'keyUsage=critical,keyCertSign,cRLSign' \
    -keyout "$private/ca.key" -out "$private/ca.pem" >/dev/null 2>&1
openssl req -new -newkey rsa:2048 -nodes -sha256 -subj '/CN=localhost' \
    -keyout "$private/data/tls.key" -out "$private/leaf.csr" >/dev/null 2>&1
cat > "$private/leaf.ext" <<'EOF'
subjectAltName=DNS:localhost,IP:127.0.0.1
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
EOF
openssl x509 -req -sha256 -days 1 -in "$private/leaf.csr" \
    -CA "$private/ca.pem" -CAkey "$private/ca.key" -CAcreateserial \
    -extfile "$private/leaf.ext" -out "$private/data/leaf.pem" >/dev/null 2>&1
cat "$private/data/leaf.pem" "$private/ca.pem" > "$private/data/tls.pem"
openssl verify -CAfile "$private/ca.pem" -verify_hostname localhost \
    "$private/data/leaf.pem" >/dev/null
cat > "$private/data/server.toml" <<'EOF'
version = "2"
bindaddress = "0.0.0.0:8443"
db_path = "/data/kanidm.db"
tls_chain = "/data/tls.pem"
tls_key = "/data/tls.key"
domain = "localhost"
origin = "https://localhost:8443"
EOF
# Validate with this exact pinned binary before starting a listener. Raw output
# stays inside the private directory and is reduced to closed error categories.
if ! docker run --rm --platform linux/amd64 --volume "$private/data:/data" \
    "$image" /sbin/kanidmd -c /data/server.toml configtest \
    > "$private/configtest-output" 2>&1; then
    python3 "$root/tests/kanidm/provider.py" diagnose-config \
        --input "$private/configtest-output" --artifacts "$artifacts"
    exit 1
fi
docker run --detach --platform linux/amd64 --name "$container" \
    --label org.tabula.purpose=disposable-kanidm-acceptance \
    --publish 127.0.0.1:8443:8443 --volume "$private/data:/data" \
    "$image" /sbin/kanidmd -c /data/server.toml server >/dev/null
python3 "$root/tests/kanidm/provider.py" ready --ca "$private/ca.pem" \
    --container "$container" --artifacts "$artifacts"
export TABULA_KANIDM_DISPOSABLE=1
python3 "$root/tests/kanidm/provider.py" bootstrap --container "$container" \
    --ca "$private/ca.pem" --config "$private/config.json" --artifacts "$artifacts"
export TABULA_KANIDM_TEST_CONFIG="$private/config.json"
export TABULA_KANIDM_TEST_HELPER="$root/tests/kanidm/provider.py"
# Both Rust and Python must opt into this exact test CA; production TLS stays strict.
export RUST_LOG=off
"$@"
printf '%s\n' 'PASS: actual Kanidm OIDC + isolated Tabula HTTP-route acceptance' > "$artifacts/result.txt"
