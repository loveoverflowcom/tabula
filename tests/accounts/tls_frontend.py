#!/usr/bin/env python3
"""Disposable HTTPS/WSS edge for ADR-0043 acceptance, without auth shortcuts.

Reuse the bounded opaque HTTP/static fixture edge. Only the fixed social socket
gets a transparent tunnel; the real native adapter owns cookie/Origin/subprotocol
authentication and all frames. No request, credential or frame data is logged.
"""

import argparse
import os
from pathlib import Path
import select
import signal
import socket
import ssl
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "online-match"))
import tls_frontend as http_edge

SOCKET_PATH = "/api/v2/lobby/ws"
SOCKET_LIFETIME = 120
TUNNEL_BYTES = 16 * 1024 * 1024


class AccountHandler(http_edge.FixtureHandler):
    """Tunnel one bounded social stream without interpreting its private frames."""

    def _proxy(self, headers):
        if self.path != SOCKET_PATH:
            return super()._proxy(headers)
        if self.command != "GET" or http_edge.request_body_length(headers):
            raise http_edge.RejectedRequest(400)
        upgrades = [value for name, value in headers if name.lower() == "upgrade"]
        if len(upgrades) != 1 or upgrades[0].lower() != "websocket":
            raise http_edge.RejectedRequest(400)
        clean = http_edge.filtered_headers(headers)
        with socket.create_connection(
                (http_edge.LOOPBACK, self.server.upstream_port), timeout=5) as upstream:
            request = [f"GET {SOCKET_PATH} HTTP/1.1"]
            request.extend(f"{name}: {value}" for name, value in clean)
            request.extend(["Upgrade: websocket", "Connection: Upgrade", "", ""])
            upstream.sendall("\r\n".join(request).encode("latin-1"))
            header = bytearray()
            deadline = time.monotonic() + 5
            while not header.endswith(b"\r\n\r\n"):
                if len(header) >= 16384 or time.monotonic() >= deadline:
                    raise http_edge.RejectedRequest(502)
                byte = upstream.recv(1)
                if not byte:
                    raise http_edge.RejectedRequest(502)
                header.extend(byte)
            lines = header.decode("latin-1").split("\r\n")
            status = lines[0].split(" ", 2)
            if len(status) < 2 or status[0] != "HTTP/1.1":
                raise http_edge.RejectedRequest(502)
            if status[1] != "101":
                allowed = {"400", "401", "403", "404", "409", "426", "503"}
                raise http_edge.RejectedRequest(int(status[1]) if status[1] in allowed else 502)
            reply_headers = []
            for line in lines[1:]:
                if not line:
                    continue
                name, separator, value = line.partition(":")
                if not separator:
                    raise http_edge.RejectedRequest(502)
                reply_headers.append((name, value.strip()))
            clean_reply = http_edge.filtered_headers(reply_headers)
            self._response_started = True
            self.close_connection = True
            self.send_response_only(101)
            for name, value in clean_reply:
                self.send_header(name, value)
            self.send_header("Upgrade", "websocket")
            self.send_header("Connection", "Upgrade")
            self.end_headers()
            self.wfile.flush()
            deadline = time.monotonic() + SOCKET_LIFETIME
            transferred = 0
            while time.monotonic() < deadline and transferred < TUNNEL_BYTES:
                readable, _, _ = select.select([self.connection, upstream], [], [], 0.25)
                if self.connection.pending() and self.connection not in readable:
                    readable.append(self.connection)
                for source in readable:
                    chunk = source.recv(16384)
                    if not chunk:
                        return
                    transferred += len(chunk)
                    target = upstream if source is self.connection else self.connection
                    target.sendall(chunk)


def serve(dist, cert, key, listen_port, upstream_port):
    if os.environ.get("TABULA_ACCOUNTS_DISPOSABLE") != "1":
        raise ValueError("Explicit disposable account acceptance scope required")
    root, certificate, private_key = http_edge.validate_inputs(dist, cert, key)
    if listen_port != 8444 or upstream_port != 3001:
        raise ValueError("Fixed account acceptance ports required")
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(str(certificate), str(private_key))
    with http_edge.FixtureServer((http_edge.LOOPBACK, listen_port), AccountHandler) as server:
        server.dist = root
        server.upstream_port = upstream_port
        server.socket = context.wrap_socket(server.socket, server_side=True)
        previous = {}
        try:
            for number in (signal.SIGINT, signal.SIGTERM):
                previous[number] = signal.signal(number, http_edge.stop_frontend)
            server.serve_forever(poll_interval=0.2)
        except http_edge.StopFrontend:
            pass
        finally:
            for number, handler in previous.items():
                signal.signal(number, handler)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", required=True, type=Path)
    parser.add_argument("--cert", required=True, type=Path)
    parser.add_argument("--key", required=True, type=Path)
    args = parser.parse_args()
    try:
        serve(args.dist, args.cert, args.key, 8444, 3001)
    except (OSError, ValueError, ssl.SSLError):
        print("Account TLS edge startup failed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
