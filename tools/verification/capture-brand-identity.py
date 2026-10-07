"""Reproduce the bounded public-shell brand matrix; serve the actual build on port 8191 first.
Requires Playwright and a local sandboxed Chrome. No page styles or response bodies are replaced.
"""
from pathlib import Path
from argparse import ArgumentParser
from playwright.sync_api import sync_playwright
import json,hashlib,subprocess
from datetime import datetime, timezone
root=Path(__file__).resolve().parents[2]
parser=ArgumentParser(description=__doc__)
parser.add_argument('--output',type=Path,default=root/'verification/brand-identity-artifacts')
parser.add_argument('--chrome',default='/opt/google/chrome/chrome')
args=parser.parse_args()
out=args.output
out.mkdir(parents=True,exist_ok=True)
cases=[]
with sync_playwright() as p:
 browser=p.chromium.launch(executable_path=args.chrome,headless=True,chromium_sandbox=True)
 version=browser.version
 for scheme,color,contrast in [('light','light','no-preference'),('dark','dark','no-preference'),('hc-light','light','more'),('hc-dark','dark','more')]:
  for width in [320,390,768,1440]:
   for locale in ['vi','en']:
    context=browser.new_context(viewport={'width':width,'height':1000},color_scheme=color,contrast=contrast,reduced_motion='reduce')
    page=context.new_page();errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
    page.goto('http://127.0.0.1:8191/');page.locator('#locale').select_option(locale)
    page.wait_for_function('(args)=>document.documentElement.dataset.theme===args[0] && document.documentElement.lang===args[1]',arg=[scheme,locale])
    drawer=width<=390
    if drawer:page.locator('#menu-toggle').click()
    brand=page.locator('#shell-menu .brand' if drawer else '.brand:visible').first
    brand.wait_for(state='visible');svg=brand.locator('svg');box=svg.bounding_box();parent=brand.bounding_box()
    assert box and parent and box['width']>0 and box['x']>=0 and box['x']+box['width']<=width+1
    assert abs(box['width']/box['height']-705.276/256)<0.015
    assert brand.get_attribute('aria-label')=='Tabula' and svg.get_attribute('aria-hidden')=='true'
    assert svg.evaluate('(el)=>getComputedStyle(el).animationName')=='none'
    for _ in range(30):
     if brand.evaluate('(el)=>document.activeElement===el'):break
     page.keyboard.press('Tab')
    assert brand.evaluate('(el)=>document.activeElement===el')
    row={'scheme':scheme,'width':width,'locale':locale,'drawer':drawer,'logo_box':box,'reduced_motion':True,'single_accessible_name':True,'keyboard_focus':True,'page_errors':errors}
    if locale=='vi' and width==1440 or locale=='en' and width==320 and scheme=='light':
     name=f'web-{scheme}-{width}-{locale}.png';page.screenshot(path=str(out/name));row['screenshot']=name
    if drawer:
     page.keyboard.press('Escape');assert page.locator('#menu-toggle').evaluate('(el)=>document.activeElement===el')
    assert not errors
    cases.append(row);context.close()
 browser.close()
wasm=list((root/'apps/web/dist').glob('*.wasm'));assert len(wasm)==1 and wasm[0].stat().st_size<900000
result={'captured_at':datetime.now(timezone.utc).isoformat(),'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'browser':version,'runtime':'actual Leptos online-feature release WASM, local loopback public shell','cases':cases,'wasm_bytes':wasm[0].stat().st_size,'wasm_sha256':hashlib.sha256(wasm[0].read_bytes()).hexdigest(),'screenshots':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.glob('*.png')},'limits':['No installed launcher/device or native window pixel execution','No authenticated online acceptance','No 200% font case in this run; previous pinned screenshots cover that case']}
(out/'runtime.json').write_text(json.dumps(result,indent=2)+'\n');print('PASS',len(cases),'actual runtime brand cases; wasm',result['wasm_bytes'])
