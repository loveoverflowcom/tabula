"""Propagate approved Expressive direction into static artifacts only. No product code."""
from pathlib import Path
import json,re,shutil
import build_designs as base
import mobile_boards as mobile
import expressive_components as m3
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'dist';Scene=base.Scene;P=base.P
P.update({'bg':'#FFFBFF','paper':'#F4EDF7','soft':'#E9E0FA'})
old_rect=Scene.rect;old_line=Scene.line

def rect(self,x,y,w,h,fill=None,stroke=None,r=12,sw=1):
 # Deliberate functional outlines (board/focus/control/error) remain. Ornament is removed.
 if stroke==P['line']:stroke=None
 old_rect(self,x,y,w,h,fill,stroke,r,sw)
def line(self,x1,y1,x2,y2,c=None,sw=1,dash=''):
 if c is None or c==P['line']:return
 old_line(self,x1,y1,x2,y2,c,sw,dash)
def card(self,x,y,w,h,title=None,kicker=None):
 self.rect(x,y,w,h,P['paper'],r=28)
 if kicker:self.label(x+20,y+28,kicker)
 if title:self.text(x+20,y+52 if kicker else y+34,title,18,weight=600)
def button(self,x,y,w,t,kind='primary',focus=False):
 bg,fg=(P['primary'],'white') if kind=='primary' else ('#CFBCFF','#32007E') if kind=='soft' else ('#EAE3ED',P['muted']) if kind=='secondary' else ('#E1DDE4','#696370')
 h=48
 if focus:self.rect(x-5,y-5,w+10,h+10,'none',P['primary'],r=29,sw=3)
 self.rect(x,y,w,h,bg,r=24);self.text(x+w/2,y+30,t,14,fg,600,'middle')
def field(self,x,y,w,label,value,helper=None,error=False):
 self.label(x,y,label);self.rect(x,y+10,w,48,'#EAE3ED',r=16);self.text(x+14,y+40,value,14,P['muted'])
 if error:old_line(self,x+10,y+58,x+w-10,y+58,P['danger'],2)
 if helper:self.text(x,y+76,helper,12,P['danger'] if error else P['muted'])
def row(self,x,y,w,title,sub=None,right=None,status=None):
 self.rect(x,y,w,64,'#EAE3ED',r=12);self.text(x+14,y+25,title,14,weight=600)
 if sub:self.text(x+14,y+47,sub,12,P['muted'])
 if right:self.text(x+w-14,y+32,right,14,P['muted'],anchor='end',mono=True)
 if status:self.pill(x+w-144,y+17,status,'planned',w=130)
def toggle(self,x,y,on=True):
 self.rect(x,y,44,28,P['primary'] if on else '#79747E',r=14);self.circle(x+31 if on else x+13,y+14,9,'white')
def pill(self,x,y,t,kind='neutral',w=None):
 colors={'neutral':('#EAE3ED',P['muted']),'current':('#DEE8DB','#244830'),'planned':('#F3E6D2','#62472E'),'selected':('#CFBCFF','#32007E'),'danger':('#FBE9E7',P['danger'])};bg,fg=colors[kind];w=w or len(t)*7+24;self.rect(x,y,w,28,bg,r=14);self.text(x+w/2,y+19,t,11,fg,600,'middle');return w
Scene.rect=rect;Scene.line=line;Scene.card=card;Scene.button=button;Scene.field=field;Scene.row=row;Scene.toggle=toggle;Scene.pill=pill

def shell(s,title,section='Thư viện',future=False):
 m3.shell(s,title,section)
 if future:m3.badge(s,1204,85,'Dự kiến / có gate','#F3E6D2','#62472E',204)
base.shell=shell
# Original state/layout contracts retained. The approved library and showcase are bespoke direction boards.
for key,pack,title,fn in base.SCREENS:
 if key=='01-library':s=m3.library()
 elif key=='22-design-system':s=m3.foundation()
 else:s=Scene();fn(s)
 d=OUT/pack/'screens';d.mkdir(parents=True,exist_ok=True);(d/(key+'.svg')).write_text(s.svg(title))
for d in sorted(OUT.glob('[0-9][0-9]-*')):
 (d/'screens/mobile-states.svg').write_text(mobile.mobile(d.name).svg(d.name+'mobile M3 Expressive'))
 # The navigator is genuinely offline-friendly, with a classic inline copy of its editableMJS source.
 ht=(d/'index.html').read_text();js=(d/'viewer.mjs').read_text();ht=ht.replace('<script type="module" src="viewer.mjs"></script>','<script>'+js+'</script>');(d/'index.html').write_text(ht)
 (d/'prototype.css').write_text('''*{box-sizing:border-box}body{margin:0;background:#FFFBFF;color:#1E1C21;font:16px system-ui,sans-serif}header{display:flex;gap:12px;align-items:center;flex-wrap:wrap;padding:12px 20px;background:#F4EDF7}h1{font-size:18px;margin:0}select,a{min-height:48px;padding:12px 16px;border:0;border-radius:24px;color:#32007E;background:#EAE3ED}a:focus-visible,select:focus-visible{outline:3px solid #5634BE;outline-offset:2px}main{overflow:auto;padding:16px}img{display:block;max-width:none}p{margin:0;font-size:13px;color:#4B4650}small{font-size:12px}@media(max-width:700px){header{align-items:stretch}select{max-width:100%}main{padding:0}}''')
 f=d/'implementation-notes.md';notes=f.read_text();notes=notes.split('\n## Approved Material 3 Expressive visual revision')[0];notes+='''\n## Approved Material 3 Expressive visual revision\n\nGroup by tonal containment, contrast, hierarchy and purposeful shape. No ornamental card outlines or shadows. Retain functional board lines, focus rings, off-control affordances and validation strokes. The prominent action is larger or higher contrast; routine actions are quieter. Keep label-first scientific scanning and no marketing hero.\n\nReference components: filled/tonal round or square buttons, standard/connected button groups, contained lists, filled fields and context floating toolbars. Connected groups use 2dp gaps, 8dp inner corners and fully rounded outer corners; mobile selection options remain visible. Shape and spring interaction states are specified, not rendered as working animations in these static SVGs.\n\nThe exact Material component anatomy is adapted to existing Leptos DOM and Macroquad RenderList contracts. This is not a claim that Tabula already ships a native Material 3 Expressive kit. Canonical Corners has four radii and MotionTokens already includes spring families/reduced-motion; use those capabilities first. Component-specific radius/color mappings that differ from existing tokens are bounded proposals for tokens.toml after a real consumer is selected, not a parallel theme source. The purple tonal reference fill is an explicit color-extension proposal; generated four-scheme semantics remain authoritative.\n''';f.write_text(notes)
 for png in d.glob('contact*.png'):png.unlink()
shared=OUT/'01-foundation/shared'
(shared/'material-component-map.md').write_text('''# Material 3 Expressive → Tabula design adapters\n\nSources inspected01October2026. These are component references, not an installed native UI kit. No new dependency or production implementation is delivered.\n\n- Buttons: round/square, selected and pressed shape feedback, five recommended size classes. Leptos native button semantics and RenderList hit/focus/Intent activation must remain aligned. Preview uses48–64dp actions and canonical44dpminimum.\n- Connected button group:2dpgaps,8dpinnercorners,roundoutercorners. Map to native radio/toggle semantics inDOM and typed focus nodes / four-radius Corners oncanvas. No third-party React kit migration.\n- Floating toolbar: context-related controls grouped in a standard or vibrant tonal surface. Use labels or explicit legend/tooltips; icon-only actions have accessible names. Rotate-left/right are distinct from undo/redo; deselect distinct from exit.\n- Contained lists: positional first/middle/last/single corners, short aligned rows with supporting text and trailing controls. Keep actual off-control affordances and44dphit wrappers.\n- Loading indicator: only short indeterminate work; use real progress indicator for measurable loading and cancellable long operations. Reduced motion retains informative status.\n- Split button: only one main action with a related menu. Do not use it for unrelated commands or hide mutually exclusive modes.\n\n## Official references\n\n- [Google research](https://design.google/library/expressive-material-design-google-research)\n- [Buttons](https://github.com/material-components/material-components-android/blob/master/docs/components/CommonButton.md)\n- [Button groups](https://github.com/material-components/material-components-android/blob/master/docs/components/ButtonGroup.md)\n- [Floating toolbar](https://github.com/material-components/material-components-android/blob/master/docs/components/FloatingToolbar.md)\n- [Lists](https://github.com/material-components/material-components-android/blob/master/docs/components/List.md)\n- [Loading indicator](https://github.com/material-components/material-components-android/blob/master/docs/components/LoadingIndicator.md)\n- [Split button](https://github.com/material-components/material-components-android/blob/master/docs/components/SplitButton.md)\n\nRepository token source remains tokens.toml. Any missing role or component metric needs a written consumer justification and generator/schema parity. Preserve all four schemes, focus3dpandminimum44dp. Mock tones/art do not silently become product semantics.\n''')
for name in ['expressive_components.py','m3_revision.py']:
 shutil.copy(ROOT/'source'/name,OUT/'source'/name)
print('Propagated approvedExpressive direction:23desktop and7mobile sources; HTML/CSS updated')
