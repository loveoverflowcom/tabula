#!/usr/bin/env python3
"""Isolated ephemeral LiveKit dev SFU + two native fixtures. No production authority."""
import argparse
import base64
import hashlib
import hmac
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import tempfile
import time

SCOPE = "tabula-native-voice-dev"


def b64(value):
    return base64.urlsafe_b64encode(value).decode("ascii").rstrip("=")


def fixture(key, secret, participant, now, endpoint):
    payload = {"iss": key, "sub": participant, "nbf": now - 5, "exp": now + 600,
               "video": {"roomJoin": True, "room": SCOPE, "canSubscribe": True,
                         "canPublish": True, "canPublishSources": ["microphone"],
                         "canPublishData": False}}
    unsigned = b64(json.dumps({"alg": "HS256", "typ": "JWT"}, separators=(",", ":")).encode()) + "." + b64(json.dumps(payload, separators=(",", ":")).encode())
    token = unsigned + "." + b64(hmac.new(secret.encode(), unsigned.encode(), hashlib.sha256).digest())
    return {"v": 1, "scope": SCOPE, "endpoint": endpoint, "token": token,
            "expiresAt": now + 600, "canPublish": True}


def write_private(path, contents):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as file:
        file.write(contents)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--android-emulator", action="store_true", help="Use 10.0.2.2 instead of simulator localhost")
    args = parser.parse_args()
    executable = shutil.which("livekit-server")
    if executable is None:
        parser.error("official livekit-server is not installed; no server or fixture was started")
    address = "10.0.2.2" if args.android_emulator else "127.0.0.1"
    endpoint = f"ws://{address}:7880"
    key = "tabula_dev_" + secrets.token_hex(12)
    secret = secrets.token_urlsafe(48)
    with tempfile.TemporaryDirectory(prefix="tabula-native-voice-") as directory:
        root = Path(directory)
        config = root / "livekit.yaml"
        # Bind signaling and media only to the host loopback; never provision hosted credentials.
        write_private(config, f"port: 7880\nbind_addresses: ['127.0.0.1']\nrtc:\n  tcp_port: 7881\n  port_range_start: 50000\n  port_range_end: 50100\n  node_ip: {address}\n  use_external_ip: false\nkeys:\n  {key}: {secret}\nlogging:\n  level: error\n")
        now = int(time.time())
        for participant in ("client-a", "client-b"):
            write_private(root / f"{participant}.json", json.dumps(fixture(key, secret, participant, now, endpoint)))
        print(f"Ephemeral local harness directory: {root}")
        print("Native fixtures client-a.json/client-b.json expire in ten minutes. Never commit them.")
        print("Copy the chosen fixture to ignored mobile/voice-dev-grant.json before a DEBUG native build.")
        print("Two distinct client identities are required; fixture generation alone proves no connection.")
        process = subprocess.Popen([executable, "--config", str(config)], stdout=subprocess.DEVNULL)
        try:
            process.wait()
            if process.returncode:
                raise SystemExit(f"SFU startup/execution failed (exit {process.returncode}); no integration pass")
        except KeyboardInterrupt:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait()
        # Temporary directory removes ephemeral keys and grant fixtures on every normal exit.


if __name__ == "__main__":
    main()
