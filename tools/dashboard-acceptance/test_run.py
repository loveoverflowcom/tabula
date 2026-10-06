#!/usr/bin/env python3
"""Pure acceptance-oracle sensitivity tests; no browser or network execution."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("dashboard_acceptance", Path(__file__).with_name("run.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class OracleTests(unittest.TestCase):
    def test_contrast_uses_independent_wcag_linear_luminance(self):
        self.assertAlmostEqual(module.contrast_ratio((0, 0, 0), (255, 255, 255)), 21)
        self.assertAlmostEqual(module.contrast_ratio((128, 128, 128), (128, 128, 128)), 1)
        self.assertGreater(module.contrast_ratio((86, 52, 190), (255, 255, 255)), 4.5)
        self.assertLess(module.contrast_ratio((190, 190, 190), (255, 255, 255)), 4.5)

    def test_reference_provenance_binds_visual_oracle_to_original_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            archive = Path(temp)
            root = archive / "design-01"
            root.mkdir()
            (root / "standalone.html").write_bytes(b"abc")
            import json
            (archive / "PROVENANCE.json").write_text(json.dumps({"design": "Tabula Design 01", "files": [{
                "path": "design-01/standalone.html", "bytes": 3,
                "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}]}))
            self.assertTrue(module.verify_reference(root)["all_archived_hashes_verified"])
            (root / "standalone.html").write_bytes(b"def")
            with self.assertRaises(module.AcceptanceFailure):
                module.verify_reference(root)

    def test_touch_target_requires_both_dimensions(self):
        self.assertTrue(module.usable_target(44, 44))
        self.assertFalse(module.usable_target(80, 42))
        self.assertFalse(module.usable_target(42, 80))

    def test_shell_wasm_is_allowed_but_game_runtime_atlas_model_is_not(self):
        allowed = {"/tabula-web-a.wasm"}
        self.assertFalse(module.is_heavy_resource("http://127.0.0.1/tabula-web-a.wasm", allowed))
        self.assertFalse(module.is_heavy_resource("http://127.0.0.1/catalog-cover.svg", allowed))
        for path in ("/play/local/index.html", "/game.wasm", "/roles-atlas.png", "/model.onnx", "/model.gguf"):
            with self.subTest(path=path):
                self.assertTrue(module.is_heavy_resource("http://127.0.0.1" + path, allowed))

    def test_manifest_hashes_actual_bytes_without_mutating_source(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "file").write_bytes(b"abc")
            self.assertEqual(module.file_manifest(root), [{"path": "file", "bytes": 3,
                "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}])

    def metrics(self):
        return {"viewport": {"width": 320, "dpr": 1}, "document_width": 320, "body_width": 320,
                "clipped_text": [], "cards": [{}], "bottom_nav": {"height": 76},
                "bottom_nav_position": "fixed", "main_bottom_padding": 80,
                "controls": [{"disabled": False, "rect": {"width": 44, "height": 44}}]}

    def test_layout_detects_overflow_missing_slot_and_small_target(self):
        module.assert_layout(self.metrics(), 320, small=True)
        for change in ({"document_width": 322}, {"main_bottom_padding": 0},
                       {"cards": []}, {"bottom_nav_position": "static"},
                       {"clipped_text": [{"tag": "H1"}]},
                       {"controls": [{"disabled": False, "rect": {"width": 43, "height": 44}}]}):
            with self.subTest(change=change):
                metrics = {**self.metrics(), **change}
                with self.assertRaises(module.AcceptanceFailure):
                    module.assert_layout(metrics, 320, small=True)

    def test_comparison_dimensions_are_css_viewports_not_mismatched_existing_pngs(self):
        sizes = {(width, height) for _, width, height in module.VIEWPORTS}
        self.assertTrue({(1440, 1000), (1100, 850), (390, 844), (320, 640), (768, 900), (844, 390)} <= sizes)

    def test_solid_contrast_selectors_keep_gradient_artwork_outside_false_claim(self):
        self.assertIn("backgroundImage!=='none'", module.CONTRAST)
        self.assertNotIn("card__art", module.CONTRAST)

    def test_existing_spa_route_handler_never_invents_account_api(self):
        handler = module.load_shell_handler()
        self.assertEqual(handler.__name__, "LocalShellHandler")
        source = Path(module.__file__).resolve().parents[1] / "serve-local-shell.py"
        self.assertIn("creates no identity, session or API response", source.read_text())


if __name__ == "__main__":
    unittest.main()
