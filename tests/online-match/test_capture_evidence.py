"""Offline metadata/redaction helper checks; never screenshot execution proof."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from types import SimpleNamespace

from capture_evidence import CaptureFailure, MASK_SELECTOR, checkout_provenance, deployed_documents, digest, public_route, source_sha, wasm_builds


class CaptureEvidenceTests(unittest.TestCase):
    def test_host_document_hashes_require_the_separate_actual_game_document(self):
        with tempfile.TemporaryDirectory() as folder:
            dist = Path(folder)
            (dist / "index.html").write_text("synthetic-shell")
            with self.assertRaises(CaptureFailure):
                deployed_documents(dist)
            game = dist / "play/local/play.html"
            game.parent.mkdir(parents=True)
            game.write_text("synthetic-host")
            script = game.parent / "direct-transport.js"
            script.write_text("synthetic-script")
            builds = deployed_documents(dist)
            self.assertEqual(len(builds), 3)
            self.assertEqual(next(row for row in builds if row["path"].endswith("direct-transport.js"))["sha256"], digest(script))

    def test_dirty_checkout_cannot_be_labelled_as_an_exact_commit(self):
        with mock.patch("capture_evidence.subprocess.run", return_value=SimpleNamespace(stdout="?? untracked-source\n")):
            with self.assertRaisesRegex(CaptureFailure, "not clean"):
                checkout_provenance(Path("synthetic-root"), Path("synthetic-artifacts"))

    def test_clean_checkout_must_match_recorded_commit_and_tree(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)
            (path / "source-sha.txt").write_text("a" * 40)
            (path / "source-tree.txt").write_text("b" * 40)
            responses = [SimpleNamespace(stdout=value) for value in ("", "a" * 40 + "\n", "b" * 40 + "\n")]
            with mock.patch("capture_evidence.subprocess.run", side_effect=responses):
                self.assertEqual(checkout_provenance(path, path)["source_checkout"], "clean")
            responses = [SimpleNamespace(stdout=value) for value in ("", "c" * 40 + "\n", "b" * 40 + "\n")]
            with mock.patch("capture_evidence.subprocess.run", side_effect=responses):
                with self.assertRaisesRegex(CaptureFailure, "differs"):
                    checkout_provenance(path, path)

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
