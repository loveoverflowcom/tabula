#!/usr/bin/env python3
"""Disposable verified TLS edge for actual local/dev service acceptance only."""
import argparse
import os
from pathlib import Path
import signal
import ssl
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "online-match"))
import tls_frontend as edge

AUTH_PATHS = frozenset(("/api/v1/auth/context", "/api/v1/auth/login",
                       "/api/v1/auth/oidc/callback", "/api/v1/auth/refresh",
                       "/api/v1/auth/logout"))

class ServiceHandler(edge.FixtureHandler):
    def _upstream_port(self):
        path = edge.parse_target(self.path)
        return 3001 if path in AUTH_PATHS or path.startswith("/api/v2/auth/") else 3002

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--cert", type=Path, required=True)
    parser.add_argument("--key", type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get("TABULA_LOCAL_DEV_DISPOSABLE") != "1":
        raise ValueError("explicit disposable service acceptance required")
    root, cert, key = edge.validate_inputs(args.dist, args.cert, args.key)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(str(cert), str(key))
    with edge.FixtureServer((edge.LOOPBACK, 8444), ServiceHandler) as server:
        server.dist = root
        server.socket = context.wrap_socket(server.socket, server_side=True)
        for number in (signal.SIGINT, signal.SIGTERM):
            signal.signal(number, edge.stop_frontend)
        try:
            server.serve_forever(poll_interval=.2)
        except edge.StopFrontend:
            pass

if __name__ == "__main__":
    main()
