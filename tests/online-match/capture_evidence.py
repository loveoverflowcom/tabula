"""Secret-free evidence from actual rendered browser screenshots, never mockups."""
from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
from importlib.metadata import version
from urllib.parse import urlsplit

from PIL import Image

MASK_SELECTOR = (
    "input,textarea,[data-testid='online-code'],[data-secret],[data-private],"
    "[name*='csrf' i],[name*='token' i],[data-testid*='credential' i]"
)
PNG_NAME = re.compile(r"^[0-9]{2}-[a-z0-9-]+\.png$")
SHA = re.compile(r"^[0-9a-f]{40}$")
SAFE_ROUTES = frozenset({"/", "/games", "/games/com.tabula.chess", "/play/local/"})


class CaptureFailure(Exception):
    """A closed failure label; no raw DOM, request data or secret values."""


def digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1_048_576), b""):
            result.update(block)
    return result.hexdigest()


def source_sha(path: Path) -> str:
    value = path.read_text().strip()
    if not SHA.fullmatch(value):
        raise CaptureFailure("exact checkout provenance is missing or invalid")
    return value


def checkout_provenance(root: Path, artifacts: Path) -> dict:
    # A checkout label must not silently describe locally edited/untracked source.
    # Capture all subprocess diagnostics privately; never print file names or diffs.
    status = subprocess.run(["git", "status", "--porcelain", "--untracked-files=all"],
                            cwd=root, check=True, capture_output=True, text=True)
    if status.stdout:
        raise CaptureFailure("actual build checkout is not clean")
    commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, check=True,
                            capture_output=True, text=True).stdout.strip()
    tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], cwd=root, check=True,
                          capture_output=True, text=True).stdout.strip()
    if commit != source_sha(artifacts / "source-sha.txt") or tree != source_sha(artifacts / "source-tree.txt"):
        raise CaptureFailure("actual checkout differs from the recorded build source")
    return {"source_commit": commit, "source_tree": tree, "source_checkout": "clean"}


def public_route(url: str) -> str:
    parsed = urlsplit(url)
    if parsed.scheme != "https" or parsed.hostname != "localhost" or parsed.port != 9443 or parsed.path not in SAFE_ROUTES:
        raise CaptureFailure("screenshot is outside the approved served UI routes")
    # Query/fragment and public routing IDs are unnecessary for image review.
    return parsed.path


def wasm_builds(dist: Path) -> list[dict]:
    files = sorted(dist.rglob("*.wasm"))
    if not files or not any(p.relative_to(dist).as_posix().startswith("play/local/") for p in files):
        raise CaptureFailure("the actual separate game WASM build is absent")
    builds = []
    for path in files:
        if not path.is_file() or path.stat().st_size == 0:
            raise CaptureFailure("a deployed WASM artifact is empty")
        builds.append({"path": path.relative_to(dist).as_posix(),
                       "size_bytes": path.stat().st_size, "sha256": digest(path)})
    return builds


def deployed_documents(dist: Path) -> list[dict]:
    files = sorted(path for path in dist.rglob("*") if path.is_file()
                   and path.suffix in (".html", ".js", ".css"))
    if not any(path.relative_to(dist).as_posix() == "play/local/play.html" for path in files):
        raise CaptureFailure("the actual separately staged game document is absent")
    return [{"path": path.relative_to(dist).as_posix(), "size_bytes": path.stat().st_size,
             "sha256": digest(path)} for path in files]


def tool_version(command: str) -> str:
    output = subprocess.run([command, "--version"], check=True, capture_output=True, text=True).stdout.strip()
    if not output or len(output) > 256 or "\n" in output:
        raise CaptureFailure("build tool provenance is unavailable")
    return output


class CaptureEvidence:
    def __init__(self, artifacts: Path, root: Path):
        self.artifacts = artifacts
        self.captures: list[dict] = []
        self.optional: list[dict] = []
        binary = root / "tests/online-match/target/debug/online-match-fixture"
        self.build = {
            **checkout_provenance(root, artifacts),
            "deployed_wasm": wasm_builds(root / "apps/web/dist"),
            "deployed_html_js_css": deployed_documents(root / "apps/web/dist"),
            "native_fixture_sha256": digest(binary),
            "rustc": tool_version("rustc"), "cargo": tool_version("cargo"),
            "trunk": tool_version("trunk"), "playwright": version("playwright"),
            "shell_build": "TABULA_PLAY_BASE=/play trunk build --release --cargo-profile wasm-release --features online",
            "game_build": "web,online; wasm32-unknown-unknown; wasm-release",
            "fixture_build": "body-publication-test; native debug; debug info disabled",
        }
        self.write()

    def write(self) -> None:
        document = {
            "version": 1, "capture_kind": "actual_browser_rendered_png",
            "authentication": "Disposable synthetic identities through real durable PostgreSQL session authority",
            "provider_login": "Separate genuine Kanidm CI job; these images do not picture provider login",
            "build": self.build, "captures": self.captures,
            "optional_states": self.optional,
        }
        run_id, attempt = os.environ.get("GITHUB_RUN_ID", ""), os.environ.get("GITHUB_RUN_ATTEMPT", "")
        if run_id.isdigit():
            document["workflow_run_id"] = run_id
        if attempt.isdigit():
            document["workflow_run_attempt"] = attempt
        (self.artifacts / "screenshots-provenance.json").write_text(json.dumps(document, indent=2) + "\n")

    def capture(self, page, filename: str, label: str, role: str,
                rendered_condition: str, *, canvas: bool = False,
                secrets: list[str] | None = None) -> bytes:
        if not PNG_NAME.fullmatch(filename):
            raise CaptureFailure("screenshot filename is invalid")
        route = public_route(page.url)
        target = page.locator("#glcanvas") if canvas else page
        if canvas and not target.is_visible():
            raise CaptureFailure("the actual game canvas is not visible")
        masks = [page.locator(MASK_SELECTOR)]
        # Dynamic text masks are in-memory only. Neither values nor mask counts
        # derived from them enter metadata, filenames, DOM edits or logs.
        masks.extend(page.get_by_text(secret, exact=False) for secret in (secrets or []) if secret)
        path = self.artifacts / filename
        options = {"path": str(path), "mask": masks, "animations": "disabled", "timeout": 30_000}
        if not canvas:
            options["full_page"] = True
        png = target.screenshot(**options)
        image = Image.open(io.BytesIO(png)).convert("RGB")
        if image.width < 600 or image.height < 400 or len(image.resize((160, 120)).getcolors(19_201) or []) <= 32:
            raise CaptureFailure("captured UI pixels are blank or unexpectedly small")
        self.captures.append({
            "file": filename, "label": label, "role": role,
            "rendered_condition": rendered_condition, "route_path": route,
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "browser": "actual Chromium, headless, independent temporary profile",
            "browser_version": page.context.browser.version,
            "viewport_css_pixels": page.viewport_size,
            "device_pixel_ratio": page.evaluate("window.devicePixelRatio"),
            "png_pixels": {"width": image.width, "height": image.height},
            "size_bytes": len(png), "sha256": hashlib.sha256(png).hexdigest(),
            "redactions": "Input/textarea values, active join-code display, secret-marked nodes and in-memory secret text matches masked by browser screenshot capture",
            "pixel_inspection": "captured; parent inspection required before issue publication",
        })
        self.write()
        return png

    def unavailable(self, label: str, reason: str) -> None:
        self.optional.append({"label": label, "status": "not_captured", "reason": reason})
        self.write()
