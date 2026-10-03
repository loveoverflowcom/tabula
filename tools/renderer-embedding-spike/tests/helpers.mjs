import {readFileSync} from 'node:fs';
export const fixture=JSON.parse(readFileSync(new URL('../fixtures/fixture.json',import.meta.url)));
export const contract=JSON.parse(readFileSync(new URL('../fixtures/render-contract.json',import.meta.url)));
export const frame=fixture.scenarios[0].frames[0];
export const identity={session_id:'test-session',generation:1};
export const envelope=(revision=0)=>({schema_version:1,...identity,revision,frame:structuredClone(frame)});
export class Target {
  constructor(){this.listeners=new Map();this.children=[];this.style={};this.attributes={};this.active=false;}
  addEventListener(name,handler){let set=this.listeners.get(name);if(!set){set=new Set();this.listeners.set(name,set);}set.add(handler);}
  removeEventListener(name,handler){this.listeners.get(name)?.delete(handler);}
  fire(name,props={}){for(const handler of [...(this.listeners.get(name)??[])])handler({preventDefault(){},...props});}
  count(){return [...this.listeners.values()].reduce((n,set)=>n+set.size,0);}
  appendChild(child){this.children.push(child);child.parent=this;}
  remove(){if(this.parent)this.parent.children=this.parent.children.filter(child=>child!==this);}
  setAttribute(k,v){this.attributes[k]=v;}
  getBoundingClientRect(){return {left:10,top:20,width:900,height:720};}
  focus(){this.active=true;this.document.activeElement=this;this.fire('focus');}
  blur(){this.active=false;this.document.activeElement=null;this.fire('blur');}
  setPointerCapture(id){this.capture=id;}
  releasePointerCapture(){this.capture=null;}
}
export function setup(){
  const window=new Target(),document=new Target(),container=new Target();document.visibilityState='visible';const callbacks=new Map();let sequence=0,time=0;
  const environment={window,document,requestAnimationFrame:f=>{const id=++sequence;callbacks.set(id,f);return id;},cancelAnimationFrame:id=>callbacks.delete(id),now:()=>++time};
  const backends=[];
  const makeBackend=()=>{const canvas=new Target();canvas.document=document;let disposed=false,draws=0;const value={canvas,attach:c=>c.appendChild(canvas),resize:v=>{value.viewport={...v};},set_theme:theme=>{value.theme=theme;},draw:()=>{if(disposed)throw Error('draw after disposal');draws++;},dispose:()=>{disposed=true;canvas.remove();},snapshot:()=>({disposed,draw_count:draws,textures:disposed?0:2,sources:disposed?0:2})};backends.push(value);return value;};
  const options={container,identity,contract,assets:fixture.assets,themes:fixture.themes,viewport:{width:900,height:720,dpi:1},environment,backendFactory:async()=>makeBackend()};
  return {options,window,document,container,environment,callbacks,backends,makeBackend,step(){const items=[...callbacks];callbacks.clear();for(const [,f]of items)f(time+=16);}};
}
