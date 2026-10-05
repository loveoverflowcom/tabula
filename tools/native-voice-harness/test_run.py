import base64
import hashlib
import hmac
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("native_voice_harness", Path(__file__).with_name("run.py"))
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


def decode(value):
    return json.loads(base64.urlsafe_b64decode(value + "=" * (-len(value) % 4)))


class FixtureTest(unittest.TestCase):
    def test_two_distinct_audio_only_participants_fixed_room_expiry_and_valid_signature(self):
        # Synthetic fixture test only: does not connect an SDK or run an SFU.
        a = harness.fixture("synthetic-key", "synthetic-secret", "client-a", 100, "ws://127.0.0.1:7880")
        b = harness.fixture("synthetic-key", "synthetic-secret", "client-b", 100, "ws://127.0.0.1:7880")
        header, body, signature = a["token"].split(".")
        claims = decode(body)
        self.assertEqual("HS256", decode(header)["alg"])
        self.assertEqual("client-a", claims["sub"])
        self.assertEqual("client-b", decode(b["token"].split(".")[1])["sub"])
        self.assertEqual(700, claims["exp"])
        self.assertEqual(harness.SCOPE, claims["video"]["room"])
        self.assertEqual(["microphone"], claims["video"]["canPublishSources"])
        self.assertFalse(claims["video"]["canPublishData"])
        self.assertEqual(signature, harness.b64(hmac.new(b"synthetic-secret", f"{header}.{body}".encode(), hashlib.sha256).digest()))
        self.assertNotIn("secret", a)

    def test_fixture_files_private_and_existing_destination_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.json"
            harness.write_private(path, "synthetic")
            self.assertEqual(0o600, path.stat().st_mode & 0o777)
            with self.assertRaises(FileExistsError):
                harness.write_private(path, "replacement")
            self.assertEqual("synthetic", path.read_text())


if __name__ == "__main__":
    unittest.main()
