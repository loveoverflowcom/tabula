#!/usr/bin/env python3
"""Bounded, genuine Leptos/Design-01 browser acceptance in authorized GitHub CI.

This is not a fallback around a denied local-browser boundary. It serves actual
Trunk output and the byte-preserved prototype, never mocks HTTP or injects layout
CSS. Public screenshots and measurements contain no account/session authority.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import functools
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import threading
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright

VIEWPORTS = (
    ("desktop", 1440, 1000),
    ("review", 1100, 850),
    ("mobile", 390, 844),
    ("small-mobile", 320, 640),
    ("tablet", 768, 900),
    ("low-landscape", 844, 390),
)
SCHEMES = (
    ("light", "light", "no-preference"),
    ("dark", "dark", "no-preference"),
    ("hc-light", "light", "more"),
    ("hc-dark", "dark", "more"),
)
FONT_VIEWPORTS = (("mobile", 390, 844), ("small-mobile", 320, 640))
FONT_LANGUAGES = ("vi", "en")
NARROW_LABELS = {
    "en": {"cta": ["Explore games"], "navigation": ["Home", "Library", "Account"]},
    "vi": {"cta": ["Khám phá game"], "navigation": ["Trang chính", "Thư viện", "Tài khoản"]},
}
FORBIDDEN_FIXTURES = ("Minh Anh", "Mạnh Duy", "05:42", "Xiangqi Tutor")
HEAVY_RESOURCE = re.compile(r"(?:/play/|atlas|role[-_]?pack|\.(?:onnx|gguf|ggml|safetensors|gltf|glb|bin|pt|pth|tbr)(?:$|\?))", re.I)


class AcceptanceFailure(Exception):
    """Public, fixed claim failure. No request body or identity is recorded."""


class PrerequisiteBlocker(AcceptanceFailure):
    """The intended real-browser condition was not established; no app verdict."""


def require(condition: bool, label: str) -> None:
    if not condition:
        raise AcceptanceFailure(label)


def require_font_preference(measured: float, expected: int):
    if abs(measured - expected) > 0.1:
        raise PrerequisiteBlocker("actual Chromium did not apply requested default font preference")


def require_skip_concealed(metrics: dict):
    """The whole unfocused box is above the viewport, while native Tab remains usable."""
    require(metrics["count"] == 1 and not metrics["focused"], "initial skip-link state is not unique and unfocused")
    require(metrics["display"] != "none" and metrics["visibility"] == "visible"
            and metrics["tab_index"] >= 0 and not metrics["inert_or_hidden"],
            "concealing skip link removed its keyboard/accessibility path")
    box = metrics["rect"]
    require(box["width"] > 0 and box["height"] > 0 and box["bottom"] <= -1,
            "unfocused wrapped skip link leaks into the viewport")


SKIP_STATE = """() => {
 const links=document.querySelectorAll('.skip-link'),el=links[0];
 if(!el)return {count:links.length,focused:false};
 const b=el.getBoundingClientRect(),c=getComputedStyle(el);
 return {count:links.length,focused:el===document.activeElement,display:c.display,visibility:c.visibility,
   tab_index:el.tabIndex,inert_or_hidden:!!el.closest('[inert],[aria-hidden="true"]'),
   rect:{x:b.x,y:b.y,width:b.width,height:b.height,bottom:b.bottom,right:b.right}};
}"""


def luminance(rgb: tuple[float, float, float]) -> float:
    def channel(value):
        value /= 255
        return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4
    red, green, blue = map(channel, rgb)
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast_ratio(first: tuple[float, float, float], second: tuple[float, float, float]) -> float:
    bright, dark = sorted((luminance(first), luminance(second)), reverse=True)
    return (bright + 0.05) / (dark + 0.05)


def is_heavy_resource(url: str, shell_wasm_paths: set[str]) -> bool:
    path = urlsplit(url).path
    return bool(HEAVY_RESOURCE.search(path)) or (path.endswith(".wasm") and path not in shell_wasm_paths)


def usable_target(width: float, height: float) -> bool:
    return width >= 43.5 and height >= 43.5


def file_manifest(root: Path) -> list[dict]:
    return [{"path": str(file.relative_to(root)), "bytes": file.stat().st_size,
             "sha256": hashlib.sha256(file.read_bytes()).hexdigest()}
            for file in sorted(root.rglob("*")) if file.is_file()]


def verify_reference(root: Path) -> dict:
    """Bind the visual oracle to its independently archived byte/hash receipt."""
    archive = root.parent
    receipt = json.loads((archive / "PROVENANCE.json").read_text())
    files = receipt.get("files", [])
    require(files, "reference provenance selection empty")
    for item in files:
        path = (archive / item["path"]).resolve()
        require(path.is_relative_to(archive.resolve()) and path.is_file(), "reference file missing or outside archive")
        payload = path.read_bytes()
        require(len(payload) == item["bytes"] and hashlib.sha256(payload).hexdigest() == item["sha256"],
                "archived reference bytes/hash do not match provenance")
    return {"design": receipt["design"], "file_count": len(files), "all_archived_hashes_verified": True}


def shell_wasm_paths(root: Path) -> set[str]:
    return {"/" + str(file.relative_to(root)) for file in root.glob("*.wasm")}


def load_shell_handler():
    path = Path(__file__).resolve().parents[1] / "serve-local-shell.py"
    spec = importlib.util.spec_from_file_location("dashboard_real_shell_server", path)
    require(spec is not None and spec.loader is not None, "existing shell static handler missing")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.LocalShellHandler


def load_loading_budgets():
    path = Path(__file__).resolve().parents[1] / "tests" / "check-loading-budgets.py"
    spec = importlib.util.spec_from_file_location("dashboard_existing_loading_budgets", path)
    require(spec is not None and spec.loader is not None, "existing loading budget owner missing")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def emitted_shell_budget(root: Path) -> dict:
    """Use the exact existing shell_budget and its shell-only raw WASM limit."""
    owner = load_loading_budgets()
    try:
        result = owner.shell_budget(root)
    except AssertionError as error:
        raise AcceptanceFailure(str(error)) from None
    return {"budget_owner": "tools/tests/check-loading-budgets.py::shell_budget",
            "shell_raw_wasm_limit": owner.SHELL_RAW_LIMIT, **result}


@contextmanager
def static_origin(root: Path, *, shell: bool):
    """Real bytes + existing documented SPA routing, no manufactured API reply."""
    base = load_shell_handler() if shell else SimpleHTTPRequestHandler

    class Handler(base):
        def log_message(self, _format, *_args):
            pass

    handler = functools.partial(Handler, directory=str(root))
    with ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield f"http://127.0.0.1:{server.server_port}"
        finally:
            server.shutdown()
            thread.join(timeout=10)


def locale(page, value="vi"):
    control = page.locator("#locale")
    require(control.count() == 1, "real locale control missing")
    control.select_option(value)
    page.wait_for_function("value => document.documentElement.lang === value", arg=value)


def settle(page, url: str):
    response = page.goto(url, wait_until="networkidle", timeout=60_000)
    require(response is not None and response.status == 200, "actual document did not load")
    page.locator("main").wait_for(state="visible", timeout=60_000)
    page.evaluate("document.fonts.ready")


def require_filtered_empty(page):
    """The results' own announced empty state; continue status cannot satisfy it."""
    require(page.locator(".card").count() == 0, "unmatched search is not empty")
    status = page.locator(".catalog__empty [role=status], .catalog__empty[role=status]")
    require(status.count() == 1 and status.first.is_visible() and bool(status.first.inner_text().strip()),
            "filtered results have no visible nonempty announced state")
    recovery = page.locator('.catalog__empty a[href="/games"]')
    require(recovery.count() == 1 and recovery.is_visible(), "filtered empty state has no real reset recovery")


MEASURE = """() => {
 const rect = el => {const b=el.getBoundingClientRect();return {x:b.x,y:b.y,width:b.width,height:b.height,bottom:b.bottom,right:b.right}};
 const visible = el => {const c=getComputedStyle(el),b=el.getBoundingClientRect();return c.display!=='none'&&c.visibility!=='hidden'&&c.clipPath==='none'&&b.width>0&&b.height>0&&!el.closest('[inert]')};
 const one = selector => {const el=document.querySelector(selector);return el&&visible(el)?rect(el):null};
 const text = selector => {const el=document.querySelector(selector); if(!el)return null;const c=getComputedStyle(el);return {font_family:c.fontFamily,font_size:parseFloat(c.fontSize),font_weight:c.fontWeight}};
 const controls=[...document.querySelectorAll('a,button,input,select,summary,[role="button"]')].filter(el=>visible(el)&&!el.classList.contains('skip-link')&&el.getBoundingClientRect().x>=0);
 const clip=[...document.querySelectorAll('h1,h2,h3,p,.btn,.nav-link')].filter(visible).filter(el=>el.scrollWidth>el.clientWidth+2&&['hidden','clip'].includes(getComputedStyle(el).overflowX)).map(el=>({tag:el.tagName,class:el.className}));
 // A word can overflow its own visible box and be cut by an ancestor instead.
 // Measure real text line rectangles; exclude intentionally decorative art and
 // visually hidden assistive labels, never insert CSS to force a result.
 for(const el of [...document.querySelectorAll('h1,h2,h3,p,small,.btn,.shell-link,.brand,.field__label')].filter(visible)){
   if(el.closest('[aria-hidden="true"]'))continue;
   const clips=[];for(let p=el;p;p=p.parentElement){const c=getComputedStyle(p);if(['hidden','clip'].includes(c.overflowX)||['hidden','clip'].includes(c.overflowY))clips.push({el:p,c,rect:p.getBoundingClientRect()})}
   if(!clips.length)continue;
   const walker=document.createTreeWalker(el,NodeFilter.SHOW_TEXT);let node,cut=false;
   while((node=walker.nextNode())&&!cut){if(!node.textContent.trim()||node.parentElement.closest('[aria-hidden="true"]'))continue;const range=document.createRange();range.selectNodeContents(node);for(const textRect of range.getClientRects())for(const parent of clips){const horizontal=['hidden','clip'].includes(parent.c.overflowX)&&(textRect.left<parent.rect.left-2||textRect.right>parent.rect.right+2);const vertical=['hidden','clip'].includes(parent.c.overflowY)&&(textRect.top<parent.rect.top-2||textRect.bottom>parent.rect.bottom+2);if(horizontal||vertical){clip.push({tag:el.tagName,class:el.className,ancestor_class:parent.el.className,reason:'text glyph line exceeds clipping ancestor'});cut=true;break}}range.detach()}
 }
 const nav=document.querySelector('.bottom-nav');
 const grid=document.querySelector('.cards');
 return {viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},document_width:document.documentElement.scrollWidth,body_width:document.body.scrollWidth,scroll_height:document.documentElement.scrollHeight,scroll_y:scrollY,
 sidebar:one('.sidebar'),topbar:one('.topbar'),hero:one('.feature-hero'),continue_strip:one('.continue-strip'),bottom_nav:one('.bottom-nav'),main:one('main'),
 main_bottom_padding:parseFloat(getComputedStyle(document.querySelector('main')).paddingBottom),bottom_nav_position:nav?getComputedStyle(nav).position:null,
 cards:[...document.querySelectorAll('.card')].filter(visible).map(rect),grid_columns:grid?getComputedStyle(grid).gridTemplateColumns:null,
 title:text('h1'),body:text('body'),controls:controls.map(el=>({tag:el.tagName,class:el.className,disabled:!!el.disabled,rect:rect(el)})),clipped_text:clip,
 scheme:document.documentElement.dataset.theme,lang:document.documentElement.lang};
}"""

# Only solid, actually computed foreground/background pairs are contrast oracles.
# Decorative artwork and gradients are reviewed as pixels, not guessed here.
CONTRAST = """() => {
 const parse=color=>{const encoded=color.startsWith('color(srgb '),legacy=/^rgba?\\(/.test(color);if(!encoded&&!legacy)return null;const p=color.match(/[\\d.]+/g)?.map(Number),scale=encoded?255:1;return p&&p.length>=3?[p[0]*scale,p[1]*scale,p[2]*scale,p[3]??1]:null};
 const background=el=>{for(let p=el;p;p=p.parentElement){const c=getComputedStyle(p);if(c.backgroundImage!=='none')return null;const bg=parse(c.backgroundColor);if(bg&&bg[3]===1)return bg.slice(0,3);if(bg&&bg[3]!==0)return null}return [255,255,255]};
 return ['body','h1','.feature-hero h2','.feature-hero .btn--filled','.continue-strip','.card__title','.bottom-nav a'].flatMap(selector=>[...document.querySelectorAll(selector)].map(el=>{const c=getComputedStyle(el),b=el.getBoundingClientRect(),fg=parse(c.color),bg=background(el);return b.width&&b.height&&fg&&fg[3]===1&&bg?{selector,foreground:fg.slice(0,3),background:bg,font_size:parseFloat(c.fontSize),font_weight:Number(c.fontWeight)}:null}).filter(Boolean));
}"""


class Evidence:
    def __init__(self, output: Path):
        self.output = output
        self.records = []
        self.errors = []
        self.output.mkdir(parents=True, exist_ok=True)

    def case(self, name, operation):
        try:
            detail = operation()
            self.records.append({"claim": name, "status": "PASS", "detail": detail})
        except Exception as error:
            # No traceback/HTTP state is copied into public artifacts.
            label = str(error) if isinstance(error, AcceptanceFailure) else type(error).__name__
            self.records.append({"claim": name, "status": "BLOCKED" if isinstance(error, PrerequisiteBlocker) else "FAIL", "reason": label})
            self.errors.append(name)
        self.write()

    def write(self):
        (self.output / "acceptance.json").write_text(json.dumps({
            "evidence_kind": "interaction-tested + screenshot-captured",
            "status": "FAIL" if self.errors else "IN_PROGRESS",
            "claims": self.records,
            "limits": ["Screenshots require named visual inspection before screenshot-inspected status.",
                       "No provider login, authenticated identity/avatar, native/CMP or gameplay execution is claimed.",
                       "Continue is genuinely unavailable; ready/empty/error session states have no runtime adapter in this slice.",
                       "Catalog is synchronous registry data; asynchronous catalog loading/server-error cases are NOT_APPLICABLE.",
                       "Actual create/join Chess remains the separate online-match workflow on this exact revision."]}, indent=2) + "\n")

    def capture(self, page, name, full_page=False):
        path = self.output / f"{name}.png"
        page.screenshot(path=str(path), full_page=full_page, animations="disabled")
        require(path.is_file() and path.stat().st_size > 1000, "empty screenshot")
        return path.name


def assert_layout(metrics: dict, width: int, *, small=False):
    require(metrics["viewport"]["width"] == width, "wrong CSS viewport")
    require(metrics["viewport"]["dpr"] == 1, "comparison must use DPR1")
    require(max(metrics["document_width"], metrics["body_width"]) <= width + 1, "horizontal overflow")
    require(not metrics["clipped_text"], "text clipped by overflow")
    require(metrics["cards"], "nonempty catalog selection missing")
    if small:
        require(metrics["bottom_nav"] is not None, "compact bottom navigation missing")
        require(metrics["bottom_nav_position"] == "fixed", "mobile navigation is not fixed")
        require(metrics["main_bottom_padding"] >= metrics["bottom_nav"]["height"], "bottom navigation has no real content slot")
        require(all(usable_target(item["rect"]["width"], item["rect"]["height"])
                    for item in metrics["controls"] if not item["disabled"]), "enabled touch target below 44 CSS px")


def assert_narrow_usability(metrics: dict, width: int, initial_font_px: float, language: str, *, home: bool):
    """Require readable words and available copy width only at <=16 initial rem."""
    if width / initial_font_px > 16:
        return False
    expected = NARROW_LABELS[language]
    for name in ("navigation", "cta") if home else ("navigation",):
        labels = metrics[name]
        require([item["text"] for item in labels] == expected[name], "narrow CTA/navigation labels changed or missing")
        require(all(item["words"] and all(word["line_count"] == 1 and not word["outside_target"]
                    for word in item["words"]) for item in labels), "narrow CTA/navigation splits or obscures a word")
    if home:
        hero = metrics["hero"]
        require(hero is not None and hero["copy"] is not None and hero["heading"] is not None,
                "narrow hero copy missing")
        require(abs(hero["copy"]["width"] - hero["inner_width"]) <= 2,
                "narrow hero copy does not use full available width")
        require(hero["heading"]["width"] >= hero["copy_inner_width"] - 2,
                "narrow hero heading retains a restricted text column")
        require(not metrics["visible_hero_art"], "narrow hero decoration competes with full-width copy")
    continuation = metrics["continue"]
    require(continuation is not None and continuation["icon"] is not None and continuation["copy"] is not None,
            "narrow Continue content missing")
    require(continuation["copy"]["y"] >= continuation["icon"]["bottom"] - 2,
            "narrow Continue copy remains beside the icon")
    require(abs(continuation["copy"]["width"] - continuation["inner_width"]) <= 2,
            "narrow Continue copy does not use full available width")
    return True


def assert_font_scaling(observations: list[dict]):
    expected = {(scale, language, width) for scale in (1, 2) for language in FONT_LANGUAGES
                for _, width, _ in FONT_VIEWPORTS}
    rows = {(row["scale"], row["language"], row["viewport"]["width"]): row for row in observations}
    require(len(observations) == len(expected) and set(rows) == expected,
            "320/390 bilingual text-scale selection incomplete or duplicated")
    for _, width, _ in FONT_VIEWPORTS:
        for language in FONT_LANGUAGES:
            normal, doubled = rows[(1, language, width)], rows[(2, language, width)]
            for route in ("home", "library"):
                for text in ("title", "body"):
                    ratio = doubled[route][text]["font_size"] / normal[route][text]["font_size"]
                    require(1.95 <= ratio <= 2.05,
                            "display/body text does not double with genuine 200-percent browser default font size")


NARROW_USABILITY = r"""() => {
 const rect=el=>{const b=el.getBoundingClientRect();return {x:b.x,y:b.y,width:b.width,height:b.height,bottom:b.bottom,right:b.right}};
 const visible=el=>{const c=getComputedStyle(el),b=el.getBoundingClientRect();return c.display!=='none'&&c.visibility!=='hidden'&&b.width>0&&b.height>0};
 const innerWidth=el=>{const c=getComputedStyle(el);return el.getBoundingClientRect().width-parseFloat(c.paddingLeft)-parseFloat(c.paddingRight)-parseFloat(c.borderLeftWidth)-parseFloat(c.borderRightWidth)};
 const labels=selector=>[...document.querySelectorAll(selector)].filter(visible).map(el=>{
   const target=el.closest('a,button'),bounds=target.getBoundingClientRect(),words=[];
   const walker=document.createTreeWalker(el,NodeFilter.SHOW_TEXT);let node;
   while((node=walker.nextNode()))for(const match of node.textContent.matchAll(/\S+/gu)){
     const range=document.createRange();range.setStart(node,match.index);range.setEnd(node,match.index+match[0].length);
     const boxes=[...range.getClientRects()].filter(b=>b.width>0&&b.height>0),lines=[];
     for(const b of boxes)if(!lines.some(y=>Math.abs(y-b.top)<=2))lines.push(b.top);
     words.push({text:match[0],line_count:lines.length,outside_target:boxes.some(b=>b.left<bounds.left-2||b.right>bounds.right+2||b.top<bounds.top-2||b.bottom>bounds.bottom+2)});range.detach();
   }
   return {text:el.textContent.replace(/\u00ad/g,'').trim().replace(/\s+/gu,' '),words};
 });
 const hero=document.querySelector('.feature-hero'),copy=hero?.querySelector('.hero-copy'),heading=copy?.querySelector('h2');
 const continuation=document.querySelector('.continue-strip'),icon=continuation?.querySelector('.continue-strip__icon'),continueCopy=continuation?.querySelector(':scope > div');
 return {navigation:labels('.bottom-nav .shell-link > span'),cta:labels('.hero-copy > .btn > span'),
   hero:hero?{inner_width:innerWidth(hero),copy:copy?rect(copy):null,copy_inner_width:copy?innerWidth(copy):null,heading:heading?rect(heading):null}:null,
   visible_hero_art:[...document.querySelectorAll('.hero-art')].filter(visible).map(rect),
   continue:continuation?{inner_width:innerWidth(continuation),icon:icon?rect(icon):null,copy:continueCopy?rect(continueCopy):null}:null};
}"""


def traced_page(context, root: Path):
    page = context.new_page()
    requests, sizes, errors = [], [], []
    page.on("request", lambda request: requests.append(request.url))
    page.on("pageerror", lambda _error: errors.append("uncaught browser error"))
    session = context.new_cdp_session(page)
    session.send("Network.enable")
    session.on("Network.loadingFinished", lambda event: sizes.append(event.get("encodedDataLength", 0)))
    return page, requests, sizes, errors


def capture_comparison(browser, evidence, origins):
    result = []
    failures = []
    for name, width, height in VIEWPORTS:
        for source, origin in origins.items():
            context = browser.new_context(viewport={"width": width, "height": height}, device_scale_factor=1,
                                          locale="vi-VN", color_scheme="light", reduced_motion="reduce")
            try:
                page = context.new_page()
                settle(page, origin + ("/standalone.html#library" if source == "prototype" else "/"))
                if source != "prototype":
                    locale(page)
                captures = [evidence.capture(page, f"{source}-{name}-{width}x{height}")]
                if name == "desktop":
                    captures.append(evidence.capture(page, f"{source}-{name}-{width}x{height}-full", True))
                metrics = page.evaluate(MEASURE)
                if source == "after":
                    try:
                        assert_layout(metrics, width, small=width <= 768 or name == "low-landscape")
                        require(metrics["hero"] is not None, "home hero missing")
                        require(metrics["continue_strip"] is not None, "compact continue region missing")
                        if name == "desktop":
                            require(metrics["sidebar"] is not None and abs(metrics["sidebar"]["width"] - 224) <= 1,
                                    "desktop rail is not 224 CSS px")
                            require(metrics["topbar"] is not None and abs(metrics["topbar"]["height"] - 80) <= 1,
                                    "desktop topbar is not 80 CSS px")
                            require(len(metrics["grid_columns"].split()) == 3, "desktop catalog is not three columns")
                        content = page.locator("main").inner_text()
                        require(not any(value in content for value in FORBIDDEN_FIXTURES), "prototype fixture advertised as runtime fact")
                        require(page.locator(".card__art svg, .card__art img, .card__art canvas").count() >= len(metrics["cards"]),
                                "runtime card cover is empty")
                    except AcceptanceFailure as error:
                        failures.append({"viewport": name, "reason": str(error)})
                result.append({"source": source, "viewport": name, "captures": captures, "metrics": metrics})
                if source != "prototype":
                    # The original combines Home/Catalog. Keep that same visual
                    # oracle beside both real route mappings, without implying
                    # the prototype contains a separate product /games route.
                    settle(page, origin + "/games")
                    locale(page)
                    catalog_captures = [evidence.capture(page, f"{source}-catalog-{name}-{width}x{height}")]
                    if name == "desktop":
                        catalog_captures.append(evidence.capture(page, f"{source}-catalog-{name}-{width}x{height}-full", True))
                    catalog_metrics = page.evaluate(MEASURE)
                    if source == "after":
                        try:
                            assert_layout(catalog_metrics, width, small=width <= 768 or name == "low-landscape")
                            require(catalog_metrics["continue_strip"] is not None, "catalog continue region missing")
                            require(page.locator("#search").is_visible() and page.locator(".catalog__filters").is_visible(),
                                    "catalog search or supported filters missing")
                        except AcceptanceFailure as error:
                            failures.append({"viewport": name, "route": "/games", "reason": str(error)})
                    result.append({"source": source + "-catalog", "route": "/games", "viewport": name,
                                   "captures": catalog_captures, "metrics": catalog_metrics,
                                   "visual_reference": "prototype #library combines Home and Catalog"})
                (evidence.output / "comparison.json").write_text(json.dumps({"comparisons": result, "failures": failures}, indent=2) + "\n")
            finally:
                context.close()
    require(not failures, "responsive comparison has recorded layout failures")
    require(len(result) == len(VIEWPORTS) * 5, "comparison selection incomplete")
    return result


def preference_matrix(browser, origin, evidence):
    result = []
    for scheme, color, contrast in SCHEMES:
        for language in ("vi", "en"):
            context = browser.new_context(viewport={"width": 1100, "height": 850}, device_scale_factor=1)
            try:
                page = context.new_page()
                page.emulate_media(color_scheme=color, contrast=contrast, reduced_motion="reduce")
                settle(page, origin + "/")
                locale(page, language)
                require(page.locator("html").get_attribute("data-theme") == scheme, "system preference mapped to wrong scheme")
                pairs = page.evaluate(CONTRAST)
                require(len(pairs) >= 3, "solid contrast selection empty")
                measured = []
                for pair in pairs:
                    ratio = contrast_ratio(tuple(pair["foreground"]), tuple(pair["background"]))
                    large = pair["font_size"] >= 24 or (pair["font_size"] >= 18.667 and pair["font_weight"] >= 700)
                    require(ratio >= (3 if large else 4.5) - 0.02, "measured solid semantic text pair lacks contrast")
                    measured.append({**pair, "ratio": round(ratio, 3)})
                durations = page.evaluate("""() => [...document.querySelectorAll('*')].filter(e=>e.getBoundingClientRect().width>0).map(e=>getComputedStyle(e)).flatMap(c=>[c.animationDuration,c.transitionDuration]).flatMap(v=>v.split(',')).map(v=>parseFloat(v)*(v.trim().endsWith('ms')?1:1000)).filter(v=>v>1)""")
                require(not durations, "reduced-motion shell keeps long animations")
                captures = [evidence.capture(page, f"after-{scheme}-{language}-1100x850")]
                # Genuine reachable catalog states, using the URL contract.
                settle(page, origin + "/games?q=__dashboard_no_match_87__")
                locale(page, language)
                require_filtered_empty(page)
                captures.append(evidence.capture(page, f"after-{scheme}-{language}-empty"))
                settle(page, origin + "/games?players=0&category=__invalid__")
                locale(page, language)
                require(page.locator(".banner--error[role=status]").count() == 2, "invalid query axes have no visible recoverable states")
                captures.append(evidence.capture(page, f"after-{scheme}-{language}-invalid"))
                result.append({"scheme": scheme, "language": language, "contrast_pairs": measured, "captures": captures})
            finally:
                context.close()
    return result


def drawer_keyboard(browser, origin, evidence):
    context = browser.new_context(viewport={"width": 390, "height": 844}, device_scale_factor=1)
    try:
        page = context.new_page()
        settle(page, origin + "/")
        toggle = page.locator("#menu-toggle")
        toggle.focus()
        page.keyboard.press("Enter")
        dialog = page.locator("#shell-menu")
        require(dialog.evaluate("el => el.open && el.matches(':modal')"), "drawer is not a genuine modal dialog")
        require(dialog.evaluate("el => el.contains(document.activeElement)"), "drawer did not take keyboard focus")
        count = dialog.locator("a[href],button:not([disabled]),select,input,[tabindex='0']").count()
        require(count >= 2, "drawer focus selection is empty")
        for _ in range(count + 2):
            page.keyboard.press("Tab")
            require(dialog.evaluate("el => el.contains(document.activeElement)"), "Tab escaped modal drawer")
        for _ in range(count + 2):
            page.keyboard.press("Shift+Tab")
            require(dialog.evaluate("el => el.contains(document.activeElement)"), "Shift+Tab escaped modal drawer")
        capture = evidence.capture(page, "after-mobile-drawer-keyboard")
        page.keyboard.press("Escape")
        require(not dialog.evaluate("el => el.open"), "Escape left drawer open")
        require(toggle.evaluate("el => el === document.activeElement"), "Escape did not restore toggle focus")
        page.keyboard.press("Enter")
        require(dialog.evaluate("el => el.open"), "drawer cannot reopen")
        dialog.locator('a[href="/games"]').first.click()
        page.wait_for_url("**/games")
        require(not dialog.evaluate("el => el.open"), "navigation left modal drawer open")
        # Brand is outside the navigation group. Mouse and Enter must still
        # dismiss the persistent modal rather than strand the new Home behind it.
        toggle.focus()
        page.keyboard.press("Enter")
        dialog.locator(".brand").click()
        page.wait_for_function("location.pathname === '/'")
        require(not dialog.evaluate("el => el.open"), "Brand click left modal drawer open on Home")
        page.locator('.bottom-nav a[href="/games"]').click()
        page.wait_for_url("**/games")
        toggle.focus()
        page.keyboard.press("Enter")
        dialog.locator(".brand").focus()
        page.keyboard.press("Enter")
        page.wait_for_function("location.pathname === '/'")
        require(not dialog.evaluate("el => el.open"), "Brand Enter left modal drawer open on Home")
        toggle.focus()
        page.keyboard.press("Enter")
        page.go_back(wait_until="networkidle")
        page.wait_for_function("location.pathname === '/games'")
        require(not dialog.evaluate("el => el.open"), "Back changed route but left modal drawer open")
        toggle.focus()
        page.keyboard.press("Enter")
        page.go_forward(wait_until="networkidle")
        page.wait_for_function("location.pathname === '/'")
        require(not dialog.evaluate("el => el.open"), "Forward changed route but left modal drawer open")
        return {"focusable_controls": count, "capture": capture, "escape": True, "focus_restore": True,
                "repeated_open": True, "navigation_close": True, "brand_click_and_enter_close": True,
                "back_and_forward_while_modal_close": True}
    finally:
        context.close()


def desktop_skip_link(browser, origin, evidence):
    context = browser.new_context(viewport={"width": 1440, "height": 1000}, device_scale_factor=1)
    try:
        page = context.new_page()
        settle(page, origin + "/")
        unfocused = page.evaluate(SKIP_STATE)
        require_skip_concealed(unfocused)
        # No programmatic focus: the very first real Tab must reach the skip link.
        page.keyboard.press("Tab")
        link = page.locator(".skip-link")
        require(link.evaluate("el => el === document.activeElement"), "first desktop Tab did not reach skip link")
        capture = evidence.capture(page, "after-desktop-focused-skip-link")
        visible = link.evaluate("""el => {const b=el.getBoundingClientRect();const hit=document.elementFromPoint(b.x+b.width/2,b.y+b.height/2);return {on_screen:b.x>=0&&b.y>=0&&b.right<=innerWidth&&b.bottom<=innerHeight,topmost:el===hit||el.contains(hit),rect:{x:b.x,y:b.y,width:b.width,height:b.height},z_index:getComputedStyle(el).zIndex}}""")
        require(visible["on_screen"] and visible["topmost"], "focused skip link is covered by desktop chrome")
        page.keyboard.press("Enter")
        require(page.locator("main#main").evaluate("el => el === document.activeElement"), "skip-link Enter did not focus main")
        return {"unfocused": unfocused, "first_tab": True, "visible_hit_test": visible,
                "enter_main_focus": True, "capture": capture}
    finally:
        context.close()


def route_and_loading(browser, origin, root, evidence):
    context = browser.new_context(viewport={"width": 390, "height": 844}, device_scale_factor=1)
    try:
        page, requests, sizes, errors = traced_page(context, root)
        settle(page, origin + "/")
        require(not errors, "landing threw a browser exception")
        require(not any(is_heavy_resource(url, shell_wasm_paths(root)) for url in requests), "landing preloaded game runtime/atlas/model")
        lazy = {"request_paths": sorted({urlsplit(url).path for url in requests}), "encoded_transfer_bytes": int(sum(sizes)),
                "shell_wasm_paths": sorted(shell_wasm_paths(root)), "game_runtime_atlas_model_requests": 0}
        require(lazy["shell_wasm_paths"], "compiled shell WASM missing")
        settle(page, origin + "/games?players=2")
        search = page.locator("#search")
        search.fill("chess")
        search.blur()
        page.wait_for_function("new URL(location.href).searchParams.get('q') === 'chess'")
        require(page.locator(".card").count() > 0, "real catalog search unexpectedly empty")
        query = urlsplit(page.url).query
        detail = page.locator('.card a[href^="/games/"]').first
        href = detail.get_attribute("href")
        detail.click()
        page.wait_for_url("**" + href)
        require(page.locator("h1").count() == 1, "detail did not render")
        require(page.locator(".online-panel [data-testid=online-create]").count() == 1
                and page.locator(".online-panel [data-testid=online-join]").count() == 1,
                "online-enabled detail create/join controls regressed")
        detail_capture = evidence.capture(page, "after-mobile-real-game-detail")
        setup = page.locator('a[href*="setup=1"]').first
        setup.click()
        page.wait_for_function("new URL(location.href).searchParams.get('setup') === '1'")
        require(page.locator(".setup fieldset").count() > 0, "registry setup did not render")
        setup_capture = evidence.capture(page, "after-mobile-real-game-setup")
        page.go_back(wait_until="networkidle")
        require(page.locator(".setup").count() == 0, "Back left setup substate mounted")
        page.go_back(wait_until="networkidle")
        require(urlsplit(page.url).query == query, "Back lost catalog query")
        require(page.locator("#search").input_value() == "chess", "Back lost search control state")
        page.go_forward(wait_until="networkidle")
        require(urlsplit(page.url).path == href, "Forward lost detail route")
        require(not errors, "route flow threw a browser exception")
        return {"landing": lazy, "catalog_query": query, "detail_path": href, "captures": [detail_capture, setup_capture], "back_and_forward": True,
                "create_join": "NOT_RUN here; separate actual online-match workflow must pass on the same revision"}
    finally:
        context.close()


def mobile_content_slot(browser, origin, evidence):
    context = browser.new_context(viewport={"width": 320, "height": 640}, device_scale_factor=1)
    try:
        page = context.new_page()
        settle(page, origin + "/games")
        page.evaluate("window.scrollTo(0, document.documentElement.scrollHeight)")
        metrics = page.evaluate(MEASURE)
        assert_layout(metrics, 320, small=True)
        require(metrics["cards"][-1]["bottom"] <= metrics["bottom_nav"]["y"] + 1, "fixed nav covers last card at scroll end")
        return {"metrics": metrics, "capture": evidence.capture(page, "after-small-mobile-catalog-bottom-slot")}
    finally:
        context.close()


def catalog_interactions(browser, origin, evidence):
    """Real input/keyboard flows across URL updates, not a static value snapshot."""
    context = browser.new_context(viewport={"width": 390, "height": 844}, device_scale_factor=1)
    try:
        page = context.new_page()
        settle(page, origin + "/games?players=2")
        require(page.locator("#filter-players").input_value() == "2", "initial deep link is not selected")
        search = page.locator("#search")
        search.focus()
        search.press_sequentially("chess ", delay=35)
        page.wait_for_function("new URL(location.href).searchParams.get('q') === 'chess'")
        require(search.input_value() == "chess ", "normalized address ate in-progress trailing search space")
        require(search.evaluate("el => el === document.activeElement"), "query update stole search focus")
        search.press_sequentially("board", delay=35)
        page.wait_for_function("new URL(location.href).searchParams.get('q') === 'chess board'")
        require(search.input_value() == "chess board", "multiword typing did not survive query updates")
        require(search.evaluate("el => el === document.activeElement"), "multiword query stole search focus")
        category = page.locator("#filter-category")
        category.focus()
        category.press("ArrowDown")
        page.wait_for_function("new URL(location.href).searchParams.get('category') === 'abstract'")
        require(category.evaluate("el => el === document.activeElement"), "category result update stole native select focus")
        require(page.evaluate("new URL(location.href).searchParams.get('q')") == "chess board"
                and page.evaluate("new URL(location.href).searchParams.get('players')") == "2",
                "filter update used stale query and lost an existing constraint")
        duration = page.locator("#filter-duration")
        first_budget = duration.locator('option[value]:not([value=""])').first.get_attribute("value")
        require(bool(first_budget), "real duration option selection empty")
        duration.focus()
        duration.press("ArrowDown")
        page.wait_for_function("value => new URL(location.href).searchParams.get('duration') === value", arg=first_budget)
        require(duration.evaluate("el => el === document.activeElement"), "duration result update stole native select focus")
        require(page.evaluate("new URL(location.href).searchParams.get('category')") == "abstract"
                and page.evaluate("new URL(location.href).searchParams.get('q')") == "chess board",
                "second filter update lost latest first filter or search")
        typing_capture = evidence.capture(page, "after-mobile-multiword-native-filter-focus")
        page.locator(".catalog__reset").click()
        page.wait_for_function("location.search === ''")
        require(search.input_value() == "", "reset left stale search draft")
        for axis in ("category", "players", "duration", "complexity", "mode"):
            require(page.locator("#filter-" + axis).input_value() == "", "reset left a stale native selection")
        # These are parser-valid constraints even where current catalog inventory
        # has no matching game or menu item. Do not turn them into invented facts.
        query = "category=abstract&players=8&duration=15&complexity=heavy&mode=bots"
        settle(page, origin + "/games?" + query)
        selected = {"category": "abstract", "players": "8", "duration": "15", "complexity": "heavy", "mode": "bots"}
        for axis, value in selected.items():
            require(page.locator("#filter-" + axis).input_value() == value,
                    "valid deep-link constraint disappeared from native select")
        require(page.locator(".banner--error").count() == 0, "valid absent-inventory constraint mislabeled invalid")
        require_filtered_empty(page)
        return {"multiword_draft_preserved": True, "search_and_select_focus_preserved": True,
                "latest_constraints_combined": True, "reset_controls": True, "valid_deep_link_selections": selected,
                "captures": [typing_capture, evidence.capture(page, "after-mobile-valid-absent-inventory-deep-link")]}
    finally:
        context.close()


def text_scale(playwright, origin, evidence, private):
    """Real browser user font preferences; no CSS-injected fake reflow evidence."""
    result = []
    failures = []
    for scale in (1, 2):
        profile = private / f"font-{scale}"
        default = profile / "Default"
        default.mkdir(parents=True)
        (default / "Preferences").write_text(json.dumps({"webkit": {"webprefs": {
            "default_font_size": 16 * scale, "default_fixed_font_size": 13 * scale,
            "minimum_font_size": 0}}, "profile": {"default_zoom_level": 0}}))
        # Default Playwright headless uses a separate shell embedder. Its lack
        # of Chrome profile preference behavior cannot stand in for 200% text.
        # Official channel='chromium' uses full Chromium's new headless mode.
        context = playwright.chromium.launch_persistent_context(str(profile), headless=True, channel="chromium",
                    viewport={"width": 390, "height": 844}, device_scale_factor=1)
        try:
            page = context.pages[0]
            probe = page.evaluate("parseFloat(getComputedStyle(document.documentElement).fontSize)")
            (evidence.output / f"font-preference-probe-{scale}.json").write_text(json.dumps({
                "browser_channel": "chromium", "requested_default_font_px": 16 * scale,
                "actual_blank_document_root_font_px": probe,
                "established": abs(probe - 16 * scale) <= 0.1}, indent=2) + "\n")
            require_font_preference(probe, 16 * scale)
            for name, width, height in FONT_VIEWPORTS:
                page.set_viewport_size({"width": width, "height": height})
                for language in FONT_LANGUAGES:
                    partition = {"scale": scale, "language": language, "viewport": {"width": width, "height": height}}
                    settle(page, origin + "/")
                    locale(page, language)
                    home = page.evaluate(MEASURE)
                    home_words = page.evaluate(NARROW_USABILITY)
                    home_skip = page.evaluate(SKIP_STATE)
                    home_capture = evidence.capture(page, f"after-{name}-{language}-default-font-{scale * 100}percent", True)
                    try:
                        assert_layout(home, width, small=True)
                        require_skip_concealed(home_skip)
                        assert_narrow_usability(home_words, width, probe, language, home=True)
                    except AcceptanceFailure as error:
                        failures.append({**partition, "route": "/", "reason": str(error)})
                    settle(page, origin + "/games")
                    locale(page, language)
                    library = page.evaluate(MEASURE)
                    library_words = page.evaluate(NARROW_USABILITY)
                    library_skip = page.evaluate(SKIP_STATE)
                    library_capture = evidence.capture(page, f"after-catalog-{name}-{language}-default-font-{scale * 100}percent", True)
                    try:
                        assert_layout(library, width, small=True)
                        require_skip_concealed(library_skip)
                        assert_narrow_usability(library_words, width, probe, language, home=False)
                    except AcceptanceFailure as error:
                        failures.append({**partition, "route": "/games", "reason": str(error)})
                    page.locator("#menu-toggle").click()
                    dialog = page.locator("#shell-menu")
                    drawer_capture = evidence.capture(page, f"after-{name}-{language}-drawer-font-{scale * 100}percent")
                    drawer = dialog.evaluate("el => ({open:el.open,scroll_width:el.scrollWidth,client_width:el.clientWidth,focus_inside:el.contains(document.activeElement)})")
                    if not drawer["open"] or drawer["scroll_width"] > drawer["client_width"] + 1:
                        failures.append({**partition, "route": "drawer", "reason": "scaled modal drawer overflows horizontally"})
                    if not drawer["focus_inside"]:
                        failures.append({**partition, "route": "drawer", "reason": "scaled drawer did not own focus"})
                    page.keyboard.press("Escape")
                    require(not dialog.evaluate("el => el.open"), "Escape did not close scaled drawer")
                    require(page.locator("#menu-toggle").evaluate("el => el === document.activeElement"),
                            "scaled drawer did not restore toggle focus")
                    result.append({**partition, "initial_font_px": probe, "narrow_reflow_required": width / probe <= 16,
                                   "home": home, "library": library, "drawer": drawer,
                                   "unfocused_skip": {"home": home_skip, "library": library_skip},
                                   "narrow_usability": {"home": home_words, "library": library_words},
                                   "captures": [home_capture, library_capture, drawer_capture]})
                    (evidence.output / "font-scaling.json").write_text(json.dumps({"observations": result, "failures": failures,
                        "capture_note": "Home and catalog are full-page rasters; fixed navigation is painted at its initial viewport position. Actual viewport/slot geometry is measured separately."}, indent=2) + "\n")
        finally:
            context.close()
    assert_font_scaling(result)
    require(not failures, "bilingual font scaling has recorded layout failures")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--after-dist", type=Path, required=True)
    parser.add_argument("--before-dist", type=Path, required=True)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--after-revision", required=True)
    parser.add_argument("--before-revision", required=True)
    parser.add_argument("--reference-revision", required=True)
    args = parser.parse_args()
    # An explicit guard prevents this CI route being misused to evade a local denial.
    parser.error("actual browser execution requires authorized disposable GitHub Actions") if not (
        os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("TABULA_DASHBOARD_ACCEPTANCE") == "1") else None
    for root in (args.after_dist, args.before_dist):
        require((root / "index.html").is_file() and shell_wasm_paths(root), "actual compiled shell build missing")
    require((args.reference_root / "standalone.html").is_file(), "byte-preserved original prototype missing")
    verified_reference = verify_reference(args.reference_root)
    evidence = Evidence(args.output)
    (args.output / "provenance.json").write_text(json.dumps({
        "after_revision": args.after_revision, "before_revision": args.before_revision,
        "reference_revision": args.reference_revision,
        "github_run_id": os.environ.get("GITHUB_RUN_ID"),
        "after_tree": subprocess.run(["git", "rev-parse", "HEAD^{tree}"], check=True, capture_output=True, text=True).stdout.strip(),
        "after_dist": file_manifest(args.after_dist), "before_dist": file_manifest(args.before_dist),
        "reference_standalone": file_manifest(args.reference_root),
        "reference_verification": verified_reference,
        "viewports": [{"name": n, "width": w, "height": h, "dpr": 1} for n, w, h in VIEWPORTS],
        "browser_route": "official Playwright channel chromium (full new-headless) in disposable authorized GitHub Actions",
        "tls_verification_disabled": False, "http_mocking": False, "layout_css_injection": False,
    }, indent=2) + "\n")
    evidence.case("before actual emitted shell uses existing raw-WASM loading budget", lambda: emitted_shell_budget(args.before_dist))
    evidence.case("after actual emitted shell uses existing raw-WASM loading budget", lambda: emitted_shell_budget(args.after_dist))
    with tempfile.TemporaryDirectory(prefix="tabula-dashboard-profile-") as temp, \
            static_origin(args.after_dist.resolve(), shell=True) as after, \
            static_origin(args.before_dist.resolve(), shell=True) as before, \
            static_origin(args.reference_root.resolve(), shell=False) as prototype, sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True, channel="chromium")
        try:
            evidence.case("same-viewport actual before/after/prototype pixels and responsive layout", lambda: capture_comparison(browser, evidence, {"before": before, "prototype": prototype, "after": after}))
            evidence.case("four real system schemes, vi/en, reduced motion, contrast, empty and invalid query", lambda: preference_matrix(browser, after, evidence))
            evidence.case("genuine native drawer keyboard trap, Escape, restore focus and repeated route dismissal", lambda: drawer_keyboard(browser, after, evidence))
            evidence.case("real desktop first-Tab skip link is topmost and Enter focuses main", lambda: desktop_skip_link(browser, after, evidence))
            evidence.case("real catalog/detail/setup Back/Forward and lazy landing network bytes", lambda: route_and_loading(browser, after, args.after_dist, evidence))
            evidence.case("multiword typing/native-select focus, latest-query combination and valid deep links", lambda: catalog_interactions(browser, after, evidence))
            evidence.case("320px fixed bottom nav leaves reachable last card slot", lambda: mobile_content_slot(browser, after, evidence))
            evidence.case("genuine isolated Chromium 200-percent default font preference reflows", lambda: text_scale(playwright, after, evidence, Path(temp)))
        finally:
            browser.close()
    evidence.write()
    receipt = json.loads((args.output / "acceptance.json").read_text())
    receipt["status"] = "FAIL" if evidence.errors else "PASS"
    receipt["browser_version"] = subprocess.run([str(playwright.chromium.executable_path), "--version"], capture_output=True, text=True).stdout.strip()
    receipt["screenshots"] = sorted(path.name for path in args.output.glob("*.png"))
    (args.output / "acceptance.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"status": receipt["status"], "claims": len(evidence.records), "failed": evidence.errors,
                      "screenshots": len(receipt["screenshots"])}))
    return 1 if evidence.errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
