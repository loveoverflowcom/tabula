"""Capture the existing real Werewolf WASM UI using ordinary player controls."""
from __future__ import annotations

from datetime import datetime, timezone
from functools import partial
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from importlib.metadata import version
import io
import json
from pathlib import Path
import re
import subprocess
import tempfile
import threading
import time
import unicodedata

from PIL import Image, ImageChops, ImageStat
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
DIST = ROOT / "target/tabula-web-werewolf"
OUT = ROOT / "verification/werewolf-runtime-artifacts"
BUILD = "cargo build --locked -p tabula-game-client --no-default-features --features web-werewolf --bin tabula-werewolf-client --target wasm32-unknown-unknown --profile wasm-release"


def utc() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def normalized(text: str) -> str:
    return "".join(c for c in unicodedata.normalize("NFD", text.lower().replace("đ", "d"))
                   if unicodedata.category(c) != "Mn")


class QuietServer(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


class Driver:
    def __init__(self, page, provenance):
        self.page = page
        self.provenance = provenance
        self.current_seat = None
        self.table_page = 0

    def write(self):
        (OUT / "screenshots-provenance.json").write_text(
            json.dumps(self.provenance, ensure_ascii=False, indent=2) + "\n")

    def action(self, name, x, y, delay=0.16):
        self.page.mouse.click(x, y, delay=80)
        self.provenance["actions"].append({"utc": utc(), "action": name,
                                          "pointer_css_pixels": [round(x, 2), round(y, 2)]})
        self.page.wait_for_timeout(delay * 1000)
        self.write()

    def geometry(self):
        size = self.page.viewport_size
        w, h = size["width"], size["height"]
        compact = w < 760 or h < 620
        m = min(16, w * .03)
        cy = 178 if compact else 128
        ch = max(1, h - cy - 116)
        cw = min(w - 2*m - 20, max(50, ch-52)/1.5, 320) if compact else min(max(60,ch-62)/1.5,320,(w-2*m)*.36)
        cw = max(20, cw)
        cx = (w-cw)/2 if compact else m+24
        tx = m if compact else cx+cw+36
        tw = w-2*m if compact else w-m-tx
        columns = 5 if tw >= 500 else 4
        cellw = max(44, (tw-8*(columns-1))/columns)
        return dict(w=w,h=h,m=m,compact=compact,cy=cy,cw=cw,cx=cx,
                    tx=tx,tw=tw,columns=columns,cellw=cellw)

    def nav(self, index, name):
        g = self.geometry()
        width = min(180,max(44,(g["w"]-2*g["m"]-16)/3))
        self.action(name,g["m"]+(width+8)*index+width/2,102)

    def seat(self, seat):
        self.nav(2,"Select public outsider perspective before disposable test seat")
        self.current_seat = None
        for index in range(seat+1):
            self.nav(1,f"Select disposable test seat {index+1}; previous private view concealed")
        self.current_seat = seat
        self.table_page = 0

    def public(self):
        self.nav(2,"Select public outsider perspective; private card concealed")
        self.current_seat = None
        if self.table_page:
            g = self.geometry()
            page_size = 8 if g["compact"] else 10
            visible = min(page_size,12-self.table_page*page_size)
            y=g["cy"]+34+((visible+g["columns"]-1)//g["columns"])*56+8+22
            self.action("Return public roster to the first real page",g["tx"]+g["tw"]/2,y)
            self.table_page = 0

    def reveal(self):
        g = self.geometry()
        if g["compact"]:
            self.action("Select own-card tab",g["w"]*.25,152)
        self.action("Deliberately reveal only the selected disposable seat's own card",
                    g["w"]/2 if g["compact"] else g["cx"]+g["cw"]/2,
                    g["cy"]+1.5*g["cw"]+30)

    def table(self):
        g = self.geometry()
        if g["compact"]:
            self.action("Select table tab while retaining the authorized own-seat reveal",g["w"]*.75,152)

    def target(self, seat):
        self.table()
        g = self.geometry()
        page_size = 8 if g["compact"] else 10
        wanted_page = seat // page_size
        if self.table_page != wanted_page:
            visible = min(page_size,12-self.table_page*page_size)
            y=g["cy"]+34+((visible+g["columns"]-1)//g["columns"])*56+8+22
            self.action("Select the next real roster page",g["tx"]+g["tw"]/2,y)
            self.table_page = wanted_page
        index = seat % page_size
        self.action(f"Select disposable target seat {seat+1}; no command sent yet",
                    g["tx"]+(g["cellw"]+8)*(index%g["columns"])+g["cellw"]/2,
                    g["cy"]+34+56*(index//g["columns"])+24)

    def action_y(self):
        g = self.geometry()
        page_size = 8 if g["compact"] else 10
        visible = min(page_size,12-self.table_page*page_size)
        # Both twelve-seat layouts retain their real pagination control.
        return g["cy"]+34+((visible+g["columns"]-1)//g["columns"])*56+8+52

    def submit(self, witch=False):
        g = self.geometry()
        self.action("Submit the selected real rules command via UI",
                    g["tx"]+(g["tw"]-8)/4,self.action_y()+22+(52 if witch else 0))

    def poison(self):
        g = self.geometry()
        self.action("Select the available poison potion via UI",
                    g["tx"]+(g["tw"]-8)/2+8+(g["tw"]-8)/4,self.action_y()+22)

    def advance(self, delay=.16):
        g = self.geometry()
        width = min(260,max(44,(g["w"]-2*g["m"]-8)/2))
        self.action("Advance actual logical deadline through the runtime's Next phase control",
                    g["m"]+width/2,g["h"]-78,delay)

    def ocr(self, region=None, flipped=False, psm=6):
        png = self.page.locator("#glcanvas").screenshot(timeout=30000)
        im = Image.open(io.BytesIO(png)).convert("RGB")
        if region:
            im = im.crop(region)
        if flipped:
            im = im.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
        # This is assessment-only OCR of actual pixels, never a generated UI.
        with tempfile.TemporaryDirectory(prefix="werewolf-ocr-") as folder:
            p = Path(folder)/"frame.png"
            im.resize((im.width*2,im.height*2)).save(p)
            text = subprocess.check_output(["tesseract",str(p),"stdout","-l","vie+eng","--psm",str(psm)],
                                            stderr=subprocess.DEVNULL,text=True)
        return normalized(text)

    def role(self):
        g = self.geometry()
        # The existing renderer vertically reflects clipped surfaces. OCR only
        # transforms temporary assessment bytes, never the captured evidence.
        text = self.ocr((int(g["cx"]),int(g["cy"]),int(g["cx"]+g["cw"]),
                         int(g["cy"]+g["cw"]*1.5)),flipped=True)
        for role, spellings in [("witch",["phu thuy"]),("wolf",["ma soi"]),
                                ("seer",["tien tri"]),("doctor",["bac si"]),
                                ("hunter",["tho san"]),("villager",["dan lang"])]:
            if any(s in text for s in spellings):
                return role
        # The first executed run exposed inverted clipped text. Identify only
        # the deliberately displayed own-seat portrait from public static art,
        # never from canonical state or exported WASM memory. Keep PNGs original.
        png=self.page.locator("#glcanvas").screenshot(timeout=30000)
        im=Image.open(io.BytesIO(png)).convert("RGB")
        portrait=im.crop((int(g["cx"]+8),int(g["cy"]+g["cw"]*.5+8),
                          int(g["cx"]+g["cw"]-8),int(g["cy"]+g["cw"]*1.5-8))).resize((64,64))
        scores=[]
        for role,asset in [("villager","villager"),("wolf","werewolf"),("seer","seer"),
                           ("doctor","doctor"),("hunter","hunter"),("witch","witch")]:
            ref=Image.open(ROOT/f"games/werewolf/assets/{asset}@1x.png").convert("RGB").resize((64,64))
            variants=[ref,ref.transpose(Image.Transpose.FLIP_TOP_BOTTOM),ref.rotate(180)]
            score=min(sum(ImageStat.Stat(ImageChops.difference(portrait,v)).rms)/3 for v in variants)
            scores.append((score,role))
        scores.sort()
        if scores[0][0]<35 and scores[1][0]-scores[0][0]>8:
            self.provenance.setdefault("pixel_assessment",[]).append(
                {"kind":"own-seat portrait match against public static source art",
                 "match_score":round(scores[0][0],3),"next_match_gap":round(scores[1][0]-scores[0][0],3),
                 "role_assignment":"not recorded; used only for ordinary fixture UI flow"})
            self.write()
            return scores[0][1]
        (OUT/"99-diagnostic-role-unreadable.png").write_bytes(png)
        self.provenance["diagnostic"]={"file":"99-diagnostic-role-unreadable.png",
             "condition":"Actual own-seat reveal could not be read; original frame, not a passing capture",
             "sha256":hashlib.sha256(png).hexdigest(),"size_bytes":len(png),
             "scores_only":[round(s,3) for s,_r in scores]}
        self.write()
        raise RuntimeError("Own-card role was not readable from actual pixels")

    def phase(self):
        g = self.geometry()
        # OCR the complete visible public phase header; long labels may extend
        # past a half-width crop. No DOM state or internal WASM exports are read.
        text=self.ocr((0,0,g["w"],52),psm=11)
        for phase, words in [("ended",["ket thuc"]),("dawn",["binh minh"]),
                             ("day",["thao luan"]),("vote",["bo phieu"]),
                             ("dusk",["hoang hon"]),("night",["ban dem"])]:
            if any(word in text for word in words):
                return phase
        png=self.page.screenshot(path=str(OUT/"99-diagnostic-phase-unreadable.png"))
        self.provenance["public_header_diagnostic"]={"ocr_normalized":text,
              "file":"99-diagnostic-phase-unreadable.png","sha256":hashlib.sha256(png).hexdigest(),
              "size_bytes":len(png),"condition":"Unreadable public phase header; not passing evidence"}
        self.write()
        raise RuntimeError("Phase header was not readable from actual pixels")

    def wait_phase(self, wanted, timeout=10):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            current=self.phase()
            if current==wanted:
                return
            self.page.wait_for_timeout(100)
        raise RuntimeError("Expected actual elapsed-time phase did not appear")

    def capture(self, filename, label, expected_phase=None):
        if expected_phase and self.phase() != expected_phase:
            raise RuntimeError("Actual rendered phase differs from screenshot condition")
        if not self.page.locator("#loader").is_hidden() or not self.page.locator("#runtime-error").is_hidden():
            raise RuntimeError("Actual runtime is loading or failed")
        if self.page.locator("#privacy-shield").is_visible():
            raise RuntimeError("Actual private surface remains shielded")
        path=OUT/filename
        png=self.page.screenshot(path=str(path),timeout=30000)
        image=Image.open(io.BytesIO(png)).convert("RGB")
        colors=image.resize((160,120)).getcolors(19201)
        if image.width < 320 or image.height < 500 or len(colors or []) < 32:
            raise RuntimeError("Actual screenshot is blank or invalid")
        self.provenance["captures"].append({
            "file": filename,"label":label,"captured_at_utc":utc(),
            "perspective":"public outsider" if self.current_seat is None else f"disposable simulator seat {self.current_seat+1}",
            "phase_condition":expected_phase,"actions_completed":len(self.provenance["actions"]),
            "browser_version":self.page.context.browser.version,
            "viewport_css_pixels":self.page.viewport_size,
            "device_pixel_ratio":self.page.evaluate("window.devicePixelRatio"),
            "png_pixels":{"width":image.width,"height":image.height},
            "size_bytes":len(png),"sha256":hashlib.sha256(png).hexdigest(),
            "pixel_inspection":"captured; separate human/model pixel inspection pending",
            "redaction":"none; entirely disposable local simulator; no account or credential UI",
        })
        self.write()
        print(f"Captured actual runtime: {filename}",flush=True)


def main():
    if git("status","--porcelain","--untracked-files=all"):
        raise RuntimeError("Build checkout is not clean before evidence outputs")
    provenance={"version":1,"capture_kind":"actual_browser_rendered_png",
                "scope":"Existing isolated-seat local Werewolf simulator, not online multiplayer",
                "test_personas":"Anonymous disposable Người N runtime seats; no real users/accounts",
                "source_commit":git("rev-parse","HEAD"),"source_tree":git("rev-parse","HEAD^{tree}"),
                "build_checkout":"clean before artifacts","build_command":BUILD,
                "stage_command":"cargo xtask stage-wasm-game --game werewolf",
                "playwright":version("playwright"),"rustc":subprocess.check_output(["rustc","--version"],text=True).strip(),
                "workflow_run_id":__import__("os").environ.get("GITHUB_RUN_ID"),
                "files":[{"path":p.relative_to(DIST).as_posix(),"size_bytes":p.stat().st_size,"sha256":sha(p)}
                         for p in sorted(DIST.rglob("*")) if p.is_file()],
                "actions":[],"captures":[],"NOT_RUN":["real online Werewolf multiplayer or accounts","voice or native audio","physical mobile touch","native application and CMP Android/iOS WebView embedding","persisted resume/replay or production rollout"]}
    if not any(p["path"].endswith(".wasm") for p in provenance["files"]):
        raise RuntimeError("Actual staged game WASM is missing")
    OUT.mkdir(parents=True,exist_ok=True)
    server=ThreadingHTTPServer(("127.0.0.1",0),partial(QuietServer,directory=str(DIST)))
    threading.Thread(target=server.serve_forever,daemon=True).start()
    base=f"http://127.0.0.1:{server.server_port}"
    result={"status":"FAIL","scope":provenance["scope"],"started_at_utc":utc()}
    try:
        with sync_playwright() as p:
            browser=p.chromium.launch(headless=True,args=["--use-gl=angle","--use-angle=swiftshader","--enable-unsafe-swiftshader"])
            context=browser.new_context(viewport={"width":1200,"height":880},device_scale_factor=1)
            page=context.new_page()
            errors=[]
            page.on("pageerror",lambda _err:errors.append("JavaScript page error"))
            page.goto(base+"/index.html",wait_until="networkidle")
            page.locator("#simulator-seats").select_option("12")
            page.locator("#simulator-form button[type='submit']").click()
            page.locator("#loader").wait_for(state="hidden",timeout=120000)
            page.locator("#glcanvas").focus()
            page.wait_for_timeout(500)
            d=Driver(page,provenance)
            d.write()
            d.seat(0)
            d.capture("00-own-role-concealed.png","Own role remains concealed until deliberate reveal","night")
            # Discover roles only from each deliberate own-seat reveal. No state exports.
            selected={}
            for seat in range(12):
                d.seat(seat)
                d.reveal()
                role=d.role()
                selected.setdefault(role,seat)
            if not {"wolf","witch","villager"}.issubset(selected):
                raise RuntimeError("Requested fixture roles were not actually rendered")
            # Wolf targets one valid non-wolf disposable villager; UI validates command.
            victim=selected["villager"]
            d.seat(selected["wolf"])
            d.reveal()
            d.target(victim)
            d.capture("01-wolf-night-target.png","Own Wolf card and selected real night target; not yet submitted","night")
            d.submit()
            d.reveal()
            g=d.geometry()
            acknowledged=d.ocr((int(g["tx"]),g["cy"],g["w"],g["h"]-110),flipped=True)
            if "da gui" not in acknowledged:
                png=page.screenshot(path=str(OUT/"99-diagnostic-command-ack.png"))
                raise RuntimeError("Wolf real command acknowledgement was not rendered")
            # Witch selects poison, then a target distinct from the wolf's target.
            poison_target=selected["wolf"]
            d.seat(selected["witch"])
            d.reveal()
            d.poison()
            d.target(poison_target)
            d.capture("02-witch-potion-target.png","Own Witch card, remaining potion inventory and selected poison target","night")
            d.submit(witch=True)
            d.public()
            d.advance(.05)
            d.capture("03-dawn-public-deaths.png","Actual referee's dawn: public death disclosure","dawn")
            # Dawn advances naturally after two seconds. A screenshot/OCR can
            # cross that deadline; another click could incorrectly skip Day.
            d.wait_phase("day")
            d.capture("04-day-discussion-public-log.png","Actual Day phase with public death log; discussion is local only","day")
            d.advance()
            d.seat(selected["witch"])
            # Use a living role discovered through normal own-card UI.
            vote_target=selected.get("seer",selected.get("doctor",selected["witch"]))
            d.target(vote_target)
            d.capture("05-vote-selection.png","Living test seat selects a ballot in the actual Vote phase","vote")
            d.submit()
            d.public()
            d.capture("06-public-vote-receipt.png","Public Vote phase after the submitted ballot","vote")
            # Preserve a real narrow-mode card/table run in a fresh independent document.
            narrow_context=browser.new_context(viewport={"width":390,"height":844},device_scale_factor=1)
            narrow=narrow_context.new_page()
            narrow.goto(base+"/play.html?game=werewolf&mode=simulator&seats=12&theme=dark&motion=system&locale=vi",wait_until="networkidle")
            narrow.locator("#loader").wait_for(state="hidden",timeout=120000)
            narrow.locator("#glcanvas").focus()
            narrow.wait_for_timeout(500)
            nd=Driver(narrow,provenance)
            nd.seat(selected["witch"])
            nd.reveal()
            nd.capture("07-narrow-own-card.png","390×844 CSS viewport: deliberate own Witch card reveal in fresh local simulator","night")
            nd.table()
            nd.capture("08-narrow-table-potions.png","390×844 CSS viewport: real table tab, pagination and potion controls","night")
            narrow_context.close()
            page.bring_to_front()
            if page.locator("#privacy-shield").is_visible():
                page.locator("#resume-private").click()
                page.wait_for_timeout(250)
            d.public()
            # Ordinary Next phase presses eventually reach the bounded real terminal state.
            for _ in range(55):
                if d.phase()=="ended":
                    break
                d.advance()
            if d.phase()!="ended":
                raise RuntimeError("Actual referee did not reach bounded terminal state")
            d.capture("09-terminal-public-result.png","Actual terminal result reached via runtime logical phase controls","ended")
            if errors:
                raise RuntimeError("Actual page reported JavaScript errors")
            result.update(status="PASS",png_count=len(provenance["captures"]),
                          browser_version=browser.version,
                          claims=["real existing WASM compiled and executed in Chromium","ordinary pointer flow reaches role reveal, wolf selection, witch potion, actual dawn/day/vote and terminal","390×844 browser viewport card/table captured; no physical/mobile embedding claim"])
            browser.close()
    except Exception as err:
        result["failure_type"]=type(err).__name__
        # Exceptions are fixture-only. Do not publish raw URLs, stacks, OCR or canonical data.
        result["failure_reason"]=str(err)[:300]
        print("Actual runtime capture failed:",result["failure_reason"],flush=True)
        raise
    finally:
        result["finished_at_utc"]=utc()
        (OUT/"browser-result.json").write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n")
        server.shutdown()


if __name__=="__main__":
    main()
