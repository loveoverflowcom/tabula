"""Narrow real 320px EN/200% skip-link review, authorized CI only."""
from datetime import datetime, timezone
from importlib.metadata import version
import hashlib, importlib.util, json, os
from pathlib import Path
import subprocess, tempfile
from urllib.parse import urlsplit
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'verification/skip-link-review-artifacts'
def utc(): return datetime.now(timezone.utc).isoformat()
def git(*args): return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    assert os.environ.get('GITHUB_ACTIONS')=='true' and os.environ.get('TABULA_SKIP_REVIEW')=='1', 'authorized disposable CI only'
    assert not git('status','--porcelain','--untracked-files=all'), 'actual build checkout not clean'
    OUT.mkdir(parents=True,exist_ok=True)
    spec=importlib.util.spec_from_file_location('merged_dashboard_helpers',ROOT/'tools/dashboard-acceptance/run.py')
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    data={'source_commit':git('rev-parse','HEAD'),'source_tree':git('rev-parse','HEAD^{tree}'),'run_id':os.environ['GITHUB_RUN_ID'],
          'started_at_utc':utc(),'scope':'actual public Leptos,320x640 EN/light,DPR1, independent Chromium32px user font; no physical-device claim',
          'build_command':'TABULA_PLAY_BASE=/play trunk build --release --cargo-profile wasm-release --features online',
          'builds':[{'path':x.relative_to(ROOT/'apps/web/dist').as_posix(),'bytes':x.stat().st_size,'sha256':sha(x)} for x in sorted((ROOT/'apps/web/dist').rglob('*')) if x.is_file()],
          'measurement_timing':'geometry/focus sampled after the original screenshot frame, matching maintained desktop helper','fixtures':'anonymous public shell; no real account/provider; no page/HTTP/layout mocks','playwright':version('playwright'),'captures':[],'checks':[]}
    def save(): (OUT/'skip-link-provenance.json').write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def capture(page,name,label):
        f=OUT/(name+'.png');b=page.screenshot(path=str(f),mask=[page.locator('input[type="password"],[data-secret],[data-private],[name*="token" i],[name*="csrf" i]')])
        data['captures'].append({'file':f.name,'label':label,'captured_at_utc':utc(),'route_path':urlsplit(page.url).path,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'viewport_css_pixels':page.viewport_size,'dpr':page.evaluate('devicePixelRatio'),'original_png_unchanged':True});save()
    def measure(page): return page.locator('.skip-link').evaluate('''el=>{const r=el.getBoundingClientRect(),s=getComputedStyle(el);return {rect:{x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right},active:el===document.activeElement,focus_visible:el.matches(':focus-visible'),transform:s.transform,top:s.top,scroll_y:scrollY,viewport:{width:innerWidth,height:innerHeight},lang:document.documentElement.lang,scheme:document.documentElement.dataset.theme,root_font_px:parseFloat(getComputedStyle(document.documentElement).fontSize)}}''')
    try:
        with helper.static_origin(ROOT/'apps/web/dist',shell=True) as origin,sync_playwright() as p,tempfile.TemporaryDirectory(prefix='skip-font-') as temp:
            profile=Path(temp);(profile/'Default').mkdir();(profile/'Default'/'Preferences').write_text(json.dumps({'webkit':{'webprefs':{'default_font_size':32,'default_fixed_font_size':26,'minimum_font_size':0}},'profile':{'default_zoom_level':0}}))
            context=p.chromium.launch_persistent_context(str(profile),channel='chromium',headless=True,viewport={'width':320,'height':640},device_scale_factor=1,color_scheme='light',reduced_motion='reduce')
            try:
                page=context.pages[0];probe=page.evaluate('parseFloat(getComputedStyle(document.documentElement).fontSize)');helper.require_font_preference(probe,32)
                data.update(browser_version=context.browser.version,initial_font_probe_px=probe);save();helper.settle(page,origin+'/');helper.locale(page,'en')
                capture(page,'skip-unfocused-320-en-font200','actual before intentional keyboard focus');before=measure(page)
                data['checks'].append({'name':'unfocused skip-link fully above viewport','status':'PASS' if not before['active'] and not before['focus_visible'] and before['rect']['bottom']<=0.5 else 'FAIL','measurement':before})
                page.keyboard.press('Tab');capture(page,'skip-focused-320-en-font200','actual first Tab focused skip-link');focused=measure(page)
                data['checks'].append({'name':'first Tab fully reveals skip-link','status':'PASS' if focused['active'] and focused['focus_visible'] and focused['rect']['y']>=0 and focused['rect']['bottom']<=640 and focused['rect']['right']<=320.5 else 'FAIL','measurement':focused})
                page.keyboard.press('Enter');capture(page,'skip-main-focused-320-en-font200','actual Enter transfers native fragment focus to main');main_focus=page.locator('main#main').evaluate('el=>el===document.activeElement');after=measure(page)
                data['checks'].append({'name':'Enter focuses main and reconceals skip-link','status':'PASS' if main_focus and not after['active'] and after['rect']['bottom']<=0.5 else 'FAIL','main_focused':main_focus,'measurement':after})
                data['status']='PASS' if all(x['status']=='PASS' for x in data['checks']) else 'FAIL';save()
            finally: context.close()
    except Exception as error:
        data.update(status='BLOCKED',error=type(error).__name__);save();raise
    finally: data['finished_at_utc']=utc();save()
    assert data['status']=='PASS','narrow actual skip-link regression failed; original evidence preserved'
if __name__=='__main__': main()
