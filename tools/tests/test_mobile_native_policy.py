import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest
import zipfile


spec = importlib.util.spec_from_file_location("mobile_native_policy", Path(__file__).parents[1] / "check-mobile-native-policy.py")
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


class MobileNativePolicyTest(unittest.TestCase):
    def test_current_production_selection_and_configuration_are_native_only(self):
        self.assertEqual([], policy.source_failures(policy.ROOT))

    def test_disguised_gameplay_host_and_retired_staging_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for platform, entry in (("android", "TabulaAndroid.kt"), ("ios", "TabulaIos.kt")):
                base = root / f"mobile/shared/src/{platform}Main/kotlin/{policy.MOBILE_PACKAGE}"
                base.mkdir(parents=True)
                (base / entry).write_text("fun install() { TabulaApp() }")
            for relative in ("mobile/android/build.gradle.kts", "mobile/shared/build.gradle.kts", "mobile/ios/TabulaApp.xcodeproj/project.pbxproj", "xtask/src/main.rs", ".github/workflows/ci.yml"):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("// historical WebView only\n")
            self.assertEqual([], policy.source_failures(root))
            host = root / f"mobile/shared/src/androidMain/kotlin/{policy.MOBILE_PACKAGE}/host/DisguisedSurface.kt"
            host.parent.mkdir()
            host.write_text("import android.webkit.WebView as Surface\nfun game() = Surface(context)")
            (root / "mobile/android/build.gradle.kts").write_text('val input = "../../target/tabula-mobile-game"')
            failures = policy.source_failures(root)
            self.assertTrue(any("web gameplay API" in failure for failure in failures))
            self.assertTrue(any("retired mobile web build input" in failure for failure in failures))

    def test_apk_payload_inspection_rejects_renamed_wasm_and_compiled_legacy_host(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "Tabula.apk"
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr("AndroidManifest.xml", b"synthetic manifest")
                archive.writestr("classes.dex", b"dex\n035\x00")
            self.assertEqual([], policy.apk_failures(path))
            with zipfile.ZipFile(path, "a") as archive:
                archive.writestr("assets/pack/image.bin", b"\x00asm\x01\x00\x00\x00")
            self.assertTrue(any("WASM" in failure for failure in policy.apk_failures(path)))
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr("AndroidManifest.xml", b"synthetic manifest")
                archive.writestr("classes.dex", b"dex\n035\x00" + policy.LEGACY_MARKERS[-1])
            self.assertTrue(any("runtime class" in failure for failure in policy.apk_failures(path)))

    def test_app_and_apk_cannot_pass_with_missing_artifacts_or_empty_selection(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertTrue(policy.apk_failures(root / "missing.apk"))
            self.assertTrue(policy.app_failures(root / "missing.app"))
            path = root / "Tabula.app"
            path.mkdir()
            self.assertTrue(policy.app_failures(path))
            (path / "Info.plist").write_bytes(plistlib.dumps({"CFBundleExecutable": "Tabula"}))
            (path / "Tabula").write_bytes(b"synthetic native executable")
            (path / "voice-dev-grant.json").write_text("{}")
            self.assertEqual([], policy.app_failures(path))
            (path / "renamed.data").write_bytes(b"\x00asm\x01\x00\x00\x00")
            self.assertTrue(any("WASM" in failure for failure in policy.app_failures(path)))

    def test_bridge_payload_and_web_document_are_rejected_under_new_directories(self):
        self.assertIsNotNone(policy.payload_failure("assets/alternate/entry.data", b"window.TabulaHostNative", True))
        self.assertIsNotNone(policy.payload_failure("assets/alternate/play.html", b"<canvas></canvas>", True))
        self.assertIsNone(policy.payload_failure("assets/brand/mark.png", b"\x89PNG", True))


if __name__ == "__main__":
    unittest.main()
