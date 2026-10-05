"""Offline metadata/redaction helper checks; never screenshot execution proof."""
import hashlib
from pathlib import Path
import tempfile
import unittest

from capture_evidence import CaptureFailure, MASK_SELECTOR, digest, public_route, source_sha, wasm_builds


class CaptureEvidenceTests(unittest.TestCase):
    def test_routes_drop_all_query_and_fragment_data(self):
        self.assertEqual(public_route("https://localhost:9443/play/local/?match_id=public&untrusted=secret#secret"), "/play/local/")

    def test_foreign_and_unexpected_routes_fail_closed(self):
        for url in ("http://localhost:9443/", "https://foreign.invalid/", "https://localhost:9443/__fixture/enroll", "https://localhost:9443/private"):
            with self.assertRaises(CaptureFailure):
                public_route(url)

    def test_redaction_selector_covers_active_codes_inputs_and_secret_nodes(self):
        for required in ("input", "textarea", "online-code", "data-secret", "data-private", "csrf", "token", "credential"):
            self.assertIn(required, MASK_SELECTOR)

    def test_exact_source_sha_required_without_fallback(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "sha.txt"
            path.write_text("a" * 40 + "\n")
            self.assertEqual(source_sha(path), "a" * 40)
            for wrong in ("", "main", "a" * 39, "A" * 40):
                path.write_text(wrong)
                with self.assertRaises(CaptureFailure):
                    source_sha(path)

    def test_deployed_wasm_provenance_requires_a_separate_nonempty_game_build(self):
        with tempfile.TemporaryDirectory() as folder:
            dist = Path(folder)
            (dist / "shell.wasm").write_bytes(b"synthetic-shell")
            with self.assertRaises(CaptureFailure):
                wasm_builds(dist)
            game = dist / "play/local/game.wasm"
            game.parent.mkdir(parents=True)
            game.write_bytes(b"synthetic-game")
            builds = wasm_builds(dist)
            self.assertEqual(len(builds), 2)
            self.assertEqual(digest(game), hashlib.sha256(b"synthetic-game").hexdigest())
            game.write_bytes(b"")
            with self.assertRaises(CaptureFailure):
                wasm_builds(dist)


if __name__ == "__main__":
    unittest.main()
