"""Meaningful stage-integrity negatives against the real Rust verifier."""
import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("spike_stage", HERE / "stage.py")
stage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(stage)


class StageAtlasIntegrity(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        configured = os.environ.get("TABULA60_AUTHORITY_TOOL")
        cls.tool = Path(configured) if configured else stage.REPO / "target/debug/examples/embedding_fixture"
        cls.tool.resolve(strict=True)

    def test_exact_snapshot_has_native_blake3_receipt(self):
        data, receipt = stage.verified_atlas_bytes(stage.REPO / "games/tiles/assets", self.tool)
        self.assertEqual(receipt["status"], "PASS")
        self.assertEqual(len(receipt["files"]), 2)
        self.assertEqual(len(data["tiles@1x.png"]), 4587)
        self.assertEqual(len(data["tiles@2x.png"]), 12943)

    def test_same_size_atlas_corruption_is_rejected_before_staging_identity(self):
        with tempfile.TemporaryDirectory(prefix="tabula60-stage-negative-") as directory:
            source = Path(directory)
            for name in ["tiles@1x.png", "tiles@2x.png"]:
                (source / name).write_bytes((stage.REPO / "games/tiles/assets" / name).read_bytes())
            original = (source / "tiles@1x.png").read_bytes()
            corrupt = bytearray(original)
            corrupt[100] ^= 1
            (source / "tiles@1x.png").write_bytes(corrupt)
            self.assertEqual(len(corrupt), len(original))
            with self.assertRaises(subprocess.CalledProcessError) as failure:
                stage.verified_atlas_bytes(source, self.tool)
            self.assertIn("BLAKE3", failure.exception.stderr)


if __name__ == "__main__":
    unittest.main()
