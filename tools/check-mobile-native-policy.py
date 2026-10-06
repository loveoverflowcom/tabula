#!/usr/bin/env python3
"""ADR-0043 source/config and packaged-artifact guard; it does not prove native gameplay.

Only gameplay owners are checked for WebView APIs. System-browser authentication and
native voice remain outside that source boundary. APK/app inspection also rejects the
retired game document, executable WASM, and its bridge/loader payload under renamed files.
"""

import argparse
from pathlib import Path
import plistlib
import re
import sys
import zipfile


ROOT = Path(__file__).resolve().parents[1]
MOBILE_PACKAGE = "com/loveoverflow/tabula/mobile"
FORBIDDEN_APIS = (
    "android.webkit", "androidx.webkit", "platform.WebKit", "WKWebView(",
    "WebView(", "evaluateJavascript(", "evaluateJavaScript(", "JavascriptInterface",
)
LEGACY_MARKERS = (
    b"TabulaHostNative", b"window.webkit.messageHandlers.tabulaHost",
    b"miniquad_add_plugin", b"WebAssembly.instantiate",
    b"Lcom/loveoverflow/tabula/mobile/host/AndroidGameRuntime;",
    b"Lcom/loveoverflow/tabula/mobile/host/WebViewGameHost;",
)


def without_comments(source):
    # Retain quoted values: build-script paths and strings carry policy decisions too.
    return re.sub(
        r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//[^\n]*|/\*.*?\*/',
        lambda match: "" if match.group().startswith(("//", "/*")) else match.group(),
        source, flags=re.S,
    )


def source_failures(root):
    failures = []
    gameplay_sources = set()
    for platform, entry in (("android", "TabulaAndroid.kt"), ("ios", "TabulaIos.kt")):
        base = root / f"apps/mobile/shared/src/{platform}Main/kotlin/{MOBILE_PACKAGE}"
        sources = [base / entry, *sorted((base / "host").rglob("*.kt"))]
        gameplay_sources.update(sources)
        for path in sources:
            if not path.is_file():
                failures.append(f"missing production entrypoint: {path.relative_to(root)}")
                continue
            source = without_comments(path.read_text())
            if path.name == entry and ("GameBundle.parse" in source or "BundlePaths" in source):
                failures.append(f"production launch still reads the web catalog: {path.relative_to(root)}")

    # The application roots must not bypass the shared GameHost selector with another web surface.
    for base in (root / "apps/mobile/android/src/main", root / "apps/mobile/ios/TabulaApp"):
        gameplay_sources.update(path for path in base.rglob("*") if path.suffix in (".kt", ".java", ".swift"))
    common = root / f"apps/mobile/shared/src/commonMain/kotlin/{MOBILE_PACKAGE}"
    gameplay_sources.update((common / "host").rglob("*.kt"))
    if (common / "TabulaApp.kt").is_file():
        gameplay_sources.add(common / "TabulaApp.kt")
    for path in sorted(gameplay_sources):
        if not path.is_file():
            continue
        source = without_comments(path.read_text())
        for api in FORBIDDEN_APIS:
            if api in source:
                failures.append(f"web gameplay API {api}: {path.relative_to(root)}")
        if re.search(r"\b(?:WKWebView|WebView)\b", source):
            failures.append(f"web surface type in gameplay owner: {path.relative_to(root)}")

    config_rules = {
        "apps/mobile/android/build.gradle.kts": ("tabula-mobile-game", "prepareGameBundle", "requireGameBundle"),
        "apps/mobile/shared/build.gradle.kts": ("libs.androidx.webkit",),
        "apps/mobile/ios/TabulaApp.xcodeproj/project.pbxproj": ("tabula-mobile-game", "stage-mobile-game", "Package game bundle"),
        "xtask/src/main.rs": ("mobile_stage_cmd::run", "mod mobile_stage_cmd;"),
        ".github/workflows/ci.yml": ("cargo xtask stage-mobile-game", "tabula.requireGameBundle"),
    }
    for relative, forbidden in config_rules.items():
        path = root / relative
        if not path.is_file():
            failures.append(f"missing build configuration: {relative}")
            continue
        source = without_comments(path.read_text())
        if path.suffix == ".yml":
            source = re.sub(r"(?m)^\s*#.*$", "", source)
        for value in forbidden:
            if value in source:
                failures.append(f"retired mobile web build input {value}: {relative}")
    return failures


def payload_failure(name, data, resource):
    parts = Path(name).parts
    if "tabula-game" in parts or Path(name).name == "tabula-games.json":
        return "retired mobile gameplay document/catalog"
    if data.startswith(b"\x00asm") or Path(name).suffix.lower() == ".wasm":
        return "executable WASM gameplay payload"
    if resource and Path(name).suffix.lower() in (".html", ".htm", ".js", ".mjs"):
        return "web document/script in the native-only resource package"
    if any(marker in data for marker in LEGACY_MARKERS):
        return "retired gameplay bridge, loader, or runtime class"
    return None


def apk_failures(path):
    if not path.is_file():
        return [f"APK does not exist: {path}"]
    failures = []
    try:
        with zipfile.ZipFile(path) as archive:
            names = set(archive.namelist())
            if "AndroidManifest.xml" not in names or "classes.dex" not in names:
                failures.append(f"not an assembled CMP APK: {path}")
            for item in archive.infolist():
                if item.is_dir():
                    continue
                data = archive.read(item)
                problem = payload_failure(item.filename, data, item.filename.startswith("assets/"))
                if problem:
                    failures.append(f"{path.name}:{item.filename}: {problem}")
    except (OSError, zipfile.BadZipFile) as error:
        failures.append(f"cannot inspect APK {path}: {error}")
    return failures


def app_failures(path):
    if not path.is_dir():
        return [f"app bundle does not exist: {path}"]
    failures = []
    try:
        info = plistlib.loads((path / "Info.plist").read_bytes())
        executable = info.get("CFBundleExecutable")
        if not isinstance(executable, str) or not (path / executable).is_file():
            failures.append(f"app bundle has no declared executable: {path}")
    except (OSError, plistlib.InvalidFileException, ValueError) as error:
        failures.append(f"cannot inspect app metadata {path}: {error}")
    for item in sorted(path.rglob("*")):
        if not item.is_file():
            continue
        relative = item.relative_to(path).as_posix()
        problem = payload_failure(relative, item.read_bytes(), "Frameworks/" not in relative)
        if problem:
            failures.append(f"{path.name}:{relative}: {problem}")
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--apk", type=Path, action="append", default=[])
    parser.add_argument("--app", type=Path, action="append", default=[])
    args = parser.parse_args()
    failures = source_failures(args.root)
    for apk in args.apk:
        failures.extend(apk_failures(apk))
    for app in args.app:
        failures.extend(app_failures(app))
    if failures:
        print("mobile native-only policy: FAIL", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1
    inspected = len(args.apk) + len(args.app)
    print(f"mobile native-only policy: PASS (source/config; {inspected} packaged artifacts inspected; native gameplay remains unimplemented)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
