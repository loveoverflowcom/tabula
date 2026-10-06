#!/usr/bin/env python3
"""Pure acceptance-oracle sensitivity tests; no browser or network execution."""
import importlib.util
from copy import deepcopy
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

    def test_font_preference_must_be_established_before_app_scale_claim(self):
        module.require_font_preference(16, 16)
        module.require_font_preference(32, 32)
        with self.assertRaises(module.PrerequisiteBlocker):
            module.require_font_preference(16, 32)
        with tempfile.TemporaryDirectory() as temp:
            evidence = module.Evidence(Path(temp))
            evidence.case("real font setting", lambda: module.require_font_preference(16, 32))
            self.assertEqual(evidence.records[0]["status"], "BLOCKED")
            self.assertEqual(evidence.errors, ["real font setting"])

    def font_observations(self):
        return [{"scale": scale, "language": language, "viewport": {"width": width, "height": height},
                 "home": {"title": {"font_size": 34 * scale}, "body": {"font_size": 14 * scale}},
                 "library": {"title": {"font_size": 34 * scale}, "body": {"font_size": 14 * scale}}}
                for scale in (1, 2) for language in ("vi", "en")
                for width, height in ((320, 640), (390, 844))]

    def test_font_matrix_requires_both_widths_languages_and_real_doubling(self):
        module.assert_font_scaling(self.font_observations())
        for changed in (self.font_observations()[:-1], self.font_observations()[1:] + self.font_observations()[1:2]):
            with self.assertRaises(module.AcceptanceFailure):
                module.assert_font_scaling(changed)
        for width in (320, 390):
            for language in ("vi", "en"):
                for route in ("home", "library"):
                    for text in ("body", "title"):
                        with self.subTest(width=width, language=language, route=route, text=text):
                            observations = self.font_observations()
                            row = next(item for item in observations if item["scale"] == 2 and item["language"] == language
                                       and item["viewport"]["width"] == width)
                            row[route][text]["font_size"] /= 2
                            with self.assertRaises(module.AcceptanceFailure):
                                module.assert_font_scaling(observations)

    def narrow_metrics(self, language="en"):
        def label(text):
            return {"text": text, "words": [{"text": word, "line_count": 1, "outside_target": False}
                                             for word in text.split()]}
        expected = {"en": {"cta": ["Explore games"], "navigation": ["Home", "Library", "Account"]},
                    "vi": {"cta": ["Khám phá game"], "navigation": ["Trang chính", "Thư viện", "Tài khoản"]}}[language]
        return {"navigation": [label(text) for text in expected["navigation"]],
                "cta": [label(text) for text in expected["cta"]], "visible_hero_art": [],
                "hero": {"inner_width": 248, "copy": {"width": 248}, "copy_inner_width": 216,
                         "heading": {"width": 216}},
                "continue": {"inner_width": 184, "icon": {"bottom": 88}, "copy": {"y": 112, "width": 184}}}

    def test_narrow_oracle_reaches_320_390_at_200_but_leaves_normal_reference_layout(self):
        for width in (320, 390):
            for language in ("vi", "en"):
                self.assertTrue(module.assert_narrow_usability(self.narrow_metrics(language), width, 32, language, home=True))
                self.assertFalse(module.assert_narrow_usability({}, width, 16, language, home=True))
        self.assertTrue(module.assert_narrow_usability(self.narrow_metrics(), 512, 32, "en", home=True))
        self.assertFalse(module.assert_narrow_usability({}, 513, 32, "en", home=True))

    def test_narrow_label_oracle_rejects_split_words_clipping_and_missing_copy(self):
        for name in ("cta", "navigation"):
            for change in ("split", "outside", "empty", "missing", "changed"):
                with self.subTest(name=name, change=change):
                    metrics = self.narrow_metrics()
                    if change == "split":
                        metrics[name][-1]["words"][-1]["line_count"] = 2
                    elif change == "outside":
                        metrics[name][-1]["words"][-1]["outside_target"] = True
                    elif change == "empty":
                        metrics[name][-1]["words"] = []
                    elif change == "missing":
                        metrics[name] = []
                    else:
                        metrics[name][-1]["text"] = "Different label"
                    with self.assertRaises(module.AcceptanceFailure):
                        module.assert_narrow_usability(metrics, 320, 32, "en", home=True)

    def test_narrow_geometry_oracle_rejects_competing_art_and_narrow_continue_column(self):
        mutations = [{"visible_hero_art": [{"width": 100}]}, {"hero": None},
                     {"hero": {**self.narrow_metrics()["hero"], "copy": {"width": 180}}},
                     {"hero": {**self.narrow_metrics()["hero"], "heading": {"width": 150}}},
                     {"continue": None},
                     {"continue": {**self.narrow_metrics()["continue"], "copy": {"y": 0, "width": 184}}},
                     {"continue": {**self.narrow_metrics()["continue"], "copy": {"y": 112, "width": 110}}}]
        for change in mutations:
            with self.subTest(change=change), self.assertRaises(module.AcceptanceFailure):
                module.assert_narrow_usability({**self.narrow_metrics(), **deepcopy(change)}, 390, 32, "en", home=True)
        # Catalog contains Continue and navigation but has no Home hero or CTA.
        catalog = self.narrow_metrics()
        catalog.update(hero=None, cta=[], visible_hero_art=[])
        self.assertTrue(module.assert_narrow_usability(catalog, 320, 32, "en", home=False))

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

    def test_exact_existing_shell_budget_is_used_without_replacing_wasm_cap(self):
        owner = module.load_loading_budgets()
        self.assertEqual(owner.SHELL_RAW_LIMIT, 900_000)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "index.html").write_text('<link href="/shell.wasm" rel="preload">')
            (root / "shell.wasm").write_bytes(bytes([0, 97, 115, 109, 1, 0, 0, 0]))
            receipt = module.emitted_shell_budget(root)
            self.assertEqual(receipt["shell_raw_wasm_limit"], owner.SHELL_RAW_LIMIT)
            self.assertEqual(receipt["unique_resource_count_including_document"], 2)
            (root / "shell.wasm").write_bytes(b"x" * (owner.SHELL_RAW_LIMIT + 1))
            with self.assertRaisesRegex(module.AcceptanceFailure, "shell WASM raw budget exceeded"):
                module.emitted_shell_budget(root)

    def test_empty_oracle_cannot_be_satisfied_by_continue_status(self):
        class Locator:
            def __init__(self, count, text="No matching games"):
                self.number, self.text = count, text
                self.first = self

            def count(self):
                return self.number

            def is_visible(self):
                return self.number > 0

            def inner_text(self):
                return self.text

        class Page:
            def __init__(self, status=1, recovery=1, text="No matching games"):
                self.status, self.recovery, self.text = status, recovery, text

            def locator(self, selector):
                if selector == ".card":
                    return Locator(0)
                if selector == ".catalog__empty [role=status], .catalog__empty[role=status]":
                    return Locator(self.status, self.text)
                if selector == '.catalog__empty a[href="/games"]':
                    return Locator(self.recovery)
                # A global continue status always exists but is irrelevant.
                return Locator(1, "Saved games are unavailable")

        module.require_filtered_empty(Page())
        for page in (Page(status=0), Page(recovery=0), Page(text="   ")):
            with self.assertRaises(module.AcceptanceFailure):
                module.require_filtered_empty(page)


if __name__ == "__main__":
    unittest.main()
