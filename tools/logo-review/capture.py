"""Source-separated real logo UI capture; no image generation or page mocks."""
from datetime import datetime,timezone
from importlib.metadata import version
from pathlib import Path
from urllib.parse import urlsplit
import argparse,hashlib,importlib.util,json,os,subprocess,tempfile
from PIL import Image
from playwright.sync_api import sync_playwright
SCHEMES=[('light','light','no-preference'),('dark','dark','no-preference'),('hc-light','light','more'),('hc-dark','dark','more')]
MASK='input[type="password"],[data-secret],[data-private],[name*="csrf" i],[name*="token" i]'
def utc():return datetime.now(timezone.utc).isoformat()
def git(root,*args):return subprocess.check_output(['git',*args],cwd=root,text=True).strip()
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def inventory(root):return [{'path':x.relative_to(root).as_posix(),'bytes':x.stat().st_size,'sha256':sha(x)} for x in sorted(root.rglob('*')) if x.is_file()]
def brand(page,selector):
    loc=page.locator(selector)
    if not loc.count():return {'canonical_lockup_count':0,'selector':selector}
    return {'canonical_lockup_count':loc.count(),'selector':selector,'measurement':loc.first.evaluate('''el=>{const r=el.getBoundingClientRect(),p=el.closest('a,[role=img]')?.getBoundingClientRect(),d=el.closest('dialog')?.getBoundingClientRect();return {rect:{x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom},link:p?{x:p.x,y:p.y,width:p.width,height:p.height,right:p.right,bottom:p.bottom}:null,dialog:d?{x:d.x,y:d.y,width:d.width,height:d.height,right:d.right,bottom:d.bottom}:null,viewBox:el.getAttribute('viewBox'),fills:[...el.querySelectorAll('path,g')].slice(0,3).map(x=>getComputedStyle(x).fill)}}''')}
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--source-root',type=Path,required=True);ap.add_argument('--output',type=Path,required=True);ap.add_argument('--snapshot',choices=['before','candidate'],required=True);ap.add_argument('--expected-sha',required=True);a=ap.parse_args()
    assert os.environ.get('GITHUB_ACTIONS')=='true' and os.environ.get('TABULA_LOGO_QA')=='1','authorized isolated CI only'
    root=a.source_root.resolve();out=a.output.resolve();out.mkdir(parents=True,exist_ok=True)
    assert git(root,'rev-parse','HEAD')==a.expected_sha,'unexpected actual source checkout'
    assert not git(root,'status','--porcelain','--untracked-files=all'),'actual built checkout not clean'
    spec=importlib.util.spec_from_file_location('source_owned_helpers',root/'tools/dashboard-acceptance/run.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    data={'snapshot':a.snapshot,'source_commit':a.expected_sha,'source_tree':git(root,'rev-parse','HEAD^{tree}'),'harness_commit':os.environ['GITHUB_SHA'],'run_id':os.environ['GITHUB_RUN_ID'],'started_at_utc':utc(),'playwright':version('playwright'),'fixtures':'public anonymous shell and disposable untimed local Chess; no actual accounts or game credentials','scope':'source-separated actual Leptos and compiled Macroquad/WASM, desktop browser viewports only; native/mobile-device/CMP pixels NOT_RUN','builds':{'shell':inventory(root/'apps/web/dist'),'chess':inventory(root/'target/tabula-web-game')},'captures':[],'checks':[],'loader_timing':'real compiled WASM download temporarily throttled by disposable browser CDP160KiB/s/100ms to observe genuine loader, then restored; no intercepted/replaced replies or UI state injected'}
    def save():(out/'logo-provenance.json').write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def check(name,ok,details):data['checks'].append({'name':name,'status':'PASS' if ok else 'FAIL','details':details});save()
    def capture(page,name,label,metrics):
        f=out/(name+'.png');b=page.screenshot(path=str(f),mask=[page.locator(MASK)]);im=Image.open(f);im.verify()
        with Image.open(f) as im:dims=list(im.size)
        data['captures'].append({'file':f.name,'label':label,'captured_at_utc':utc(),'route_path':urlsplit(page.url).path,'viewport_css_pixels':page.viewport_size,'png_pixels':dims,'dpr':page.evaluate('devicePixelRatio'),'scheme':page.locator('html').get_attribute('data-theme'),'locale':page.locator('html').get_attribute('lang'),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'original_png_unchanged':True,'metrics':metrics});save()
    try:
        with helper.static_origin(root/'apps/web/dist',shell=True) as shell,helper.static_origin(root/'target/tabula-web-game',shell=False) as game,sync_playwright() as p:
            browser=p.chromium.launch(headless=True,args=['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader']);data['browser_version']=browser.version;save()
            for theme,color,contrast in SCHEMES:
                for width,height,kind in [(1440,1000,'desktop'),(390,844,'mobile')]:
                    context=browser.new_context(viewport={'width':width,'height':height},device_scale_factor=1)
                    try:
                        page=context.new_page();page.emulate_media(color_scheme=color,contrast=contrast,reduced_motion='reduce');helper.settle(page,shell+'/');helper.locale(page,'vi')
                        sel='.sidebar > .brand .brand-lockup' if width==1440 else '.topbar__brand .brand-lockup';metrics=brand(page,sel)
                        actual=page.locator('html').get_attribute('data-theme');check(f'{kind}-{theme} actual scheme',actual==theme,{'requested':theme,'actual':actual})
                        if a.snapshot=='candidate':check(f'{kind}-{theme} canonical SVG present',metrics['canonical_lockup_count']==1,metrics)
                        capture(page,f'{a.snapshot}-{kind}-{theme}-home','actual shell brand in source-owned UI',metrics)
                        if kind=='mobile':
                            page.locator('.mobile-menu-button').click();page.locator('dialog.shell-menu').wait_for(state='visible');metrics=brand(page,'dialog.shell-menu .brand-lockup');capture(page,f'{a.snapshot}-mobile-{theme}-drawer','actual opened mobile menu brand',metrics)
                            if a.snapshot=='candidate':check(f'mobile-{theme} drawer SVG present',metrics['canonical_lockup_count']==1,metrics)
                            page.keyboard.press('Escape')
                    finally:context.close()
                context=browser.new_context(viewport={'width':1200,'height':880},device_scale_factor=1)
                try:
                    page=context.new_page();page.emulate_media(color_scheme=color,contrast=contrast,reduced_motion='reduce');page.goto(game+'/index.html',wait_until='networkidle')
                    page.locator('#clock').select_option('untimed');page.locator('details.preferences summary').click();page.locator('#theme').select_option(theme);page.locator('#motion').select_option('reduced')
                    metrics=brand(page,'header.shared-header .brand .brand-lockup');check(f'standalone-{theme} actual scheme',page.locator('html').get_attribute('data-theme')==theme,{'actual':page.locator('html').get_attribute('data-theme')})
                    if a.snapshot=='candidate':check(f'standalone-{theme} header SVG present',metrics['canonical_lockup_count']==1,metrics)
                    capture(page,f'{a.snapshot}-standalone-{theme}-header','actual staged local Chess setup header',metrics)
                    session=context.new_cdp_session(page);session.send('Network.enable');session.send('Network.emulateNetworkConditions',{'offline':False,'latency':100,'downloadThroughput':163840,'uploadThroughput':163840})
                    page.locator('#setup button[type=submit]').click();page.wait_for_function('theme=>document.documentElement.dataset.theme===theme',arg=theme,timeout=30000);page.locator('#loader').wait_for(state='visible',timeout=30000);check(f'standalone-{theme} loader actual scheme',page.locator('html').get_attribute('data-theme')==theme,{'requested':theme,'actual':page.locator('html').get_attribute('data-theme')});metrics=brand(page,'#loader .runtime-brand .brand-lockup')
                    capture(page,f'{a.snapshot}-standalone-{theme}-loader','actual bootloader during real throttled WASM transfer',metrics)
                    if a.snapshot=='candidate':check(f'standalone-{theme} genuine loader SVG present',metrics['canonical_lockup_count']==1,metrics)
                    session.send('Network.emulateNetworkConditions',{'offline':False,'latency':0,'downloadThroughput':-1,'uploadThroughput':-1});page.locator('#loader').wait_for(state='hidden',timeout=120000)
                    check(f'standalone-{theme} actual WASM completed',page.locator('#runtime-error').is_hidden() and page.locator('#glcanvas').is_visible(),{'game_wasm':next(x for x in data['builds']['chess'] if x['path'].endswith('.wasm') and 'resources/' not in x['path'])})
                finally:context.close()
            browser.close()
            with tempfile.TemporaryDirectory(prefix='logo-font32-') as temp:
                profile=Path(temp);(profile/'Default').mkdir();(profile/'Default'/'Preferences').write_text(json.dumps({'webkit':{'webprefs':{'default_font_size':32,'default_fixed_font_size':26,'minimum_font_size':0}},'profile':{'default_zoom_level':0}}))
                context=p.chromium.launch_persistent_context(str(profile),channel='chromium',headless=True,viewport={'width':320,'height':640},device_scale_factor=1,color_scheme='light',reduced_motion='reduce')
                try:
                    page=context.pages[0];probe=page.evaluate('parseFloat(getComputedStyle(document.documentElement).fontSize)');helper.require_font_preference(probe,32);helper.settle(page,shell+'/');helper.locale(page,'en');page.locator('.mobile-menu-button').click();page.locator('dialog.shell-menu').wait_for(state='visible')
                    metrics=brand(page,'dialog.shell-menu .brand-lockup');drawer=page.locator('dialog.shell-menu').evaluate('el=>({scroll_width:el.scrollWidth,client_width:el.clientWidth,focus_inside:el.contains(document.activeElement),open:el.open})');metrics.update(drawer=drawer,initial_font_px=probe)
                    capture(page,f'{a.snapshot}-320-en-font200-drawer','actual320 EN200% drawer with independently established32px profile',metrics)
                    if a.snapshot=='candidate':
                        r=metrics.get('measurement',{}).get('rect',{});d=metrics.get('measurement',{}).get('dialog',{})
                        check('candidate320 EN200 drawer canonical SVG stays inside dialog',metrics['canonical_lockup_count']==1 and r.get('right',10000)<=d.get('right',0)+.5 and r.get('x',-1)>=d.get('x',0)-.5 and drawer['scroll_width']<=drawer['client_width']+1,metrics)
                    page.keyboard.press('Escape')
                finally:context.close()
        data['status']='PASS' if all(x['status']=='PASS' for x in data['checks']) else 'FAIL'
    except Exception as error:data.update(status='BLOCKED',error_type=type(error).__name__);save();raise
    finally:data['finished_at_utc']=utc();save()
    assert data['status']=='PASS','bounded logo checks have failures; originals preserved'
if __name__=='__main__':main()
