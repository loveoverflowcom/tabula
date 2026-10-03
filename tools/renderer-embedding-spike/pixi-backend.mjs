// Isolated direct-canvas PixiJS backend; consumes only the Rust-lowered RenderList.
import { BoundaryError, requireThat } from './boundary.mjs';

export async function verifiedBytes(file, baseUrl, { fetch: fetcher = globalThis.fetch, crypto = globalThis.crypto, signal } = {}) {
  requireThat(typeof file.path==='string'&&file.path.length<=512&&/^(?:\.\/)?[A-Za-z0-9_@.-]+(?:\/[A-Za-z0-9_@.-]+)*$/.test(file.path)&&file.path.replace(/^\.\//,'').split('/').every(part=>part!=='.'&&part!=='..'),'asset_path',file.name);
  const url = new URL(file.path, baseUrl); const base = new URL(baseUrl);
  requireThat(url.origin === base.origin && /^https?:$/.test(url.protocol), 'asset_origin', file.name);
  requireThat(/^[a-f0-9]{64}$/.test(file.sha256) && Number.isSafeInteger(file.bytes) && file.bytes > 0 && file.bytes <= 16_777_216, 'asset_descriptor', file.name);
  const response = await fetcher(url, { signal, credentials: 'same-origin', cache: 'default' });
  requireThat(response.ok, 'asset_fetch', file.name);
  const length = response.headers?.get('content-length'); if (length) requireThat(Number(length) === file.bytes, 'asset_size', file.name);
  let bytes;
  if(response.body?.getReader) {
    const reader=response.body.getReader();const chunks=[];let total=0;
    try {while(true){const item=await reader.read();if(item.done)break;total+=item.value.byteLength;requireThat(total<=file.bytes,'asset_size',file.name);chunks.push(item.value);}}
    catch(error){await reader.cancel();throw error;}
    requireThat(total===file.bytes,'asset_size',file.name);const joined=new Uint8Array(total);let offset=0;for(const chunk of chunks){joined.set(chunk,offset);offset+=chunk.byteLength;}bytes=joined.buffer;
  } else {bytes=await response.arrayBuffer();requireThat(bytes.byteLength===file.bytes,'asset_size',file.name);}
  const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), v => v.toString(16).padStart(2, '0')).join('');
  requireThat(hash === file.sha256, 'asset_integrity', file.name); return bytes;
}
const rgb = c => (c[0] << 16) | (c[1] << 8) | c[2];
export function inspectPng(bytes, file = {}) {
  const data=new Uint8Array(bytes); requireThat(data.length>=24&&[137,80,78,71,13,10,26,10].every((v,i)=>data[i]===v)&&[73,72,68,82].every((v,i)=>data[i+12]===v),'asset_png_header',file.name??'png');
  const view=new DataView(bytes);const width=view.getUint32(16),height=view.getUint32(20);
  requireThat(width>0&&height>0&&width<=8192&&height<=8192&&width*height<=16_777_216,'asset_pixel_limit',file.name??'png');
  if(file.width!==undefined)requireThat(width===file.width&&height===file.height,'asset_dimensions',file.name);
  return {width,height,pixels:width*height};
}
const multiply = (p, c) => [p[0]*c[0]+p[2]*c[1],p[1]*c[0]+p[3]*c[1],p[0]*c[2]+p[2]*c[3],p[1]*c[2]+p[3]*c[3],p[0]*c[4]+p[2]*c[5]+p[4],p[1]*c[4]+p[3]*c[5]+p[5]];
const intersect = (a,b) => { const x=Math.max(a[0],b[0]),y=Math.max(a[1],b[1]);return [x,y,Math.max(0,Math.min(a[0]+a[2],b[0]+b[2])-x),Math.max(0,Math.min(a[1]+a[3],b[1]+b[3])-y)]; };

export async function createPixiBackend({ viewport, assets, themes, theme = 'light', baseUrl = globalThis.location?.href, font = null, signal, pixi = null }) {
  // Import only after mount so init timing includes module initialization in cold runs.
  const P = pixi ?? await import('./node_modules/pixi.js/dist/pixi.mjs');
  const app = new P.Application();
  let initialized = false; let disposed = false; let mounted = false;
  const sources = new Map(); const textures = new Map(); const bitmaps = []; const gradients = []; let ownedFont = null;
  function clear() { app.stage?.removeChildren().forEach(child => child.destroy({ children: true, texture: false, textureSource: false, context: true })); while(gradients.length) gradients.pop().destroy?.(); }
  function dispose() {
    if (disposed) return; disposed = true;
    if (initialized) { clear(); app.destroy({ removeView: true }, { children: true, texture: false, textureSource: false, context: true }); }
    else { app.renderer?.destroy({removeView:true});app.stage?.destroy({children:true}); }
    for (const texture of textures.values()) texture.destroy(false); textures.clear();
    for (const source of sources.values()) source.destroy(); sources.clear();
    for (const bitmap of bitmaps) bitmap.close?.(); bitmaps.length = 0;
    if (ownedFont) globalThis.document?.fonts?.delete(ownedFont); ownedFont = null;
  }
  try {
    if (signal?.aborted) throw new BoundaryError('cancelled', 'init');
    await app.init({ width:viewport.width,height:viewport.height,resolution:viewport.dpi,autoDensity:true,autoStart:false,sharedTicker:false,preference:'webgl',antialias:true,background:rgb(themes[theme].background),backgroundAlpha:themes[theme].background[3]/255,accessibilityOptions:{activateOnTab:false},eventMode:'none' });
    initialized = true; app.stop();
    // No Pixi DOM interaction plugin: the mount controller alone owns input listeners.
    app.renderer.events?.setTargetElement(null);
    let decodedPixels=0;
    for (const file of assets.files) {
      const bytes = await verifiedBytes(file, baseUrl, { signal });
      const dimensions=inspectPng(bytes,file);decodedPixels+=dimensions.pixels;requireThat(decodedPixels<=16_777_216,'asset_pack_pixel_limit','assets');
      if (signal?.aborted) throw new BoundaryError('cancelled', 'assets');
      const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' })); bitmaps.push(bitmap);
      requireThat(bitmap.width === dimensions.width && bitmap.height === dimensions.height, 'asset_decode_dimensions', file.name);
      const source = new P.ImageSource({ resource:bitmap,scaleMode:'linear',autoGarbageCollect:false }); sources.set(file.name,source);
    }
    for (const resource of assets.resources) {
      for (const variant of resource.variants) {
        const source = sources.get(variant.file); requireThat(source, 'missing_asset_file', resource.id);
        const r = variant.region ?? [0,0,source.pixelWidth,source.pixelHeight];
        requireThat(r.length === 4 && r.every(Number.isFinite) && r[0] >= 0 && r[1] >= 0 && r[2] > 0 && r[3] > 0 && r[0]+r[2] <= source.pixelWidth && r[1]+r[3] <= source.pixelHeight, 'asset_region', resource.id);
        textures.set(`${resource.id}|${variant.file}`,new P.Texture({ source,frame:new P.Rectangle(...r) }));
      }
    }
    if (font) {
      const bytes = await verifiedBytes({ ...font, name:'baseline-font', path:font.url ?? font.path },baseUrl,{signal});
      const face = new FontFace('TabulaSpikeProggy',bytes); await face.load(); if (signal?.aborted) throw new BoundaryError('cancelled', 'font');
      document.fonts.add(face); ownedFont = face;
    }
    if (signal?.aborted) throw new BoundaryError('cancelled', 'init');
  } catch (error) { dispose(); throw error; }
  let currentViewport = { ...viewport }; let drawCount = 0; let latestCommands = 0;
  const matrix = values => new P.Matrix(...values);
  function fillStyle(paint) {
    if (paint.kind === 'solid') return { color:rgb(paint.color),alpha:paint.color[3]/255 };
    const gradient = new P.FillGradient({ type:'linear',start:{x:paint.from[0],y:paint.from[1]},end:{x:paint.to[0],y:paint.to[1]},textureSpace:'global',colorStops:paint.stops.map(s => ({offset:s.offset,color:s.color.map(v=>v/255)})) }); gradients.push(gradient); return {fill:gradient};
  }
  function shape(g, command) {
    if (command.kind === 'path') { g.moveTo(...command.points[0]); for (const point of command.points.slice(1)) g.lineTo(...point); if(command.closed) g.closePath(); return; }
    const [x,y,w,h]=command.rect; const r=command.radii.map(v=>Math.min(v,w/2,h/2));
    g.moveTo(x+r[0],y).lineTo(x+w-r[1],y).quadraticCurveTo(x+w,y,x+w,y+r[1]).lineTo(x+w,y+h-r[2]).quadraticCurveTo(x+w,y+h,x+w-r[2],y+h).lineTo(x+r[3],y+h).quadraticCurveTo(x,y+h,x,y+h-r[3]).lineTo(x,y+r[0]).quadraticCurveTo(x,y,x+r[0],y).closePath();
  }
  return {
    canvas:app.canvas,
    attach(container) { requireThat(!disposed && !mounted,'already_attached','backend'); container.appendChild(app.canvas); mounted=true; },
    resize(v) { requireThat(!disposed,'disposed','backend');currentViewport={...v};app.renderer.resize(v.width,v.height,v.dpi); },
    set_theme(next) { requireThat(Object.hasOwn(themes,next),'unknown_theme','theme');theme=next;app.renderer.background.color=rgb(themes[theme].background);app.renderer.background.alpha=themes[theme].background[3]/255; },
    draw(frame) {
      requireThat(!disposed,'disposed','backend');clear();
      const zoom=frame.camera.zoom; const camera=[zoom,0,0,zoom,-frame.camera.origin[0]*zoom,-frame.camera.origin[1]*zoom];
      let transform=[1,0,0,1,0,0],opacity=1,clip=[0,0,currentViewport.width,currentViewport.height]; const stack=[]; let parent=app.stage;
      for(const command of frame.commands) {
        switch(command.kind) {
          case 'push_transform':stack.push({kind:'transform',transform});transform=multiply(transform,command.matrix);continue;
          case 'push_opacity':stack.push({kind:'opacity',opacity});opacity*=command.opacity;continue;
          case 'push_clip': {
            stack.push({kind:'clip',clip,parent});clip=intersect(clip,command.rect);
            const group=new P.Container();const mask=new P.Graphics().rect(...clip).fill(0xffffff);parent.addChild(mask);parent.addChild(group);group.mask=mask;parent=group;continue;
          }
          case 'pop_transform':transform=stack.pop().transform;continue;
          case 'pop_opacity':opacity=stack.pop().opacity;continue;
          case 'pop_clip': {const old=stack.pop();clip=old.clip;parent=old.parent;continue;}
        }
        const geometry=multiply(camera,transform);let object;
        if(command.kind==='sprite') {
          const resource=assets.resources.find(r=>r.id===command.asset);const variants=resource.variants.map(v=>({...v,density:assets.files.find(f=>f.name===v.file).density})).sort((a,b)=>a.density-b.density);
          const variant=variants.find(v=>v.density>=currentViewport.dpi)??variants.at(-1);const texture=textures.get(`${command.asset}|${variant.file}`);
          requireThat(texture,'missing_asset',command.asset);object=new P.Sprite(texture);object.width=command.rect[2];object.height=command.rect[3];
          const c=Math.cos(command.rotation),s=Math.sin(command.rotation),[px,py]=command.pivot;
          const rotate=[c,s,-s,c,px-c*px+s*py,py-s*px-c*py];const local=[command.rect[2]/texture.width,0,0,command.rect[3]/texture.height,command.rect[0],command.rect[1]];
          object.setFromMatrix(matrix(multiply(geometry,multiply(rotate,local))));object.tint=rgb(command.tint);object.alpha=opacity*command.tint[3]/255;
        } else if(command.kind==='rect'||command.kind==='path') {
          object=new P.Graphics();object.setFromMatrix(matrix(geometry));shape(object,command);if(command.fill)object.fill(fillStyle(command.fill));const b=command.kind==='rect'?command.border:command.stroke;if(b)object.stroke({width:b.width,color:rgb(b.color),alpha:b.color[3]/255,alignment:0.5});object.alpha=opacity;
        } else if(command.kind==='text') {
          const style=themes[theme].text_styles[command.style];requireThat(style,'unknown_text_style',command.style);
          // Baseline uses ProggyClean for every family/weight token. Same bytes; engines' metrics differ.
          const rasterSize=Math.max(16,Math.round(style.font_size)),logicalScale=style.font_size/rasterSize,rasterLineHeight=style.line_height/logicalScale;
          object=new P.Text({ text:command.text,style:{fontFamily:ownedFont?'TabulaSpikeProggy':'monospace',fontSize:rasterSize,lineHeight:rasterLineHeight,fontWeight:'normal',letterSpacing:0,fill:rgb(command.color),align:command.align==='center'?'center':command.align==='end'?'right':'left',wordWrap:command.max_width!==null,wordWrapWidth:command.max_width===null?undefined:command.max_width/logicalScale,breakWords:true} });
          // RenderCmd::Text's at is the line-box origin. Baseline backend draws first baseline
          // at + line_height, while Pixi's canvas places it at ascent + centered leading.
          const fontProperties=P.CanvasTextMetrics.measureText(command.text,object.style).fontProperties;
          const pixiBaseline=fontProperties.ascent+Math.max(0,(rasterLineHeight-fontProperties.fontSize)/2);
          object.anchor.set(command.align==='center'?0.5:command.align==='end'?1:0,0);const local=[logicalScale,0,0,logicalScale,command.at[0],command.at[1]+style.line_height-pixiBaseline*logicalScale];object.setFromMatrix(matrix(multiply(geometry,local)));object.alpha=opacity*command.color[3]/255;
        } else throw new BoundaryError('unsupported_command',command.kind);
        object.eventMode='none';parent.addChild(object);
      }
      latestCommands=frame.commands.length;app.render();drawCount++;
    },
    dispose,
    snapshot() { return { renderer:'pixi-webgl',version:P.VERSION,disposed,mounted,draw_count:drawCount,commands:latestCommands,textures:textures.size,sources:sources.size,font_loaded:Boolean(ownedFont),canvas_width:disposed?0:app.canvas.width,canvas_height:disposed?0:app.canvas.height }; },
  };
}
