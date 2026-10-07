"""Bounded real Leptos native-history diagnostic on unchanged source."""
import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import tempfile
from datetime import datetime, timezone
from importlib.metadata import version
from pathlib import Path
from urllib.parse import urlsplit

from PIL import Image
from playwright.sync_api import sync_playwright


def utc():
    return datetime.now(timezone.utc).isoformat()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-root", type=Path, required=True)
    ap.add_argument("--source-commit", required=True)
    ap.add_argument("--source-tree", required=True)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()
    assert os.environ.get("GITHUB_ACTIONS") == "true"
    assert os.environ.get("TABULA_CLASSIC_GRAPHICS_QA") == "1"
    root, out = args.source_root.resolve(), args.output.resolve()
    git = lambda *a: subprocess.check_output(["git", *a], cwd=root, text=True).strip()
    assert git("rev-parse", "HEAD") == args.source_commit
    assert git("rev-parse", "HEAD^{tree}") == args.source_tree
    assert not git("status", "--porcelain", "--untracked-files=all")
    out.mkdir(parents=True, exist_ok=True)
    spec = importlib.util.spec_from_file_location("maintained", root / "tools/dashboard-acceptance/run.py")
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    dist = root / "apps/web/dist"
    data = {
        "source_commit": args.source_commit, "source_tree": args.source_tree, "pr": 114,
        "harness_commit": os.environ["GITHUB_SHA"], "run_id": os.environ["GITHUB_RUN_ID"],
        "started_at_utc": utc(), "playwright": version("playwright"),
        "tool_versions": {"rustc": subprocess.check_output(["rustc", "--version"], text=True).strip()},
        "fixture": "Actual static anonymous Leptos shell and native keyboard-typed disposable public code; authority unavailable. No account, API response, state, CSS or input value injected",
        "scope": "Only 320/16px actual Library-to-Chess-to-native-Back/Forward and keyboard input recovery, with bounded public pageerror/console.error detail. No board rerun, authenticated admission or clipboard permission",
        "historical_runs": [37602621508, 37606637708, 37609958188],
        "builds": {"shell": []}, "captures": [], "actions": [], "checks": [], "cases": [],
        "unrun": ["Authenticated create/join admission", "Clipboard/paste permission", "Physical mobile/native/CMP", "Audio, full motion and performance", "Board graphics rerun"],
    }
    for p in sorted(dist.rglob("*")):
        if p.is_file():
            b = p.read_bytes()
            data["builds"]["shell"].append({"path": p.relative_to(dist).as_posix(), "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()})

    def save():
        (out / "classic-provenance.json").write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")

    def check(name, ok, facts):
        data["checks"].append({"name": name, "status": "PASS" if ok else "FAIL", "facts": facts})
        save()

    def metrics(page):
        return page.evaluate("""() => {
          const rect=e=>{const b=e.getBoundingClientRect();return{x:b.x,y:b.y,width:b.width,height:b.height,right:b.right,bottom:b.bottom}};
          const facts=document.querySelector('.facts');
          const controls=[...document.querySelectorAll('[data-testid=online-create],[data-testid=online-join-code],[data-testid=online-join],[data-testid=online-status]')].map(e=>({test_id:e.dataset.testid,rect:rect(e),disabled:e.disabled??null}));
          return {viewport:innerWidth,document_scroll_width:document.documentElement.scrollWidth,root_font_px:parseFloat(getComputedStyle(document.documentElement).fontSize),facts:facts?{rect:rect(facts),grid_columns:getComputedStyle(facts).gridTemplateColumns,children:[...facts.children].map(e=>({tag:e.tagName,rect:rect(e)}))}:null,controls,scroll:{x:scrollX,y:scrollY}};
        }""")

    def capture(page, name, label, full=False, extra=None):
        target = out / (name + ".png")
        started = utc()
        b = page.screenshot(path=str(target), full_page=full)
        with Image.open(target) as im:
            im.verify()
        with Image.open(target) as im:
            size = list(im.size)
        data["captures"].append({"file": target.name, "label": label, "capture_started_at_utc": started,
            "captured_at_utc": utc(), "route_path": urlsplit(page.url).path,
            "viewport_css_pixels": page.viewport_size, "png_pixels": size, "dpr": page.evaluate("devicePixelRatio"),
            "scheme": page.locator("html").get_attribute("data-theme"), "locale": page.locator("html").get_attribute("lang"),
            "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest(), "original_png_unchanged": True,
            "actions_through_index": len(data["actions"]), "extra": extra, "entry_metrics_after_paint": metrics(page)})
        save()

    def attempt(name, operation):
        count = len(data["captures"])
        try:
            operation()
            data["cases"].append({"name": name, "status": "CAPTURED", "new_captures": len(data["captures"]) - count})
        except Exception as error:
            data["cases"].append({"name": name, "status": "BLOCKED", "error_type": type(error).__name__, "new_captures": len(data["captures"]) - count})
        save()

    try:
        check("unchanged 900000-byte shell loading budget", True, helper.emitted_shell_budget(dist))
    except Exception as error:
        check("unchanged 900000-byte shell loading budget", False, {"error_type": type(error).__name__, "budget_not_waived": True})

    with helper.static_origin(dist, shell=True) as origin, sync_playwright() as p:
        for width, font in ((320, 16),):
            def partition():
                with tempfile.TemporaryDirectory(prefix="classic114-font-") as temp:
                    profile = Path(temp)
                    (profile / "Default").mkdir()
                    (profile / "Default/Preferences").write_text(json.dumps({"webkit": {"webprefs": {"default_font_size": font, "default_fixed_font_size": 26 if font == 32 else 13, "minimum_font_size": 0}}, "profile": {"default_zoom_level": 0}}))
                    ctx = p.chromium.launch_persistent_context(str(profile), channel="chromium", headless=True, viewport={"width": width, "height": 640 if width == 320 else 844}, device_scale_factor=1, reduced_motion="reduce")
                    try:
                        page = ctx.pages[0]
                        browser_probe = ctx.new_cdp_session(page)
                        data["browser_version_probe"] = browser_probe.send("Browser.getVersion")
                        data["browser_version"] = data["browser_version_probe"]["product"].split("/", 1)[-1]
                        browser_probe.detach()
                        errors, console_errors = [], []
                        def page_error(error):
                            if len(errors) >= 10:
                                return
                            stack = getattr(error, "stack", None)
                            errors.append({"at_utc": utc(), "name": getattr(error, "name", None), "message": str(getattr(error, "message", str(error)))[:2000], "stack": str(stack)[:8000] if stack else None})
                        def console_error(message):
                            if message.type == "error" and len(console_errors) < 12:
                                console_errors.append({"at_utc": utc(), "text": message.text[:4000]})
                        page.on("pageerror", page_error)
                        page.on("console", console_error)
                        initial = page.evaluate("parseFloat(getComputedStyle(document.documentElement).fontSize)")
                        helper.require_font_preference(initial, font)
                        helper.settle(page, origin + "/games")
                        helper.locale(page, "en")
                        page.locator('a[href="/games/com.tabula.chess"]').first.click()
                        page.locator('[data-testid=online-create]').wait_for(state="visible")
                        page.wait_for_timeout(250)
                        before_code = page.locator('[data-testid=online-join-code]')
                        before_code.click()
                        before_code.press_sequentially("ab12cd34ef56zz", delay=25)
                        before_value = before_code.input_value()
                        check("pre-Back native synthetic code uppercase/max12", before_value == "AB12CD34EF56", {"actual_disposable_value": before_value, "expected_disposable_value": "AB12CD34EF56"})
                        m = metrics(page)
                        check(f"{width} EN font{font} native preference", m["root_font_px"] == font, {"initial_font_px": initial, "root_font_px": m["root_font_px"]})
                        facts = m["facts"]
                        check(f"{width} EN font{font} document/facts reflow", m["document_scroll_width"] <= width + 1 and facts is not None and all(c["rect"]["right"] <= width + 1 and c["rect"]["x"] >= -1 for c in facts["children"]), m)
                        check(f"{width} EN font{font} entry controls fit", len(m["controls"]) == 4 and all(c["rect"]["right"] <= width + 1 and c["rect"]["x"] >= -1 for c in m["controls"]), {"controls": m["controls"]})
                        capture(page, f"entry-{width}-en-font{font}-full", "Actual source-pinned detail with native typed disposable code before history recheck", True, {"initial_font_px": initial, "synthetic_only": True})
                        page.locator('.facts').scroll_into_view_if_needed()
                        capture(page, f"entry-{width}-en-font{font}-facts-viewport", "Original viewport at actual native-scrolled facts grid", False, {"initial_font_px": initial})
                        if font == 16:
                            def history():
                                page.evaluate("scrollTo(0,0)")
                                try:
                                    data["actions"].append({"at_utc": utc(), "action": "Before native Back after actual Library card click, detail full-page and native facts viewport capture, then scroll-to-top", "path": urlsplit(page.url).path})
                                    save()
                                    page.go_back(wait_until="networkidle", timeout=10000)
                                    page.locator('.catalog #search').wait_for(state="visible", timeout=10000)
                                    data["actions"].append({"at_utc": utc(), "action": "Native Back after actual Library content visible", "path": urlsplit(page.url).path})
                                    capture(page, "entry-320-native-back-library", "Native Back with real Library marker awaited", True)
                                    page.go_forward(wait_until="networkidle", timeout=10000)
                                    page.locator('[data-testid=online-join-code]').wait_for(state="visible", timeout=10000)
                                    data["actions"].append({"at_utc": utc(), "action": "Native Forward after actual detail entry visible", "path": urlsplit(page.url).path})
                                    restored_code = page.locator('[data-testid=online-join-code]').input_value()
                                    check("same-document public-code memory restores after Forward", restored_code == before_value == "AB12CD34EF56", {"expected_disposable_value": before_value, "actual_disposable_value": restored_code, "scope": "same document only, no reload/provider redirect persistence claim"})
                                    capture(page, "entry-320-native-forward-detail", "Native Forward with actual entry marker awaited", True)
                                    match_posts = []
                                    page.on("request", lambda request: match_posts.append(urlsplit(request.url).path) if request.method == "POST" and urlsplit(request.url).path.startswith("/api/matches") else None)
                                    code = page.locator('[data-testid=online-join-code]')
                                    code.click()
                                    code_metrics = code.evaluate("e=>({length:e.value.length,uppercase:e.value===e.value.toUpperCase(),max_length:e.maxLength,labels:e.labels?.length??0,focused:e===document.activeElement})")
                                    check("native public-code input works after Forward", code_metrics["length"] == 12 and code_metrics["uppercase"] and code_metrics["max_length"] == 12 and code_metrics["labels"] == 1 and code_metrics["focused"], code_metrics)
                                    capture(page, "entry-320-native-forward-code-focused", "Actual keyboard-typed disposable code after Forward; not a real invitation and never admitted", False, {"synthetic_only": True})
                                    code.press("Enter")
                                    page.wait_for_timeout(250)
                                    check("recovered anonymous Enter creates no match POST", not match_posts, {"match_post_count": len(match_posts)})
                                    check("native Back/Forward restored content without browser exception", not errors and urlsplit(page.url).path == "/games/com.tabula.chess", {"browser_errors": errors, "public_console_errors": console_errors})
                                except Exception:
                                    data["native_history_blocked_observation"] = {"path": urlsplit(page.url).path, "library_marker_count": page.locator('.catalog #search').count(), "entry_control_count": page.locator('[data-testid=online-join-code]').count(), "browser_errors": errors, "public_console_errors": console_errors}
                                    capture(page, "entry-320-native-history-blocked", "Original UI after bounded semantic history wait failed; cause unproven", True)
                                    raise
                            attempt("semantic native history recheck", history)
                        data.setdefault("browser_error_observations", []).append({"font_px": font, "errors": errors, "public_console_errors": console_errors})
                    finally:
                        ctx.close()
            attempt(f"actual {width} EN font{font} facts partition", partition)
    data["status"] = "PASS" if all(x["status"] == "PASS" for x in data["checks"]) and all(x["status"] == "CAPTURED" for x in data["cases"]) else "PARTIAL"
    data["finished_at_utc"] = utc()
    save()
    assert len(data["captures"]) >= 3, "diagnostic must preserve actual pre-Back and outcome originals"


if __name__ == "__main__":
    main()
