"""Bounded actual PR112 Chess screenshots; no state/UI/response injection."""
from datetime import datetime, timezone
from importlib.metadata import version
from pathlib import Path
from urllib.parse import urlsplit
import argparse
import hashlib
import importlib.util
import json
import math
import os
import re
import subprocess
import time

from PIL import Image
from playwright.sync_api import sync_playwright
from graphics_probe import public_samples, probe

EXPECTED = "94dc7df5012640e7d6909d1ab5c02eb273d1dc37"
TREE = "7a2f8d0ddd9474d038ab0fd59c6de23fa219da13"


def utc():
    return datetime.now(timezone.utc).isoformat()


def git(root, *args):
    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def layout(width, height):
    """Source-owned BoardLayout geometry for pointer mapping, not a rules oracle."""
    margin = min(width * .02, height * .025, 16)
    gap = min(height * .01, 8)
    title = min(height * .05, 32)
    player = 44 if height >= 450 else 24 if height >= 160 else min(height * .075, 24)
    rail = (width >= 760 and height >= 420) or (width >= 600 and height < 420)
    rail_width = min(width * .3, 240) if rail else 0
    game_width = max(width - margin * 2 - rail_width - (gap * 2 if rail else 0), 0)
    coordinate = min(game_width * .025, 12)
    status_height = 0 if rail else min(height * .08, 64 if height >= 700 else 48)
    rail_toolbar = rail and height < 450
    controls_height = 44 if height >= 200 and width >= 280 and not rail_toolbar else 0
    fixed = margin * 2 + title + player * 2 + status_height + gap * 6 + coordinate * 2
    side = min(max(game_width - coordinate * 2, 0), max(height - fixed - controls_height, 0), 640)
    compact = not rail_toolbar and (width < 760 or side < 464)
    if controls_height and not compact:
        columns = max(math.floor((side + 4) / 76), 1)
        controls_height = math.ceil(6 / columns) * 48 - 4
        side = min(max(game_width - coordinate * 2, 0), max(height - fixed - controls_height, 0), 640)
    content_width = side + coordinate * 2 + (rail_width + gap * 2 if rail else 0)
    left = (width - content_width) * .5 + coordinate
    top_player = margin + title + gap
    board_y = top_player + player + gap + coordinate
    bottom_player = board_y + side + coordinate + gap
    status_y = bottom_player + player + gap
    controls_y = status_y if rail else status_y + status_height + gap
    return {"width": width, "height": height, "left": left, "top": board_y,
            "side": side, "square": side / 8, "controls_y": controls_y,
            "controls_height": controls_height, "compact": compact,
            "mapping_source": "games/chess/src/presentation/mod.rs:183 BoardLayout::oriented"}


def square(g, name, flipped=False):
    assert re.fullmatch("[a-h][1-8]", name)
    file, rank = ord(name[0]) - ord("a"), int(name[1]) - 1
    column, row = (7 - file, rank) if flipped else (file, 7 - rank)
    return g["left"] + (column + .5) * g["square"], g["top"] + (row + .5) * g["square"]


def control(g, index):
    count = 2 if g["compact"] else 3
    columns = min(max(math.floor((g["side"] + 4) / 76), 1), count)
    width = (g["side"] - 4 * (columns - 1)) / columns
    return g["left"] + (index % columns) * (width + 4) + width / 2, g["controls_y"] + (index // columns) * 48 + 22


def promotion(g, target, index=None):
    # Source PromotionLayout with unchanged generated spatial/type metrics.
    padding, gap, section_gap, heading, minimum = 12, 2, 8, 24, 44
    size = min(max(g["square"] * .9, minimum + 16), (g["width"] - padding * 4 - gap * 3) / 4,
               g["height"] - padding * 4 - heading - section_gap * 2 - minimum)
    assert size >= minimum
    group = size * 4 + gap * 3
    panel_width, panel_height = group + padding * 2, padding * 2 + heading + section_gap * 2 + size + minimum
    cx, cy = square(g, target)
    target_y = cy - g["square"] / 2
    y = target_y + g["square"] + section_gap if cy < g["top"] + g["side"] / 2 else target_y - panel_height - section_gap
    x = min(max(cx - panel_width / 2, padding), g["width"] - panel_width - padding)
    y = min(max(y, padding), g["height"] - panel_height - padding)
    first_y = y + padding + heading + section_gap
    if index is None:
        return x + padding + group / 2, first_y + size + section_gap + minimum / 2
    return x + padding + index * (size + gap) + size / 2, first_y + size / 2


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-root", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()
    assert os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("TABULA_CHESS_GRAPHICS_QA") == "1", "authorized isolated capture only"
    root, out = args.source_root.resolve(), args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    assert git(root, "rev-parse", "HEAD") == EXPECTED and git(root, "rev-parse", "HEAD^{tree}") == TREE
    assert not git(root, "status", "--porcelain", "--untracked-files=all"), "actual source checkout is not clean"
    spec = importlib.util.spec_from_file_location("maintained_static", root / "tools/dashboard-acceptance/run.py")
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    dist = root / "target/tabula-web-game"
    data = {"source_commit": EXPECTED, "source_tree": TREE, "pr": 112,
            "base_commit": "80d9fdb96f18cd59fa85c9401b533d66bf04b5d7",
            "harness_commit": os.environ["GITHUB_SHA"], "run_id": os.environ["GITHUB_RUN_ID"],
            "started_at_utc": utc(), "playwright": version("playwright"),
            "fixture": "Disposable untimed same-device local Chess. Public legal input sequences only; no accounts, credentials, online service, FEN or canonical state injected/read",
            "scope": "Narrow PR112 corrected HUD recheck against preserved c61 originals; actual source-owned compiled WASM, desktop Chromium viewports only; native/mobile/CMP/performance NOT_RUN",
            "before_source": "c61dbc62271cd4a5554dd62bc139a06bab58fb44", "before_run_id": 37589145897, "before_artifact_id": 11468256290,
            "builds": [], "captures": [], "actions": [], "checks": [], "cases": []}
    for p in sorted(dist.rglob("*")):
        if p.is_file():
            b = p.read_bytes()
            data["builds"].append({"path": p.relative_to(dist).as_posix(), "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()})

    def save():
        (out / "chess-provenance.json").write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")

    def check(name, ok, facts):
        data["checks"].append({"name": name, "status": "PASS" if ok else "FAIL", "facts": facts})
        save()
        return ok

    def capture(page, name, label):
        start = time.perf_counter()
        capture_started = utc()
        p = out / (name + ".png")
        b = page.screenshot(path=str(p))
        capture_finished = utc()
        screenshot_elapsed = round((time.perf_counter() - start) * 1000)
        with Image.open(p) as im:
            im.verify()
        with Image.open(p) as im:
            dimensions = list(im.size)
            colors = len(im.convert("RGB").getcolors(im.width * im.height) or [])
        ocr = subprocess.run(["tesseract", str(p), "stdout", "--psm", "11"], text=True, capture_output=True, timeout=20)
        text = ocr.stdout.strip() if ocr.returncode == 0 else ""
        record = {"file": p.name, "label": label, "capture_started_at_utc": capture_started,
                  "captured_at_utc": capture_finished, "route_path": urlsplit(page.url).path,
                  "viewport_css_pixels": page.viewport_size, "png_pixels": dimensions, "dpr": page.evaluate("devicePixelRatio"),
                  "scheme": page.locator("html").get_attribute("data-theme"), "locale": page.locator("html").get_attribute("lang"),
                  "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest(), "original_png_unchanged": True,
                  "screenshot_elapsed_ms": screenshot_elapsed,
                  "ocr_analysis": {"tool": "Tesseract 5 public screenshot analysis, no transformed PNG saved", "text": text[:8000]},
                  "actions_through_index": len(data["actions"])}
        if page.locator("#glcanvas").is_visible() and page.locator("#loader").is_hidden():
            record["canvas_bounds_after_paint"] = page.locator("#glcanvas").bounding_box()
            record["canvas_intrinsic_after_paint"] = page.locator("#glcanvas").evaluate("el=>({width:el.width,height:el.height})")
            record["pointer_mapping_for_nonterminal_actions_only"] = layout(record["canvas_bounds_after_paint"]["width"], record["canvas_bounds_after_paint"]["height"])
            record["terminal_mapping_note"] = "Completion reserves a separate Rust dock and smaller BoardLayout; no terminal pointer action attempted by this driver"
            check(name + " actual nonblank rendering", colors > 100, {"unique_rgb_colors": colors})
        if name in ("01-desktop-light-initial", "04-desktop-capture-session-hud", "19-checkmate-result"):
            bounds = record.get("canvas_bounds_after_paint")
            if bounds:
                mapping = layout(bounds["width"], bounds["height"] - (164 if name == "19-checkmate-result" else 0))
                # Desktop rail X derives from source-owned geometry. Sampling is
                # limited to public helper/first-history text and its background.
                board_height = bounds["height"] - (164 if name == "19-checkmate-result" else 0)
                source_gap = min(board_height * .01, 8)
                status_x = mapping["left"] + mapping["side"] + 12 + source_gap * 2
                x = int(status_x + 8)
                region = (x, 98, x + 68, 117)
                samples = public_samples(p, region, (53,33,70), (254,251,244))
                record["read_only_graphics_probe"] = probe(page, samples)
        data["captures"].append(record)
        save()
        return record

    def click(page, point, label, delay=75):
        g = page.locator("#glcanvas").bounding_box()
        assert g and 0 <= point[0] <= g["width"] and 0 <= point[1] <= g["height"]
        page.locator("#glcanvas").click(position={"x": point[0], "y": point[1]}, delay=delay)
        data["actions"].append({"at_utc": utc(), "action": label, "canvas_local_css_point": list(point)})
        save()
        page.wait_for_timeout(130)

    def geom(page):
        b = page.locator("#glcanvas").bounding_box()
        assert b and b["width"] > 0 and b["height"] > 0
        return layout(b["width"], b["height"])

    def move(page, name, flipped=False):
        g = geom(page)
        click(page, square(g, name[:2], flipped), "select " + name[:2])
        click(page, square(g, name[2:4], flipped), "destination " + name[2:4])
        page.wait_for_timeout(180)

    def sequence(page, moves):
        for name in moves:
            move(page, name)

    def start(browser, origin, width=1200, height=880, theme="light", reduced=True, label=None):
        context = browser.new_context(viewport={"width": width, "height": height}, device_scale_factor=1)
        page = context.new_page()
        page.emulate_media(color_scheme="dark" if "dark" in theme else "light", contrast="more" if theme.startswith("hc-") else "no-preference", reduced_motion="reduce" if reduced else "no-preference")
        page.goto(origin + "/index.html", wait_until="networkidle")
        page.locator("#locale").select_option("en")
        page.locator("#clock").select_option("untimed")
        page.locator("details.preferences summary").click()
        page.locator("#theme").select_option(theme)
        page.locator("#motion").select_option("reduced" if reduced else "system")
        if label:
            capture(page, label, "Actual standalone setup before explicit local launch")
        page.locator("#setup button[type=submit]").click()
        page.locator("#loader").wait_for(state="hidden", timeout=120000)
        page.locator("#glcanvas").wait_for(state="visible")
        page.wait_for_timeout(350)
        check("actual canvas startup " + theme + " " + str(width), page.locator("#runtime-error").is_hidden(), {"theme": page.locator("html").get_attribute("data-theme"), "width": width})
        data["actions"].append({"at_utc": utc(), "action": "New explicit local setup launch", "viewport": page.viewport_size, "theme": theme, "motion": "reduced" if reduced else "system with no-preference media", "clock": "untimed"})
        save()
        return context, page

    def attempt(name, run):
        begin = len(data["captures"])
        try:
            run()
            data["cases"].append({"name": name, "status": "CAPTURED", "new_captures": len(data["captures"]) - begin})
        except Exception as e:
            data["cases"].append({"name": name, "status": "BLOCKED", "error_type": type(e).__name__, "new_captures": len(data["captures"]) - begin})
        save()

    with helper.static_origin(dist, shell=False) as origin, sync_playwright() as p:
        browser = p.chromium.launch(headless=True, args=["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"])
        data["browser_version"] = browser.version
        data["tool_versions"] = {"rustc": subprocess.check_output(["rustc", "--version"],text=True).strip(), "tesseract":subprocess.check_output(["tesseract","--version"],text=True).splitlines()[0]}
        save()
        def desktop():
            context, page = start(browser, origin)
            try:
                capture(page, "01-desktop-light-initial", "Actual corrected initial board/seat guidance; paired with before c61")
                sequence(page, ["e2e4", "d7d5", "e4d5", "g8f6"])
                r = capture(page, "04-desktop-capture-session-hud", "Actual corrected session/history guidance after same public sequence")
                normalized = re.sub(r"[^a-z0-9]", "", r["ocr_analysis"]["text"].lower())
                check("capture sequence public HUD coordinate observed", "e4" in normalized and "d5" in normalized and "f6" in normalized, {"oracle":"Raw OCR of original public rendered pixels; same assertion retained from old run"})
            finally:
                context.close()
            context, page = start(browser, origin)
            try:
                sequence(page,["f2f3","e7e5","g2g4","d8h4"])
                r = capture(page,"19-checkmate-result","Actual corrected Black wins/checkmate/terminal detail after same public inputs")
                check("terminal visible public result", "checkmate" in r["ocr_analysis"]["text"].lower(), {"oracle":"Raw Tesseract result from original screenshot; same assertion retained"})
            finally:
                context.close()
        attempt("corrected initial, history and result", desktop)
        def narrow():
            for width,height in [(390,844),(320,640)]:
                context,page = start(browser,origin,width,height)
                try:
                    capture(page,f"narrow-{width}-initial","Actual corrected narrow player/status wording and layout")
                    move(page,"e2e4")
                    capture(page,f"narrow-{width}-after-e2e4","Actual narrow corrected Black-turn/player/status layout")
                finally:
                    context.close()
        attempt("390px and320px wrapping",narrow)
        browser.close()
    data["status"] = "PASS" if all(x["status"] == "PASS" for x in data["checks"]) and all(x["status"] == "CAPTURED" for x in data["cases"]) else "PARTIAL"
    data["finished_at_utc"] = utc()
    save()
    assert len(data["captures"]) == 7, "bounded required screenshot coverage unavailable"


if __name__ == "__main__":
    main()
