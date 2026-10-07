"""Actual PR114 entry/Chessnut WASM screenshots in authorized isolated CI."""
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
import subprocess
import tempfile

from PIL import Image
from playwright.sync_api import sync_playwright

HEAD = "8ea4d9bae8ca377ed189e103218dc57e73cfcd0a"
TREE = "8ce161e210510d8fe693c38e2bf428ba07588ba6"


def utc():
    return datetime.now(timezone.utc).isoformat()


def layout(w, h):
    """PR114/base80d9 geometry only for pointers, not a rules oracle."""
    margin, gap, title = min(w * .035, h * .025, 24), min(h * .008, 8), min(h * .06, 40)
    player = min(max(h * .07, 44 if h >= 450 else 24), 56)
    rail = (w >= 760 and h >= 420) or (w >= 600 and h < 420)
    rail_w = min(w * .28, 360) if rail else 0
    game_w = max(w - margin * 2 - rail_w - (gap * 2 if rail else 0), 0)
    columns = max(math.floor((game_w + 4) / 76), 1)
    controls_h = math.ceil(6 / columns) * 48 - 4 if h >= 200 and w >= 280 and not (rail and h < 450) else 0
    coord = min(game_w * .03, 12 if h < 420 else 16)
    status_h = 0 if rail else min(h * .1, 64)
    side = min(max(game_w - coord * 2, 0), max(h - margin * 2 - title - player * 2 - controls_h - status_h - gap * 6 - coord * 2, 0), 680)
    left, top = margin + (game_w - side) / 2, margin + title + gap + player + gap + coord
    controls_y = top + side + coord + gap + player + gap
    return {"width": w, "height": h, "left": left, "top": top, "side": side,
            "square": side / 8, "controls_x": margin, "controls_y": controls_y,
            "controls_width": game_w, "mapping_source": "PR114 BoardLayout::oriented; full actual standalone playing canvas, no online Rust dock subtraction"}


def square(g, name, flipped=False):
    f, r = ord(name[0]) - ord("a"), int(name[1]) - 1
    assert 0 <= f < 8 and 0 <= r < 8
    col, row = (7 - f, r) if flipped else (f, 7 - r)
    return g["left"] + (col + .5) * g["square"], g["top"] + (row + .5) * g["square"]


def flip(g):
    width = (g["controls_width"] - 8) / 3
    return g["controls_x"] + width / 2, g["controls_y"] + 22


def promotion(g, index=None):
    b = min(max(g["square"] * .9, 60), (g["width"] - 30) / 4, g["height"] - 108)
    assert b >= 44
    panel_w, panel_h = b * 4 + 30, b + 108
    x, y = (g["width"] - panel_w) / 2, g["height"] - panel_h if g["width"] < 600 else (g["height"] - panel_h) / 2
    if index is None:
        return g["width"] / 2, y + 44 + b + 8 + 22
    return x + 12 + index * (b + 2) + b / 2, y + 44 + b / 2


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-root", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()
    assert os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("TABULA_CLASSIC_GRAPHICS_QA") == "1"
    root, out = args.source_root.resolve(), args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    git = lambda *a: subprocess.check_output(["git", *a], cwd=root, text=True).strip()
    assert git("rev-parse", "HEAD") == HEAD and git("rev-parse", "HEAD^{tree}") == TREE
    assert not git("status", "--porcelain", "--untracked-files=all"), "actual source is not clean"
    spec = importlib.util.spec_from_file_location("maintained", root / "tools/dashboard-acceptance/run.py")
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    shell_dist, game_dist = root / "apps/web/dist", root / "target/tabula-web-game"
    data = {"source_commit": HEAD, "source_tree": TREE, "base_commit": "80d9fdb96f18cd59fa85c9401b533d66bf04b5d7", "pr": 114,
            "harness_commit": os.environ["GITHUB_SHA"], "run_id": os.environ["GITHUB_RUN_ID"], "started_at_utc": utc(),
            "playwright": version("playwright"), "tool_versions": {"rustc": subprocess.check_output(["rustc", "--version"], text=True).strip()},
            "fixture": "Public static anonymous Leptos entry, synthetic never-submitted join-code input, and disposable untimed same-device local Chess. No real account, session, canonical state, FEN, API response or CSS injected",
            "scope": "Source-separated actual PR114, not stacked on PR112. Desktop Chromium CSS viewports/DPR only; physical mobile/native/CMP/online admission/paste permission/performance NOT_RUN",
            "builds": {}, "captures": [], "actions": [], "checks": [], "cases": [],
            "unrun": ["Authenticated create/join HTTPS/PostgreSQL fixture", "Clipboard permission/actual paste", "Native/device/CMP", "Audio/performance/frame pacing"]}
    for name, dist in [("shell", shell_dist), ("game", game_dist)]:
        data["builds"][name] = []
        for p in sorted(dist.rglob("*")):
            if p.is_file():
                b = p.read_bytes()
                data["builds"][name].append({"path": p.relative_to(dist).as_posix(), "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()})

    def save():
        (out / "classic-provenance.json").write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")

    def check(name, ok, facts):
        data["checks"].append({"name": name, "status": "PASS" if ok else "FAIL", "facts": facts})
        save()

    def entry_metrics(page):
        return page.evaluate("""() => {
          const input=document.querySelector('[data-testid=online-join-code]'), create=document.querySelector('[data-testid=online-create]'), join=document.querySelector('[data-testid=online-join]'), status=document.querySelector('[data-testid=online-status]');
          const box=e=>{if(!e)return null;const r=e.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom}};
          return {root_font_px:parseFloat(getComputedStyle(document.documentElement).fontSize),body_scroll_width:document.documentElement.scrollWidth,viewport_width:innerWidth,
            code:input?{length:input.value.length,uppercase:input.value===input.value.toUpperCase(),max_length:input.maxLength,read_only:input.readOnly,focused:input===document.activeElement,labels:input.labels?.length??0,box:box(input),font_px:parseFloat(getComputedStyle(input).fontSize)}:null,
            create:create?{disabled:create.disabled,box:box(create)}:null,join:join?{disabled:join.disabled,box:box(join)}:null,
            status:status?{text:status.textContent,role:status.getAttribute('role'),box:box(status)}:null,
            quick_unavailable:!!document.querySelector('[data-testid=online-quick-unavailable]'),scroll:{x:scrollX,y:scrollY}};
        }""")

    def capture(page, filename, label, kind="game", full=False, extra=None):
        p = out / (filename + ".png")
        began = utc()
        b = page.screenshot(path=str(p), full_page=full)
        ended = utc()
        with Image.open(p) as im:
            im.verify()
        with Image.open(p) as im:
            dimensions = list(im.size)
            colors = len(im.convert("RGB").getcolors(im.width * im.height) or [])
        r = {"file": p.name, "label": label, "kind": kind, "capture_started_at_utc": began, "captured_at_utc": ended,
             "route_path": urlsplit(page.url).path, "viewport_css_pixels": page.viewport_size, "png_pixels": dimensions,
             "dpr": page.evaluate("devicePixelRatio"), "scheme": page.locator("html").get_attribute("data-theme"),
             "locale": page.locator("html").get_attribute("lang"), "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest(),
             "original_png_unchanged": True, "actions_through_index": len(data["actions"]), "extra": extra}
        if kind == "entry":
            r["entry_metrics_after_paint"] = entry_metrics(page)
        else:
            r["canvas_bounds_after_paint"] = page.locator("#glcanvas").bounding_box()
            r["canvas_intrinsic_after_paint"] = page.locator("#glcanvas").evaluate("e=>({width:e.width,height:e.height})")
            check(filename + " actual rendered colors", colors > 100, {"unique_rgb_colors": colors})
        data["captures"].append(r)
        save()
        return r

    def attempt(name, operation):
        count = len(data["captures"])
        try:
            operation()
            data["cases"].append({"name": name, "status": "CAPTURED", "new_captures": len(data["captures"]) - count})
        except Exception as error:
            data["cases"].append({"name": name, "status": "BLOCKED", "error_type": type(error).__name__, "new_captures": len(data["captures"]) - count})
        save()

    def geometry(page):
        b = page.locator("#glcanvas").bounding_box()
        assert b and b["width"] > 0 and b["height"] > 0
        return layout(b["width"], b["height"])

    def click(page, point, action):
        page.locator("#glcanvas").click(position={"x": point[0], "y": point[1]}, delay=75)
        data["actions"].append({"at_utc": utc(), "action": action, "canvas_local_css_point": list(point)})
        page.wait_for_timeout(140)
        save()

    def move(page, name):
        g = geometry(page)
        click(page, square(g, name[:2]), "select " + name[:2])
        click(page, square(g, name[2:]), "destination " + name[2:])
        page.wait_for_timeout(200)

    def sequence(page, moves):
        for name in moves:
            move(page, name)

    def local(browser, origin, width=1200, height=880, theme="light", dpr=1):
        ctx = browser.new_context(viewport={"width": width, "height": height}, device_scale_factor=dpr)
        page = ctx.new_page()
        page.emulate_media(color_scheme="dark" if "dark" in theme else "light", contrast="more" if theme.startswith("hc-") else "no-preference", reduced_motion="reduce")
        page.goto(origin + "/index.html", wait_until="networkidle")
        page.locator("#locale").select_option("en")
        page.locator("#clock").select_option("untimed")
        page.locator("details.preferences summary").click()
        page.locator("#theme").select_option(theme)
        page.locator("#motion").select_option("reduced")
        page.locator("#setup button[type=submit]").click()
        page.locator("#loader").wait_for(state="hidden", timeout=120000)
        page.locator("#glcanvas").wait_for(state="visible")
        page.wait_for_timeout(350)
        check(f"actual startup {width}/{dpr}/{theme}", page.locator("#runtime-error").is_hidden(), {"actual_scheme": page.locator("html").get_attribute("data-theme")})
        data["actions"].append({"at_utc": utc(), "action": "Actual local setup launch", "viewport": page.viewport_size, "theme": theme, "dpr": dpr, "clock": "untimed", "motion": "reduced"})
        save()
        return ctx, page

    try:
        budget = helper.emitted_shell_budget(shell_dist)
        check("existing unchanged shell WASM loading budget", True, budget)
    except Exception as error:
        check("existing unchanged shell WASM loading budget", False, {"exception_type": type(error).__name__, "owner": "tools/tests/check-loading-budgets.py; not waived"})

    with helper.static_origin(shell_dist, shell=True) as shell, helper.static_origin(game_dist, shell=False) as game, sync_playwright() as p:
        browser = p.chromium.launch(headless=True, args=["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"])
        data["browser_version"] = browser.version
        save()
        def entries():
            for theme, color, contrast in [("light", "light", "no-preference"), ("dark", "dark", "no-preference")]:
                ctx = browser.new_context(viewport={"width": 1440, "height": 1000}, device_scale_factor=1)
                try:
                    page = ctx.new_page()
                    page.emulate_media(color_scheme=color, contrast=contrast, reduced_motion="reduce")
                    helper.settle(page, shell + "/games")
                    helper.locale(page, "vi")
                    page.locator('a[href="/games/com.tabula.chess"]').first.click()
                    page.locator('[data-testid=online-create]').wait_for(state="visible")
                    page.wait_for_timeout(250)
                    capture(page, "entry-desktop-" + theme, "Actual anonymous static Chess entry; authority unavailable, no admission", "entry", True)
                    m = entry_metrics(page)
                    check("anonymous create/join actual disabled " + theme, m["create"]["disabled"] and m["join"]["disabled"], {"create_disabled": m["create"]["disabled"], "join_disabled": m["join"]["disabled"], "quick_unavailable": m["quick_unavailable"]})
                    if theme == "light":
                        match_posts = []
                        page.on("request", lambda req: match_posts.append(urlsplit(req.url).path) if req.method == "POST" and "/matches" in urlsplit(req.url).path else None)
                        code = page.locator('[data-testid=online-join-code]')
                        code.click()
                        code.press_sequentially("ab12cd34ef56zz", delay=25)
                        page.wait_for_timeout(100)
                        m = entry_metrics(page)
                        check("native input uppercase/max12/label", m["code"]["length"] == 12 and m["code"]["uppercase"] and m["code"]["max_length"] == 12 and m["code"]["labels"] == 1, m["code"])
                        code.press("Enter")
                        page.wait_for_timeout(150)
                        check("anonymous Enter creates no match POST", not match_posts, {"match_post_count": len(match_posts)})
                        capture(page, "entry-desktop-synthetic-code-focused", "Actual keyboard-typed synthetic code, never submitted; no real invitation", "entry", True, {"synthetic_only": True})
                        page.go_back()
                        page.wait_for_url("**/games")
                        page.go_forward()
                        page.locator('[data-testid=online-join-code]').wait_for(state="visible")
                        helper.locale(page, "en")
                        capture(page, "entry-desktop-forward-en", "Actual browser Back/Forward and source locale control", "entry", True)
                finally:
                    ctx.close()
            for width, height, locale in [(390, 844, "vi"), (320, 640, "en")]:
                ctx = browser.new_context(viewport={"width": width, "height": height}, device_scale_factor=1, reduced_motion="reduce")
                try:
                    page = ctx.new_page()
                    helper.settle(page, shell + "/games/com.tabula.chess")
                    helper.locale(page, locale)
                    page.locator('[data-testid=online-create]').wait_for(state="visible")
                    capture(page, f"entry-{width}-{locale}", "Actual narrow anonymous create/join and unavailable quick-match UI", "entry", True)
                finally:
                    ctx.close()
        attempt("anonymous real entry, native input and route/locale", entries)

        def font_entry():
            with tempfile.TemporaryDirectory(prefix="classic-entry-font32-") as temp:
                profile = Path(temp)
                (profile / "Default").mkdir()
                (profile / "Default/Preferences").write_text(json.dumps({"webkit": {"webprefs": {"default_font_size": 32, "default_fixed_font_size": 26, "minimum_font_size": 0}}, "profile": {"default_zoom_level": 0}}))
                ctx = p.chromium.launch_persistent_context(str(profile), channel="chromium", headless=True, viewport={"width": 320, "height": 640}, device_scale_factor=1, reduced_motion="reduce")
                try:
                    page = ctx.pages[0]
                    actual = page.evaluate("parseFloat(getComputedStyle(document.documentElement).fontSize)")
                    helper.require_font_preference(actual, 32)
                    helper.settle(page, shell + "/games/com.tabula.chess")
                    helper.locale(page, "en")
                    capture(page, "entry-320-en-font200", "Actual Chromium 32px default-font preference, anonymous entry; no CSS scaling injected", "entry", True, {"initial_font_px": actual})
                finally:
                    ctx.close()
        attempt("actual 200-percent default font entry", font_entry)

        def boards():
            for theme in ("light", "dark", "hc-light", "hc-dark"):
                ctx, page = local(browser, game, theme=theme)
                try:
                    r = capture(page, "board-desktop-" + theme, "Actual Chessnut0.3 pieces in " + theme)
                    check("actual requested board scheme " + theme, r["scheme"] == theme, {"scheme": r["scheme"]})
                    if theme == "light":
                        click(page, square(geometry(page), "e2"), "select e2")
                        page.keyboard.press("ArrowRight")
                        capture(page, "board-selected-keyboard-focus", "Actual selection/legal hints plus native directional keyboard focus")
                        page.keyboard.press("Escape")
                        click(page, flip(geometry(page)), "Flip local display")
                        capture(page, "board-flipped-upright", "Actual reversed coordinates; piece artwork remains upright")
                        click(page, flip(geometry(page)), "Flip back")
                        sequence(page, ["e2e4", "d7d5", "e4d5", "g8f6"])
                        capture(page, "board-after-capture", "Actual legal public capture sequence, not injected state")
                finally:
                    ctx.close()
            for width, height, dpr in [(1200, 880, 2), (390, 844, 1), (390, 844, 2), (320, 640, 1)]:
                ctx, page = local(browser, game, width, height, dpr=dpr)
                try:
                    capture(page, f"board-{width}-dpr{dpr}", "Actual local board at CSS viewport and recorded DPR")
                    if width == 390 and dpr == 1:
                        move(page, "e2e4")
                        capture(page, "board-390-after-e2e4", "Actual narrow pointer input and Black-turn board")
                finally:
                    ctx.close()
        attempt("four themes, selection/focus, both orientation, small/DPR boards", boards)

        def promotions():
            ctx, page = local(browser, game)
            try:
                sequence(page, ["a2a4", "h7h5", "a4a5", "h5h4", "a5a6", "h4h3", "a6b7", "h3g2", "b7a8"])
                capture(page, "board-white-promotion-chooser", "Actual centered classic-base White promotion chooser")
                click(page, promotion(geometry(page)), "Cancel promotion")
                capture(page, "board-promotion-cancelled", "Actual Cancel keeps pawn at b7 and rook at a8")
                move(page, "b7a8")
                click(page, promotion(geometry(page), 0), "Choose White Queen")
                move(page, "g2h1")
                capture(page, "board-black-promotion-chooser", "Actual Black promotion chooser using licensed pieces")
                click(page, promotion(geometry(page), 3), "Choose Black Knight")
                page.wait_for_timeout(250)
                capture(page, "board-both-promotions", "Actual White Queen a8 and Black Knight h1")
            finally:
                ctx.close()
        attempt("both colors promotion with real Cancel/choice", promotions)

        def check_and_result():
            for filename, moves in [("board-live-check", ["e2e4", "f7f6", "d1h5"]), ("board-checkmate-result", ["f2f3", "e7e5", "g2g4", "d8h4"])]:
                ctx, page = local(browser, game)
                try:
                    sequence(page, moves)
                    capture(page, filename, "Actual ordinary public opening sequence and resulting check/result pixels")
                finally:
                    ctx.close()
        attempt("reachable live check and terminal", check_and_result)
        browser.close()
    data["status"] = "PASS" if all(x["status"] == "PASS" for x in data["checks"]) and all(x["status"] == "CAPTURED" for x in data["cases"]) else "PARTIAL"
    data["finished_at_utc"] = utc()
    save()
    assert any(c["kind"] == "game" for c in data["captures"]), "no actual runtime board captured"


if __name__ == "__main__":
    main()
