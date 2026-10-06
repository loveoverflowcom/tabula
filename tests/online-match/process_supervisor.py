"""Job-lifetime CI fixture process control, never a network or gameplay service.

Only the supplied already-built online-match-fixture is launched. The private
Unix socket carries four closed commands; status means process liveness only,
not HTTP readiness. Actual HTTPS/browser/PostgreSQL acceptance owns that proof.
PID and native stdout/stderr remain in the enclosing run's private directory.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import socket
import stat
import subprocess
import sys
import time


SOCKET_NAME = "process-supervisor.sock"
PID_NAME = "fixture.pid"
LOG_NAME = "fixture.log"
MESSAGE_LIMIT = 512
IO_TIMEOUT = 2.0
WAIT_TIMEOUT = 5.0
MAX_RESTARTS = 12
MAX_LIFETIME = 1800.0
MODES = {"normal": "serve", "evicted": "serve-evicted", "expired": "serve-expired"}
ERRORS = frozenset({"invalid_arguments", "invalid_request", "invalid_response",
                    "opt_in_absent", "invalid_inputs", "child_alive", "child_not_alive",
                    "kill_failed", "restart_failed", "restart_limit", "stop_failed",
                    "unavailable", "startup_failed"})
STATUS_KEYS = frozenset({"alive", "generation", "kills", "restarts"})
_stop_requested = False


class SupervisorError(Exception):
    """A closed failure class safe to retain without input or exception text."""

    def __init__(self, code):
        self.code = code if code in ERRORS else "unavailable"
        super().__init__(self.code)


def require_opt_in():
    if os.environ.get("CI") != "true" or os.environ.get("TABULA_ONLINE_MATCH_DISPOSABLE") != "1":
        raise SupervisorError("opt_in_absent")


def validate_inputs(fixture, private):
    """Validate trusted CLI paths before starting any child or socket."""
    require_opt_in()
    fixture = Path(fixture)
    private = Path(private)
    try:
        directory = private.lstat()
        executable = fixture.lstat()
        if (not stat.S_ISDIR(directory.st_mode) or directory.st_uid != os.getuid()
                or stat.S_IMODE(directory.st_mode) != 0o700
                or not stat.S_ISREG(executable.st_mode)
                or fixture.name != "online-match-fixture" or not os.access(fixture, os.X_OK)):
            raise SupervisorError("invalid_inputs")
        return fixture.resolve(strict=True), private.resolve(strict=True)
    except OSError:
        raise SupervisorError("invalid_inputs") from None


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise SupervisorError("invalid_request")
        result[key] = value
    return result


def decode_request(raw):
    """Reject unknown fields, duplicate keys, arbitrary modes and oversized JSON."""
    if not raw or len(raw) > MESSAGE_LIMIT:
        raise SupervisorError("invalid_request")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object)
    except (ValueError, UnicodeError, RecursionError):
        raise SupervisorError("invalid_request") from None
    if not isinstance(value, dict) or not isinstance(value.get("command"), str):
        raise SupervisorError("invalid_request")
    command = value["command"]
    if command == "restart":
        if set(value) != {"command", "mode"} or not isinstance(value["mode"], str) or value["mode"] not in MODES:
            raise SupervisorError("invalid_request")
    elif command not in {"status", "kill", "stop"} or set(value) != {"command"}:
        raise SupervisorError("invalid_request")
    return value


def read_message(connection, timeout=IO_TIMEOUT):
    """Read one EOF-delimited message within both byte and elapsed-time bounds."""
    deadline = time.monotonic() + timeout
    raw = bytearray()
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise SupervisorError("invalid_request")
        connection.settimeout(remaining)
        chunk = connection.recv(MESSAGE_LIMIT + 1 - len(raw))
        if not chunk:
            return bytes(raw)
        raw.extend(chunk)
        if len(raw) > MESSAGE_LIMIT:
            raise SupervisorError("invalid_request")


def open_private_log(private):
    flags = os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
    descriptor = os.open(private / LOG_NAME, flags, 0o600)
    try:
        facts = os.fstat(descriptor)
        if (not stat.S_ISREG(facts.st_mode) or facts.st_uid != os.getuid()
                or facts.st_nlink != 1 or stat.S_IMODE(facts.st_mode) != 0o600):
            raise SupervisorError("invalid_inputs")
        return os.fdopen(descriptor, "ab", buffering=0)
    except BaseException:
        os.close(descriptor)
        raise


class FixtureProcess:
    """Own exactly one supplied fixture child; restart never overlaps owners (I-14)."""

    def __init__(self, fixture, private):
        self.fixture, self.private = validate_inputs(fixture, private)
        self.child = None
        self.generation = 0
        self.kills = 0
        self.restarts = 0

    def status(self):
        return {"alive": self.child is not None and self.child.poll() is None,
                "generation": self.generation, "kills": self.kills, "restarts": self.restarts}

    def _write_pid(self):
        temporary = self.private / (PID_NAME + ".new")
        descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
        try:
            with os.fdopen(descriptor, "w") as stream:
                stream.write(str(self.child.pid) + "\n")
            os.replace(temporary, self.private / PID_NAME)
        finally:
            temporary.unlink(missing_ok=True)

    def start(self, mode="normal"):
        require_opt_in()
        if not isinstance(mode, str) or mode not in MODES:
            raise SupervisorError("invalid_request")
        if self.status()["alive"]:
            raise SupervisorError("child_alive")
        if self.generation and self.restarts >= MAX_RESTARTS:
            raise SupervisorError("restart_limit")
        try:
            with open_private_log(self.private) as log:
                # No shell, caller-supplied arguments, credentials or substituted env.
                self.child = subprocess.Popen([str(self.fixture), MODES[mode]],
                                              stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                              close_fds=True)
            if self.child.poll() is not None:
                raise SupervisorError("restart_failed")
            self._write_pid()
        except (OSError, SupervisorError):
            self.stop()
            raise SupervisorError("restart_failed") from None
        self.restarts += int(self.generation > 0)
        self.generation += 1
        return self.status()

    def kill(self):
        require_opt_in()
        if not self.status()["alive"]:
            raise SupervisorError("child_not_alive")
        try:
            # Popen owns the target. No request can supply a PID or a signal.
            self.child.kill()
            outcome = self.child.wait(timeout=WAIT_TIMEOUT)
        except (OSError, subprocess.TimeoutExpired):
            raise SupervisorError("kill_failed") from None
        if outcome != -signal.SIGKILL:
            raise SupervisorError("kill_failed")
        self.kills += 1
        return dict(self.status(), sigkill_reaped=True)

    def stop(self):
        if self.child is not None and self.child.poll() is None:
            try:
                self.child.terminate()
                try:
                    self.child.wait(timeout=WAIT_TIMEOUT)
                except subprocess.TimeoutExpired:
                    self.child.kill()
                    self.child.wait(timeout=WAIT_TIMEOUT)
            except (OSError, subprocess.TimeoutExpired):
                raise SupervisorError("stop_failed") from None
        # A failed second supervisor bind must not remove the first owner's PID.
        if self.child is not None:
            (self.private / PID_NAME).unlink(missing_ok=True)
        return self.status()


def stop_supervisor(signum, frame):
    global _stop_requested
    # Do not interrupt spawn/wait mid-transition: the bounded loop owns cleanup.
    _stop_requested = True


def serve(fixture, private):
    """Run the private control loop until stop, clean signals, parent death or deadline."""
    global _stop_requested
    process = FixtureProcess(fixture, private)
    socket_path = process.private / SOCKET_NAME
    _stop_requested = False
    parent_pid = os.getppid()
    deadline = time.monotonic() + MAX_LIFETIME
    handlers = {}
    bound = False
    try:
        for signum in (signal.SIGINT, signal.SIGTERM):
            handlers[signum] = signal.signal(signum, stop_supervisor)
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
            # Never unlink or take over another supervisor's existing socket.
            listener.bind(str(socket_path))
            bound = True
            os.chmod(socket_path, 0o600)
            listener.listen(1)
            listener.settimeout(0.2)
            process.start()
            while not _stop_requested and os.getppid() == parent_pid and time.monotonic() < deadline:
                try:
                    connection, _ = listener.accept()
                except socket.timeout:
                    continue
                stopping = False
                with connection:
                    try:
                        request = decode_request(read_message(connection))
                        command = request["command"]
                        if command == "status":
                            result = process.status()
                        elif command == "kill":
                            result = process.kill()
                        elif command == "restart":
                            result = process.start(request["mode"])
                        else:
                            result = process.stop()
                            stopping = True
                        response = {"ok": True, "result": result}
                    except SupervisorError as error:
                        response = {"ok": False, "error": error.code}
                    except (OSError, ValueError):
                        response = {"ok": False, "error": "invalid_request"}
                    try:
                        connection.settimeout(IO_TIMEOUT)
                        connection.sendall(json.dumps(response, separators=(",", ":")).encode("ascii"))
                    except OSError:
                        pass
                if stopping:
                    break
    finally:
        try:
            process.stop()
        finally:
            if bound:
                socket_path.unlink(missing_ok=True)
            for signum, previous in handlers.items():
                signal.signal(signum, previous)


class SupervisorClient:
    """Bounded private-socket caller with no arbitrary command, argument or PID API."""

    def __init__(self, path):
        self.path = Path(path)

    def _request(self, command, mode=None):
        request = {"command": command}
        if mode is not None:
            request["mode"] = mode
        raw = json.dumps(request, separators=(",", ":")).encode("ascii")
        decode_request(raw)
        try:
            directory = self.path.parent.lstat()
            endpoint = self.path.lstat()
            if (not stat.S_ISDIR(directory.st_mode) or directory.st_uid != os.getuid()
                    or stat.S_IMODE(directory.st_mode) != 0o700
                    or not stat.S_ISSOCK(endpoint.st_mode) or endpoint.st_uid != os.getuid()
                    or stat.S_IMODE(endpoint.st_mode) != 0o600):
                raise SupervisorError("unavailable")
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
                connection.settimeout(IO_TIMEOUT)
                connection.connect(str(self.path))
                connection.sendall(raw)
                connection.shutdown(socket.SHUT_WR)
                response = json.loads(read_message(connection, 2 * WAIT_TIMEOUT + IO_TIMEOUT).decode("utf-8"),
                                      object_pairs_hook=unique_object)
        except (OSError, ValueError, UnicodeError, RecursionError):
            raise SupervisorError("unavailable") from None
        if not isinstance(response, dict) or type(response.get("ok")) is not bool:
            raise SupervisorError("invalid_response")
        if not response["ok"]:
            if set(response) != {"ok", "error"} or not isinstance(response["error"], str) or response["error"] not in ERRORS:
                raise SupervisorError("invalid_response")
            raise SupervisorError(response["error"])
        result = response.get("result")
        expected = STATUS_KEYS | ({"sigkill_reaped"} if command == "kill" else set())
        if (set(response) != {"ok", "result"} or not isinstance(result, dict) or set(result) != expected
                or type(result["alive"]) is not bool
                or any(type(result[key]) is not int or not 0 <= result[key] <= MAX_RESTARTS + 1
                       for key in ("generation", "kills", "restarts"))
                or result["generation"] != result["restarts"] + 1
                or result["kills"] > result["generation"] or result["restarts"] > MAX_RESTARTS
                or (command == "stop" and result["alive"])
                or (command == "kill" and (result["sigkill_reaped"] is not True or result["alive"]))):
            raise SupervisorError("invalid_response")
        return result

    def status(self):
        return self._request("status")

    def kill(self):
        return self._request("kill")

    def restart(self, mode="normal"):
        return self._request("restart", mode)

    def stop(self):
        return self._request("stop")


class ClosedArgumentParser(argparse.ArgumentParser):
    def error(self, message):
        raise SupervisorError("invalid_arguments")


def main(argv=None):
    parser = ClosedArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--private", type=Path, required=True)
    try:
        arguments = parser.parse_args(argv)
        serve(arguments.fixture, arguments.private)
    except (SupervisorError, OSError, ValueError):
        print("Fixture process supervisor failed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
