"""Offline process-control helper tests, never native/browser/PG acceptance evidence."""
from contextlib import redirect_stderr, redirect_stdout
import io
import json
import errno
import os
from pathlib import Path
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock
from types import SimpleNamespace

import process_supervisor as supervisor


OPT_IN = {"CI": "true", "TABULA_ONLINE_MATCH_DISPOSABLE": "1"}


class ClosedControlTests(unittest.TestCase):
    def test_four_closed_commands_and_three_restart_modes(self):
        for command in ("status", "kill", "stop"):
            value = {"command": command}
            self.assertEqual(supervisor.decode_request(json.dumps(value).encode()), value)
        for mode in ("normal", "evicted", "expired"):
            value = {"command": "restart", "mode": mode}
            self.assertEqual(supervisor.decode_request(json.dumps(value).encode()), value)

    def test_unknown_fields_modes_targets_and_duplicate_keys_are_rejected(self):
        invalid = [b"", b"\xff", b"[]", b"null", b"{}", b'{"command":true}',
                   b'{"command":"kill","pid":1}', b'{"command":"kill","signal":15}',
                   b'{"command":"restart","argv":["synthetic-secret"]}',
                   b'{"command":"restart","mode":"serve"}', b'{"command":"restart","mode":[]}',
                   b'{"command":"restart"}', b'{"command":"status","path":"synthetic-secret"}',
                   b'{"command":"exec"}', b'{"command":"status","command":"kill"}',
                   b'{"command":"status"} {"command":"kill"}', b" " * (supervisor.MESSAGE_LIMIT + 1)]
        for raw in invalid:
            with self.subTest(raw=raw), self.assertRaises(supervisor.SupervisorError) as raised:
                supervisor.decode_request(raw)
            self.assertEqual(raised.exception.code, "invalid_request")
            self.assertNotIn("synthetic-secret", str(raised.exception))

    def test_message_byte_limit_prevents_unbounded_read(self):
        connection = mock.Mock()
        connection.recv.return_value = b"x" * (supervisor.MESSAGE_LIMIT + 1)
        with self.assertRaises(supervisor.SupervisorError):
            supervisor.read_message(connection)
        connection.recv.assert_called_once_with(supervisor.MESSAGE_LIMIT + 1)

    def test_elapsed_deadline_cannot_be_extended_by_partial_input(self):
        connection = mock.Mock()
        connection.recv.return_value = b"x"
        with mock.patch.object(supervisor.time, "monotonic", side_effect=[0, 0, supervisor.IO_TIMEOUT + 1]):
            with self.assertRaises(supervisor.SupervisorError):
                supervisor.read_message(connection)
        connection.recv.assert_called_once()

    def test_cli_errors_and_startup_errors_are_sanitized(self):
        capture = io.StringIO()
        with redirect_stderr(capture), redirect_stdout(capture):
            self.assertEqual(supervisor.main(["--unexpected=synthetic-secret"]), 1)
            with mock.patch.object(supervisor, "serve", side_effect=OSError("synthetic-secret")):
                self.assertEqual(supervisor.main(["--fixture", "x", "--private", "y"]), 1)
        self.assertEqual(capture.getvalue(), "Fixture process supervisor failed\n" * 2)

    def client_response(self, response, method="status"):
        client = supervisor.SupervisorClient("/synthetic-private/process-supervisor.sock")
        connection = mock.MagicMock()
        connection.__enter__.return_value = connection
        connection.recv.side_effect = [json.dumps(response).encode(), b""]
        directory = SimpleNamespace(st_mode=stat.S_IFDIR | 0o700, st_uid=os.getuid())
        endpoint = SimpleNamespace(st_mode=stat.S_IFSOCK | 0o600, st_uid=os.getuid())
        with mock.patch.object(Path, "lstat", side_effect=[directory, endpoint]), \
             mock.patch.object(supervisor.socket, "socket", return_value=connection) as socket_factory:
            result = getattr(client, method)()
        socket_factory.assert_called_once_with(socket.AF_UNIX, socket.SOCK_STREAM)
        connection.connect.assert_called_once_with(str(client.path))
        connection.shutdown.assert_called_once_with(socket.SHUT_WR)
        request = json.loads(connection.sendall.call_args.args[0])
        self.assertEqual(request, {"command": method})
        return result

    def test_client_accepts_only_closed_status_and_confirmed_kill_outcome(self):
        status = {"alive": True, "generation": 1, "kills": 0, "restarts": 0}
        self.assertEqual(self.client_response({"ok": True, "result": status}), status)
        killed = dict(status, alive=False, kills=1, sigkill_reaped=True)
        self.assertEqual(self.client_response({"ok": True, "result": killed}, "kill"), killed)

    def test_client_rejects_pid_secret_extra_field_and_false_kill_success(self):
        status = {"alive": True, "generation": 1, "kills": 0, "restarts": 0}
        invalid = [({}, "status"), ({"ok": 1, "result": status}, "status"),
                   ({"ok": True, "result": dict(status, pid=123)}, "status"),
                   ({"ok": True, "result": dict(status, secret="synthetic-secret")}, "status"),
                   ({"ok": True, "result": dict(status, generation=True)}, "status"),
                   ({"ok": True, "result": dict(status, generation=2)}, "status"),
                   ({"ok": True, "result": dict(status, sigkill_reaped=True)}, "kill"),
                   ({"ok": True, "result": dict(status, alive=False, sigkill_reaped=False)}, "kill"),
                   ({"ok": True, "result": status}, "stop"),
                   ({"ok": False, "error": "synthetic-secret"}, "status")]
        for response, method in invalid:
            with self.subTest(response=response), self.assertRaises(supervisor.SupervisorError) as raised:
                self.client_response(response, method)
            self.assertEqual(raised.exception.code, "invalid_response")
            self.assertNotIn("synthetic-secret", str(raised.exception))

    def test_closed_client_failure_never_echoes_native_error_or_inputs(self):
        with self.assertRaises(supervisor.SupervisorError) as raised:
            self.client_response({"ok": False, "error": "child_alive"})
        self.assertEqual(raised.exception.code, "child_alive")

    def test_signal_handler_sets_cleanup_flag_without_interrupting_process_transition(self):
        with mock.patch.object(supervisor, "_stop_requested", False):
            supervisor.stop_supervisor(signal.SIGTERM, None)
            self.assertTrue(supervisor._stop_requested)

    def test_control_loop_uses_unix_socket_closed_dispatch_and_cleanup(self):
        process = mock.Mock(private=Path("/synthetic-private"))
        status = {"alive": True, "generation": 1, "kills": 0, "restarts": 0}
        process.status.return_value = status
        process.stop.return_value = dict(status, alive=False)
        listener = mock.MagicMock()
        listener.__enter__.return_value = listener
        connection = mock.MagicMock()
        connection.__enter__.return_value = connection
        connection.recv.side_effect = [b'{"command":"stop"}', b""]
        listener.accept.return_value = (connection, "unused")
        with mock.patch.object(supervisor, "FixtureProcess", return_value=process), \
             mock.patch.object(supervisor.socket, "socket", return_value=listener) as socket_factory, \
             mock.patch.object(supervisor.os, "chmod") as chmod, \
             mock.patch.object(supervisor.signal, "signal", return_value="old") as signals, \
             mock.patch.object(Path, "unlink") as unlink:
            supervisor.serve("fixture", "private")
        socket_factory.assert_called_once_with(socket.AF_UNIX, socket.SOCK_STREAM)
        listener.bind.assert_called_once_with("/synthetic-private/process-supervisor.sock")
        chmod.assert_called_once_with(Path("/synthetic-private/process-supervisor.sock"), 0o600)
        process.start.assert_called_once_with()
        self.assertEqual(process.stop.call_count, 2)
        self.assertEqual(json.loads(connection.sendall.call_args.args[0]),
                         {"ok": True, "result": dict(status, alive=False)})
        unlink.assert_called_once_with(missing_ok=True)
        self.assertEqual(signals.call_args_list,
                         [mock.call(signal.SIGINT, supervisor.stop_supervisor),
                          mock.call(signal.SIGTERM, supervisor.stop_supervisor),
                          mock.call(signal.SIGINT, "old"), mock.call(signal.SIGTERM, "old")])

    def test_bind_failure_does_not_remove_an_existing_supervisor_socket(self):
        process = mock.Mock(private=Path("/synthetic-private"))
        listener = mock.MagicMock()
        listener.__enter__.return_value = listener
        listener.bind.side_effect = OSError("synthetic-private")
        with mock.patch.object(supervisor, "FixtureProcess", return_value=process), \
             mock.patch.object(supervisor.socket, "socket", return_value=listener), \
             mock.patch.object(supervisor.signal, "signal"), mock.patch.object(Path, "unlink") as unlink:
            with self.assertRaises(OSError):
                supervisor.serve("fixture", "private")
        process.start.assert_not_called()
        process.stop.assert_called_once_with()
        unlink.assert_not_called()


@unittest.skipUnless(os.name == "posix" and hasattr(socket, "AF_UNIX"), "private POSIX process helper")
class ProcessFixtureTestCase(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tabula-process-helper-")
        self.addCleanup(self.temporary.cleanup)
        self.private = Path(self.temporary.name)
        self.fixture = self.private / "online-match-fixture"
        # Safe sleep helper, NOT the native server or actual acceptance fixture.
        self.fixture.write_text(f"#!{sys.executable}\n"
                                "import os, pathlib, sys, time\n"
                                "assert os.environ['CI'] == 'true'\n"
                                "assert os.environ['TABULA_ONLINE_MATCH_DISPOSABLE'] == '1'\n"
                                "with (pathlib.Path(__file__).parent / 'helper-modes').open('a') as stream:\n"
                                "    stream.write(sys.argv[1] + '\\n')\n"
                                "print('synthetic-private-stdout', flush=True)\n"
                                "print('synthetic-private-stderr', file=sys.stderr, flush=True)\n"
                                "time.sleep(60)\n")
        self.fixture.chmod(0o700)
        self.environment = mock.patch.dict(os.environ, OPT_IN)
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def process(self):
        process = supervisor.FixtureProcess(self.fixture, self.private)
        self.addCleanup(process.stop)
        return process

    def eventually(self, predicate):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(0.02)
        self.fail("bounded helper condition did not occur")


class ProcessHelperTests(ProcessFixtureTestCase):
    def test_explicit_opt_in_required_before_spawn_or_socket(self):
        for environment in ({}, {"CI": "1", "TABULA_ONLINE_MATCH_DISPOSABLE": "1"},
                            {"CI": "true"}, {"TABULA_ONLINE_MATCH_DISPOSABLE": "1"}):
            with self.subTest(environment=environment), mock.patch.dict(os.environ, environment, clear=True), \
                 mock.patch.object(supervisor.subprocess, "Popen") as spawn, \
                 mock.patch.object(supervisor.socket, "socket") as listener:
                with self.assertRaises(supervisor.SupervisorError) as raised:
                    supervisor.serve(self.fixture, self.private)
                self.assertEqual(raised.exception.code, "opt_in_absent")
                spawn.assert_not_called()
                listener.assert_not_called()

    def test_wrong_name_non_executable_symlink_or_public_directory_is_rejected(self):
        wrong_name = self.private / "other-executable"
        wrong_name.write_text("synthetic")
        wrong_name.chmod(0o700)
        linked = self.private / "linked"
        linked.symlink_to(self.private, target_is_directory=True)
        for fixture, private in ((wrong_name, self.private), (self.fixture, linked),
                                 (self.private / "missing", self.private)):
            with self.subTest(fixture=fixture, private=private), self.assertRaises(supervisor.SupervisorError):
                supervisor.validate_inputs(fixture, private)
        self.fixture.chmod(0o600)
        with self.assertRaises(supervisor.SupervisorError):
            supervisor.validate_inputs(self.fixture, self.private)
        self.fixture.chmod(0o700)
        self.private.chmod(0o755)
        with self.assertRaises(supervisor.SupervisorError):
            supervisor.validate_inputs(self.fixture, self.private)

    def test_fixed_argv_and_inherited_environment_with_private_output(self):
        process = self.process()
        fake = mock.Mock(pid=123)
        fake.poll.return_value = None
        with mock.patch.object(supervisor.subprocess, "Popen", return_value=fake) as spawn:
            process.start()
        self.assertEqual(spawn.call_args.args, ([str(self.fixture), "serve"],))
        kwargs = spawn.call_args.kwargs
        self.assertNotIn("env", kwargs)
        self.assertNotIn("shell", kwargs)
        self.assertIs(kwargs["stdout"], kwargs["stderr"])
        self.assertEqual(kwargs["stdin"], subprocess.DEVNULL)
        self.assertTrue(kwargs["close_fds"])
        self.assertTrue(kwargs["stdout"].closed)
        self.assertEqual((self.private / supervisor.PID_NAME).read_text(), "123\n")
        for filename in (supervisor.PID_NAME, supervisor.LOG_NAME):
            self.assertEqual(stat.S_IMODE((self.private / filename).stat().st_mode), 0o600)
        # Mock is not a real child, so make cleanup observe its completed state.
        fake.poll.return_value = 0

    def test_real_helper_sigkill_is_reaped_and_restart_has_no_overlapping_owner(self):
        process = self.process()
        self.assertEqual(process.start(), {"alive": True, "generation": 1, "kills": 0, "restarts": 0})
        first = process.child
        first_pid = first.pid
        with self.assertRaises(supervisor.SupervisorError) as raised:
            process.start("evicted")
        self.assertEqual(raised.exception.code, "child_alive")
        result = process.kill()
        self.assertEqual(result, {"alive": False, "generation": 1, "kills": 1,
                                  "restarts": 0, "sigkill_reaped": True})
        self.assertEqual(first.returncode, -signal.SIGKILL)
        with self.assertRaises(ProcessLookupError):
            os.kill(first_pid, 0)
        with self.assertRaises(supervisor.SupervisorError) as raised:
            process.kill()
        self.assertEqual(raised.exception.code, "child_not_alive")
        self.assertEqual(process.start("evicted"), {"alive": True, "generation": 2, "kills": 1, "restarts": 1})
        self.assertNotEqual(process.child.pid, first_pid)
        self.assertEqual((self.private / supervisor.PID_NAME).read_text().strip(), str(process.child.pid))
        self.assertFalse(process.stop()["alive"])
        self.assertFalse((self.private / supervisor.PID_NAME).exists())
        self.assertEqual(set(result), supervisor.STATUS_KEYS | {"sigkill_reaped"})

    def test_all_three_modes_are_fixed_and_logged_privately(self):
        process = self.process()
        for index, mode in enumerate(("normal", "evicted", "expired"), 1):
            process.start(mode)
            self.eventually(lambda: (self.private / "helper-modes").exists()
                            and len((self.private / "helper-modes").read_text().splitlines()) == index)
            process.kill()
        self.assertEqual((self.private / "helper-modes").read_text().splitlines(),
                         ["serve", "serve-evicted", "serve-expired"])
        self.assertIn("synthetic-private-stdout", (self.private / supervisor.LOG_NAME).read_text())
        self.assertIn("synthetic-private-stderr", (self.private / supervisor.LOG_NAME).read_text())
        self.assertNotIn("synthetic-private", json.dumps(process.status()))

    def test_symlink_and_public_log_fail_before_spawn(self):
        outside = self.private / "synthetic-outside"
        outside.write_text("unchanged")
        log = self.private / supervisor.LOG_NAME
        log.symlink_to(outside)
        process = self.process()
        with mock.patch.object(supervisor.subprocess, "Popen") as spawn:
            with self.assertRaises(supervisor.SupervisorError):
                process.start()
            spawn.assert_not_called()
        self.assertEqual(outside.read_text(), "unchanged")
        log.unlink()
        log.write_text("synthetic")
        log.chmod(0o644)
        with self.assertRaises(supervisor.SupervisorError):
            process.start()

    def test_failed_exec_and_failed_pid_write_leave_no_live_child(self):
        process = self.process()
        with mock.patch.object(supervisor.subprocess, "Popen", side_effect=OSError("synthetic-secret")):
            with self.assertRaises(supervisor.SupervisorError) as raised:
                process.start()
        self.assertEqual(raised.exception.code, "restart_failed")
        self.assertFalse(process.status()["alive"])
        with mock.patch.object(process, "_write_pid", side_effect=OSError("synthetic-secret")):
            with self.assertRaises(supervisor.SupervisorError):
                process.start()
        self.assertIsNotNone(process.child.returncode)
        self.assertFalse(process.status()["alive"])

    def test_clean_stop_escalates_only_its_managed_child_after_bounded_wait(self):
        process = self.process()
        fake = mock.Mock()
        fake.poll.return_value = None
        fake.wait.side_effect = [subprocess.TimeoutExpired("synthetic", 1), -signal.SIGKILL]
        process.child = fake
        process.stop()
        self.assertEqual(fake.method_calls, [mock.call.poll(), mock.call.terminate(),
                                            mock.call.wait(timeout=supervisor.WAIT_TIMEOUT), mock.call.kill(),
                                            mock.call.wait(timeout=supervisor.WAIT_TIMEOUT), mock.call.poll()])
        process.child = None

    def test_incorrect_kill_outcome_is_never_reported_as_success(self):
        process = self.process()
        fake = mock.Mock()
        fake.poll.return_value = None
        fake.wait.return_value = 0
        process.child = fake
        with self.assertRaises(supervisor.SupervisorError) as raised:
            process.kill()
        self.assertEqual(raised.exception.code, "kill_failed")
        self.assertEqual(process.kills, 0)
        fake.poll.return_value = 0

    def test_restart_count_is_bounded(self):
        process = self.process()
        process.generation = supervisor.MAX_RESTARTS + 1
        process.restarts = supervisor.MAX_RESTARTS
        with mock.patch.object(supervisor.subprocess, "Popen") as spawn:
            with self.assertRaises(supervisor.SupervisorError) as raised:
                process.start()
        self.assertEqual(raised.exception.code, "restart_limit")
        spawn.assert_not_called()

    def test_non_owner_cleanup_never_removes_another_supervisors_pid(self):
        process = self.process()
        pid = self.private / supervisor.PID_NAME
        pid.write_text("123\n")
        process.stop()
        self.assertEqual(pid.read_text(), "123\n")


class SocketHelperTests(ProcessFixtureTestCase):
    @classmethod
    def setUpClass(cls):
        # Explicit capability blocker only. Native acceptance never uses this skip.
        with tempfile.TemporaryDirectory(prefix="tabula-unix-helper-") as private:
            try:
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
                    listener.bind(str(Path(private) / "probe.sock"))
            except OSError as error:
                if error.errno in (errno.EPERM, errno.EACCES) and os.environ.get("CI") != "true":
                    raise unittest.SkipTest("BLOCKED: executor denies private AF_UNIX socket") from None
                raise

    def launch_supervisor(self):
        child = subprocess.Popen([sys.executable, str(Path(supervisor.__file__)),
                                  "--fixture", str(self.fixture), "--private", str(self.private)],
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        def cleanup():
            if child.poll() is None:
                child.terminate()
            child.communicate(timeout=15)
        self.addCleanup(cleanup)
        client = supervisor.SupervisorClient(self.private / supervisor.SOCKET_NAME)
        def ready():
            if child.poll() is not None:
                self.fail("helper supervisor exited during setup")
            try:
                return client.status()["alive"]
            except supervisor.SupervisorError:
                return False
        self.eventually(ready)
        return child, client

    def test_private_socket_client_controls_real_helper_and_closes_on_stop(self):
        child, client = self.launch_supervisor()
        endpoint = self.private / supervisor.SOCKET_NAME
        self.assertEqual(stat.S_IMODE(endpoint.stat().st_mode), 0o600)
        self.assertEqual(client.status(), {"alive": True, "generation": 1, "kills": 0, "restarts": 0})
        self.assertTrue(client.kill()["sigkill_reaped"])
        self.assertFalse(client.status()["alive"])
        self.assertTrue(client.restart("expired")["alive"])
        with self.assertRaises(supervisor.SupervisorError):
            client.restart("synthetic-secret")
        self.assertFalse(client.stop()["alive"])
        stdout, stderr = child.communicate(timeout=15)
        self.assertEqual(child.returncode, 0)
        self.assertEqual((stdout, stderr), (b"", b""))
        self.assertFalse(endpoint.exists())
        self.assertFalse((self.private / supervisor.PID_NAME).exists())

    def test_malformed_private_socket_request_cannot_kill_or_target_another_pid(self):
        child, client = self.launch_supervisor()
        before = client.status()
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.settimeout(5)
            connection.connect(str(self.private / supervisor.SOCKET_NAME))
            connection.sendall(b'{"command":"kill","pid":1,"secret":"synthetic-secret"}')
            connection.shutdown(socket.SHUT_WR)
            raw = supervisor.read_message(connection)
        self.assertEqual(json.loads(raw), {"ok": False, "error": "invalid_request"})
        self.assertEqual(client.status(), before)
        self.assertNotIn(b"synthetic-secret", raw)
        client.stop()
        child.communicate(timeout=15)

    def test_clean_supervisor_sigterm_terminates_and_reaps_real_helper(self):
        child, client = self.launch_supervisor()
        helper_pid = int((self.private / supervisor.PID_NAME).read_text())
        child.terminate()
        stdout, stderr = child.communicate(timeout=15)
        self.assertEqual(child.returncode, 0)
        self.assertEqual((stdout, stderr), (b"", b""))
        with self.assertRaises(ProcessLookupError):
            os.kill(helper_pid, 0)
        self.assertFalse((self.private / supervisor.SOCKET_NAME).exists())


if __name__ == "__main__":
    unittest.main()
