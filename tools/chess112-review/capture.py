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

EXPECTED = "c61dbc62271cd4a5554dd62bc139a06bab58fb44"
TREE = "0e1ec1821b396a72250c6e436586c4d1a7cb4a86"


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
            "base_commit": "face8a3e058e42fc846efa32082a9b76efaef3c2",
            "harness_commit": os.environ["GITHUB_SHA"], "run_id": os.environ["GITHUB_RUN_ID"],
            "started_at_utc": utc(), "playwright": version("playwright"),
            "fixture": "Disposable untimed same-device local Chess. Public legal input sequences only; no accounts, credentials, online service, FEN or canonical state injected/read",
            "scope": "Actual source-owned setup and compiled Macroquad/WASM in desktop Chromium viewports; native/mobile/CMP/performance NOT_RUN",
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
        save()
        def initial_cases():
            for theme in ("light", "dark", "hc-light", "hc-dark"):
                context, page = start(browser, origin, theme=theme, label="00-desktop-setup" if theme == "light" else None)
                try:
                    r = capture(page, "01-desktop-" + theme + "-initial", "Actual local White/Black upright initial board in " + theme)
                    check("requested actual theme " + theme, r["scheme"] == theme, {"actual": r["scheme"]})
                    if theme == "light":
                        click(page, square(geom(page), "e2"), "select e2")
                        capture(page, "02-desktop-selected-e2", "Actual selected e2 pawn and projected legal destinations")
                        page.keyboard.press("Escape")
                        click(page, control(geom(page), 0), "local Flip control")
                        capture(page, "03-desktop-flipped-upright", "Actual reversed board coordinates; artwork stays upright")
                        click(page, control(geom(page), 0), "local Flip back")
                        sequence(page, ["e2e4", "d7d5", "e4d5", "g8f6"])
                        r = capture(page, "04-desktop-capture-session-hud", "Actual attempted legal capture sequence and session HUD")
                        normalized = re.sub(r"[^a-z0-9]", "", r["ocr_analysis"]["text"].lower())
                        check("capture sequence public HUD coordinate observed", "e4" in normalized and "d5" in normalized and "f6" in normalized, {"oracle": "OCR of original public rendered pixels; human inspection still required"})
                finally:
                    context.close()
        attempt("four themes, selection, flip and capture HUD", initial_cases)

        def narrow_cases():
            context, page = start(browser, origin, 390, 844)
            try:
                capture(page, "05-narrow-initial", "Actual390px viewport local board and compact controls")
                click(page, control(geom(page), 1), "open compact Actions")
                capture(page, "06-narrow-actions-open", "Actual disclosed local Actions popup")
                g = geom(page)
                # Opening contains two action specs; click actual panel padding.
                columns = max(math.floor((g["side"] - 12) / 100), 1)
                panel_height = math.ceil(2 / columns) * 48 + 12
                click(page, (g["left"] + 3, max(g["controls_y"] - panel_height - 8, 8) + 3), "click popup padding; no board move intended")
                capture(page, "07-narrow-actions-padding-shield", "Actual popup padding click, followed by original rendering")
                page.keyboard.press("Escape")
                capture(page, "08-narrow-actions-dismissed", "Actual Escape dismissal and board restoration")
                move(page, "e2e4")
                capture(page, "09-narrow-after-e2e4", "Actual narrow board after legal opening pointer input")
            finally:
                context.close()
            context, page = start(browser, origin, 320, 640, theme="dark")
            try:
                capture(page, "10-small-dark-initial", "Actual320px dark viewport initial rendering")
                click(page, control(geom(page), 1), "open small compact Actions")
                capture(page, "11-small-dark-actions", "Actual320px Actions disclosure")
            finally:
                context.close()
        attempt("narrow Actions shield and dismissal", narrow_cases)

        def special_case(name, moves, filename):
            context, page = start(browser, origin)
            try:
                sequence(page, moves)
                capture(page, filename, "Actual legal public " + name + " sequence; resulting pixels require inspection")
            finally:
                context.close()
        attempt("en passant", lambda: special_case("en passant", ["e2e4", "a7a6", "e4e5", "d7d5", "e5d6"], "12-desktop-en-passant"))
        attempt("both castles", lambda: special_case("White and Black castle", ["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5", "d2d3", "g8f6", "e1g1", "e8g8"], "13-desktop-both-castles"))

        def promotion_cases():
            context, page = start(browser, origin)
            try:
                sequence(page, ["a2a4", "h7h5", "a4a5", "h5h4", "a5a6", "h4h3", "a6b7", "h3g2", "b7a8"])
                capture(page, "14-white-promotion-chooser", "Actual White promotion chooser after public opening sequence")
                click(page, promotion(geom(page), "a8"), "Cancel White promotion chooser")
                capture(page, "15-white-promotion-cancelled", "Actual chooser Cancel returns to unchanged public position")
                move(page, "b7a8")
                click(page, promotion(geom(page), "a8", 0), "Choose White Queen")
                page.wait_for_timeout(200)
                capture(page, "16-white-promoted-queen", "Actual committed White Queen choice")
                move(page, "g2h1")
                capture(page, "17-black-promotion-chooser", "Actual Black promotion chooser")
                click(page, promotion(geom(page), "h1", 3), "Choose Black Knight")
                page.wait_for_timeout(200)
                capture(page, "18-black-promoted-knight", "Actual committed Black Knight choice")
            finally:
                context.close()
        attempt("both colors promotion and explicit cancel", promotion_cases)

        def terminal_case():
            context, page = start(browser, origin)
            try:
                sequence(page, ["f2f3", "e7e5", "g2g4", "d8h4"])
                r = capture(page, "19-checkmate-result", "Actual Fool's mate pointer sequence, terminal board and public result")
                check("terminal visible public result", "checkmate" in r["ocr_analysis"]["text"].lower(), {"oracle": "Tesseract of original screenshot, not canonical state"})
            finally:
                context.close()
        attempt("Fool's mate check and result", terminal_case)

        def motion_cases():
            for reduced in (False, True):
                context, page = start(browser, origin, reduced=reduced)
                try:
                    g = geom(page)
                    click(page, square(g, "e2"), "select e2 for motion comparison")
                    # Snapshot timing is observational and not a frame-pacing benchmark.
                    point = square(g, "e4")
                    page.locator("#glcanvas").click(position={"x": point[0], "y": point[1]}, delay=20)
                    data["actions"].append({"at_utc": utc(), "action": "e4 destination motion comparison", "canvas_local_css_point": list(point), "mode": "reduced" if reduced else "full"})
                    capture(page, "20-" + ("reduced" if reduced else "full") + "-motion-immediate", "Actual immediate screenshot following accepted-input attempt; observed timing only")
                    page.wait_for_timeout(350)
                    capture(page, "21-" + ("reduced" if reduced else "full") + "-motion-settled", "Actual settled board under requested motion preference")
                finally:
                    context.close()
        attempt("full and reduced motion observations", motion_cases)
        browser.close()
    data["status"] = "PASS" if all(x["status"] == "PASS" for x in data["checks"]) and all(x["status"] == "CAPTURED" for x in data["cases"]) else "PARTIAL"
    data["finished_at_utc"] = utc()
    save()
    assert len(data["captures"]) >= 5, "actual basic screenshot coverage unavailable"


if __name__ == "__main__":
    main()
