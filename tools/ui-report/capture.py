"""Fresh real UI report, intentionally bounded to public shell/local games."""
from __future__ import annotations
from datetime import datetime,timezone
import hashlib
import importlib.util
from importlib.metadata import version
import io
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile
from urllib.parse import urlsplit

from PIL import Image,ImageChops,ImageStat
from playwright.sync_api import sync_playwright

ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'verification/current-ui-report-artifacts'
MASK='input,textarea,[data-secret],[data-private],[name*="csrf" i],[name*="token" i]'


def utc():
    return datetime.now(timezone.utc).isoformat()


def git(*args):
    return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inventory(path):
    return [{'path':p.relative_to(path).as_posix(),'bytes':p.stat().st_size,'sha256':digest(p)}
            for p in sorted(path.rglob('*')) if p.is_file()]


def require(condition,label):
    if not condition:
        raise RuntimeError(label)


class Evidence:
    def __init__(self,metadata):
        self.metadata=metadata
        self.write()

    def write(self):
        (OUT/'capture-provenance.json').write_text(json.dumps(self.metadata,ensure_ascii=False,indent=2)+'\n')

    def capture(self,page,name,label,*,full=False,scope='public actual Leptos shell'):
        path=OUT/(name+'.png')
        png=page.screenshot(path=str(path),full_page=full,mask=[page.locator(MASK)],timeout=30000)
        im=Image.open(io.BytesIO(png)).convert('RGB')
        require(im.width>=320 and im.height>=300 and len(im.resize((160,120)).getcolors(19201) or [])>32,
                'Screenshot pixels are blank or invalid')
        self.metadata['captures'].append({'file':path.name,'label':label,'scope':scope,'captured_at_utc':utc(),
            'route_path':urlsplit(page.url).path,'viewport_css_pixels':page.viewport_size,
            'device_pixel_ratio':page.evaluate('window.devicePixelRatio'),'png_pixels':[im.width,im.height],
            'bytes':len(png),'sha256':hashlib.sha256(png).hexdigest(),'browser_version':page.context.browser.version,
            'original_png_unchanged':True,'pixel_inspection':'pending separate post-download review',
            'privacy':'public shell or anonymous local fixtures; account inputs and secret-marked nodes masked'})
        self.write()
        print('Actual PNG',path.name,flush=True)
        return png

    def observation(self,name,status,details):
        self.metadata['observations'].append({'name':name,'status':status,'details':details})
        self.write()


def chess_board(width,height):
    # Exact existing public BoardLayout geometry, only for pointer locations.
    margin=min(width*.035,height*.025,24)
    gap=min(height*.008,8)
    title=min(height*.06,40)
    player=min(max(height*.07,44 if height>=450 else 24),56) if height>=160 else height*.08
    rail=(width>=760 and height>=420) or (width>=600 and height<420)
    status=0 if rail else min(height*.1,64)
    railw=min(width*.28,360) if rail else 0
    gamew=max(width-margin*2-railw-(gap*2 if rail else 0),0)
    rail_toolbar=rail and height<450
    columns=max(math.floor((gamew+4)/76),1)
    controls=math.ceil(6/columns)*48-4 if height>=200 and width>=280 and not rail_toolbar else 0
    coordinate=min(gamew*.03,12 if height<420 else 16)
    remaining=max(height-margin*2-title-player*2-controls-status-gap*6-coordinate*2,0)
    side=min(max(gamew-coordinate*2,0),remaining,680)
    left=margin+(gamew-side)*.5
    top=margin+title+gap+player+gap+coordinate
    return {'x':left,'y':top,'side':side,'square':side/8}


def main():
    require(os.environ.get('GITHUB_ACTIONS')=='true' and os.environ.get('TABULA_UI_REPORT')=='1',
            'This real browser report runs only in the authorized disposable GitHub CI environment')
    require(not git('status','--porcelain','--untracked-files=all'),'Actual build checkout is not clean')
    spec=importlib.util.spec_from_file_location('maintained_dashboard_helpers',ROOT/'tools/dashboard-acceptance/run.py')
    dashboard=importlib.util.module_from_spec(spec);spec.loader.exec_module(dashboard)
    OUT.mkdir(parents=True,exist_ok=True)
    metadata={'version':1,'source_commit':git('rev-parse','HEAD'),'source_tree':git('rev-parse','HEAD^{tree}'),
              'build_checkout':'clean after builds before capture outputs','started_at_utc':utc(),
              'workflow_run_id':os.environ.get('GITHUB_RUN_ID'),'playwright':version('playwright'),
              'browser_kind':'actual official Chromium, headless, ANGLE/SwiftShader; no physical-device claim',
              'build_commands':['TABULA_PLAY_BASE=/play trunk build --release --cargo-profile wasm-release --features online',
                  'cargo build --locked -p tabula-game-client --no-default-features --features web --bin tabula-game-client --target wasm32-unknown-unknown --profile wasm-release',
                  'cargo xtask stage-wasm-game',
                  'cargo build --locked -p tabula-game-client --no-default-features --features web-werewolf --bin tabula-werewolf-client --target wasm32-unknown-unknown --profile wasm-release',
                  'cargo xtask stage-wasm-game --game werewolf'],
              'builds':{'dashboard':inventory(ROOT/'apps/web/dist'),'chess':inventory(ROOT/'target/tabula-web-game'),
                        'werewolf':inventory(ROOT/'target/tabula-web-werewolf')},
              'captures':[],'observations':[],'fixtures':'No real accounts; public missing-adapter shell and disposable local game seats',
              'NOT_RUN':['physical mobile/touch','CMP/native GameHost','real login/accounts/avatar provider','Werewolf online/chat/voice',
                         'full fresh online Chess fault acceptance','full accessibility and frame-pacing acceptance']}
    evidence=Evidence(metadata)
    report={'status':'FAIL','started_at_utc':utc(),'scope':'actual screenshot capture; visual/budget gates are separate'}
    try:
        with (dashboard.static_origin(ROOT/'apps/web/dist',shell=True) as shell,
              dashboard.static_origin(ROOT/'target/tabula-web-game',shell=False) as chess,
              dashboard.static_origin(ROOT/'target/tabula-web-werewolf',shell=False) as wolf,
              sync_playwright() as p):
            args=['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader']
            browser=p.chromium.launch(headless=True,args=args)
            metadata['browser_version']=browser.version;evidence.write()
            for viewport,theme,locale in [({'width':1440,'height':1000},'light','vi'),
                                          ({'width':390,'height':844},'light','vi'),
                                          ({'width':1100,'height':850},'dark','en')]:
                context=browser.new_context(viewport=viewport,device_scale_factor=1,color_scheme=theme,reduced_motion='reduce')
                page=context.new_page();dashboard.settle(page,shell+'/');dashboard.locale(page,locale)
                prefix=f'dashboard-{viewport["width"]}-{theme}-{locale}'
                evidence.capture(page,prefix+'-home','Current real dashboard Home')
                measured=page.evaluate(dashboard.MEASURE)
                evidence.observation(prefix+'-home-geometry','MEASURED',measured)
                if viewport['width']==1440:
                    page.keyboard.press('Tab');skip=page.locator('.skip-link')
                    first=skip.evaluate('el=>el===document.activeElement')
                    evidence.capture(page,'dashboard-desktop-first-tab','Actual first Tab skip link')
                    page.keyboard.press('Enter');main_focused=page.locator('main#main').evaluate('el=>el===document.activeElement')
                    evidence.observation('desktop skip-link Enter focuses main','PASS' if first and main_focused else 'FAIL',
                                         {'first_tab_focuses_skip':first,'enter_focuses_main':main_focused})
                dashboard.settle(page,shell+'/games');dashboard.locale(page,locale)
                evidence.capture(page,prefix+'-library','Current real game Library',full=viewport['width']==390)
                evidence.observation(prefix+'-library-geometry','MEASURED',page.evaluate(dashboard.MEASURE))
                context.close()
            # Genuine Chromium browser font preference, not injected CSS scaling.
            with tempfile.TemporaryDirectory(prefix='ui-report-font-') as temp:
                profile=Path(temp);default=profile/'Default';default.mkdir()
                (default/'Preferences').write_text(json.dumps({'webkit':{'webprefs':{'default_font_size':32,
                    'default_fixed_font_size':26,'minimum_font_size':0}},'profile':{'default_zoom_level':0}}))
                context=p.chromium.launch_persistent_context(str(profile),channel='chromium',headless=True,args=args,
                          viewport={'width':390,'height':844},device_scale_factor=1,reduced_motion='reduce')
                page=context.pages[0];probe=page.evaluate('parseFloat(getComputedStyle(document.documentElement).fontSize)')
                dashboard.require_font_preference(probe,32)
                evidence.observation('genuine 200 percent browser font preference','PASS',{'requested_root_font_px':32,'observed_root_font_px':probe})
                for language in ['vi','en']:
                    dashboard.settle(page,shell+'/');dashboard.locale(page,language)
                    evidence.capture(page,f'dashboard-390-{language}-font200-home','Current Home with genuine 200 percent Chromium font preference',full=True)
                    evidence.observation('font200 Home '+language,'MEASURED',page.evaluate(dashboard.MEASURE))
                context.close()
            for viewport in [{'width':1200,'height':880},{'width':390,'height':844}]:
                context=browser.new_context(viewport=viewport,device_scale_factor=1,color_scheme='light',reduced_motion='reduce')
                page=context.new_page();page.goto(chess+'/index.html',wait_until='networkidle')
                page.locator('#clock').select_option('untimed')
                page.locator('details.preferences summary').click();page.locator('#motion').select_option('reduced')
                page.locator('#theme').select_option('light');page.locator('#locale').select_option('vi')
                if viewport['width']==1200:
                    evidence.capture(page,'chess-desktop-setup','Actual local two-human Chess setup',full=True,scope='local anonymous Chess fixture')
                page.locator('#setup button[type=submit]').click();page.locator('#loader').wait_for(state='hidden',timeout=120000)
                require(page.locator('#runtime-error').is_hidden(),'Actual Chess runtime failed to load')
                page.locator('#glcanvas').focus();page.wait_for_timeout(650)
                initial=evidence.capture(page,f'chess-{viewport["width"]}-initial','Actual local Chess initial board',scope='local anonymous Chess fixture')
                bounds=page.locator('#glcanvas').bounding_box();g=chess_board(bounds['width'],bounds['height'])
                evidence.observation(f'Chess {viewport["width"]} actual canvas/pointer geometry','MEASURED',{'canvas':bounds,'board':g})
                if viewport['width']==1200:
                    for square in ['e2','e4']:
                        file=ord(square[0])-ord('a');rank=int(square[1])-1
                        x=bounds['x']+g['x']+(file+.5)*g['square'];y=bounds['y']+g['y']+(7-rank+.5)*g['square']
                        page.mouse.click(x,y,delay=80);page.wait_for_timeout(250)
                    page.wait_for_timeout(500)
                    moved=evidence.capture(page,'chess-desktop-after-e2-e4','Actual board after ordinary e2-to-e4 pointer input',scope='local anonymous Chess fixture')
                    before=Image.open(io.BytesIO(initial)).convert('RGB');after=Image.open(io.BytesIO(moved)).convert('RGB')
                    changed=sum(ImageStat.Stat(ImageChops.difference(before,after)).mean)
                    evidence.observation('Chess visible response to e2-e4','PASS' if changed>1 else 'FAIL',
                                         {'sum_mean_pixel_difference':changed,'semantic_move_verdict':'requires independent original-pixel review; no canonical state read'})
                context.close()
            browser.close()
            # Reuse the maintained current Werewolf actual-input driver. Only
            # platform browser launch changes in a temporary copy, no source edit.
            original=ROOT/'games/werewolf/tests/verify-redesign.mjs'
            source=original.read_text();needle="const browser = await chromium.launch({ executablePath: process.env.TABULA_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });"
            require(source.count(needle)==1,'Current maintained Werewolf driver launch changed; re-inspect before running')
            replacement="const browser = await chromium.launch({ executablePath: process.env.TABULA_CHROME, headless: true, args: ['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });"
            with tempfile.TemporaryDirectory(prefix='ui-report-wolf-driver-') as temp:
                adapted=source.replace(needle,replacement)
                # The current maintained game moved its options controls into
                # Layout::options. Adapt only public pointer locations; private
                # drawer geometry and actual game/input/assertion paths stay real.
                original_return="return { compact, landscape, table, reveal, dock, card, footer, dialog: rect(margin, Math.max(height * .16, 8), width - margin * 2, Math.max(height * .68, 244)) };"
                options_return="""const optionsWidth = Math.min(width - 32, 520), condensed = height < 420;
  const optionsHeight = condensed ? 276 : 408;
  const optionsDialog = rect((width - optionsWidth) / 2, (height - optionsHeight) / 2, optionsWidth, optionsHeight);
  return { compact, landscape, table, reveal, dock, card, footer, optionsDialog,
    tools_y: optionsDialog.y + (condensed ? 128 : 240), row_step: condensed ? 48 : 52,
    dialog: rect(margin, Math.max(height * .16, 8), width - margin * 2, Math.max(height * .68, 244)) };"""
                require(adapted.count(original_return)==1,'Current public geometry return changed; re-inspect')
                adapted=adapted.replace(original_return,options_return)
                pointer_replacements={
                    'Math.max((g.dialog.width - 32) / 2, 44)':'Math.max((g.optionsDialog.width - 32) / 2, 44)',
                    'rect(g.dialog.x + 12 + w + 8, g.dialog.y + 70 + 52, w, 44)':'rect(g.optionsDialog.x + 12 + w + 8, g.tools_y + g.row_step, w, 44)',
                    'rect(g.dialog.x + 12 + w + 8, g.dialog.y + 70, w, 44)':'rect(g.optionsDialog.x + 12 + w + 8, g.tools_y, w, 44)',
                    'rect(g.dialog.x + 12, g.dialog.y + 70 + 52, w, 44)':'rect(g.optionsDialog.x + 12, g.tools_y + g.row_step, w, 44)',
                    'rect(g.dialog.x + 12 + w + 8, g.dialog.y + 122, w, 44)':'rect(g.optionsDialog.x + 12 + w + 8, g.tools_y + g.row_step, w, 44)',
                    'Math.max((deadGeometry.dialog.width - 32) / 2, 44)':'Math.max((deadGeometry.optionsDialog.width - 32) / 2, 44)',
                    'rect(deadGeometry.dialog.x + 12, deadGeometry.dialog.y + 122, deadDialogWidth, 44)':'rect(deadGeometry.optionsDialog.x + 12, deadGeometry.tools_y + deadGeometry.row_step, deadDialogWidth, 44)',
                }
                for old,new in pointer_replacements.items():
                    require(old in adapted,'Expected maintained public input location changed; re-inspect')
                    adapted=adapted.replace(old,new)
                driver=Path(temp)/'verify-redesign.mjs';driver.write_text(adapted)
                metadata['werewolf_driver']={'original_source':original.relative_to(ROOT).as_posix(),'original_sha256':digest(original),
                    'executed_copy_sha256':digest(driver),'only_change':'official CI browser launch and public pointer locations matching current Layout::options; no game state or source edits',
                    'cases':'1200x880-dpr1,390x844-dpr1; theme-light and public-phase-captures extras'};evidence.write()
                env=os.environ.copy();env.update(TABULA_CASES='1200x880-dpr1,390x844-dpr1',TABULA_EXTRA_CASES='public-phase-captures,theme-light',TABULA_KEEP_ALL='1')
                result=subprocess.run(['node',str(driver),wolf,str(OUT/'werewolf')],env=env,text=True,capture_output=True,timeout=300)
                (OUT/'werewolf-driver-output.log').write_text(result.stdout+'\n'+result.stderr)
                evidence.observation('maintained current Werewolf driver','PASS' if result.returncode==0 else 'FAIL',{'exit_code':result.returncode,
                    'receipts':'werewolf/runs/summary.json','original_screenshots':'werewolf/screenshots'})
                print('Werewolf driver exit',result.returncode,flush=True)
            report.update(status='CAPTURED',public_shell_and_chess_pngs=len(metadata['captures']),
                          werewolf_driver_exit=result.returncode,visual_quality='post-download pixel inspection pending; budget remains separately enforced')
    finally:
        report['finished_at_utc']=utc();(OUT/'capture-result.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
        metadata['finished_at_utc']=utc();evidence.write()


if __name__=='__main__':
    main()
