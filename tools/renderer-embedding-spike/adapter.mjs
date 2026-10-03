// Reversible tool-only mount ownership. No authority, sockets, storage, or rules.
import { BoundaryError, validateIdentity, validateView, validateViewport, validatePreferences, validateInput, requireThat } from './boundary.mjs';
import { createPixiBackend } from './pixi-backend.mjs';
const owners = new WeakMap();

export function mount(options) {
  const {container,contract,assets,themes,on_input=()=>{},on_error=()=>{},on_frame=()=>{},backendFactory=createPixiBackend}=options;
  const identity=structuredClone(validateIdentity(options.identity));let viewport=structuredClone(validateViewport(options.viewport));
  let preferences=structuredClone(validatePreferences(options.preferences??{reduced_motion:false,audio_enabled:false,volume:0,modal:false}));
  requireThat(container && typeof container.appendChild==='function','invalid_container','mount');
  const env=options.environment??{window:globalThis.window,document:globalThis.document,requestAnimationFrame:globalThis.requestAnimationFrame.bind(globalThis),cancelAnimationFrame:globalThis.cancelAnimationFrame.bind(globalThis),now:()=>globalThis.performance.now()};
  owners.get(container)?.dispose();
  const cancellation=new AbortController();let status='initializing',backend=null,view=null,revision=-1,raf=null,manualSuspended=false,visibilitySuspended=env.document.visibilityState==='hidden';
  let frames=0,inputs=0,dropped=0,errors=0,lastFrame=null,lastInput=null,pointer=null,focusBeforeModal=null,lastResources=null;const removers=[];
  function report(error) {errors++;on_error({code:error.code??'renderer_error',message:error.message,session_id:identity.session_id,generation:identity.generation,revision});}
  function assertActive() {requireThat(status!=='disposed'&&status!=='failed','disposed','adapter');}
  function active() {return status==='ready'&&!manualSuspended&&!visibilitySuspended&&!preferences.modal;}
  function emit(input,force=false) {
    if(status==='disposed'||status==='failed'||(!force&&!active())||revision<0){dropped++;return;}
    validateInput(input);inputs++;lastInput=env.now();on_input({session_id:identity.session_id,generation:identity.generation,revision,input});
  }
  function cancelInput() {
    if(pointer) {emit({kind:'pointer',position:pointer.position,button:pointer.button,phase:'cancel'},true);try{backend?.canvas.releasePointerCapture?.(pointer.id);}catch{}pointer=null;}
    emit({kind:'focus',focused:false},true);
  }
  function cancelRaf() {if(raf!==null){env.cancelAnimationFrame(raf);raf=null;}}
  function schedule() {if(status==='ready'&&!manualSuspended&&!visibilitySuspended&&raf===null)raf=env.requestAnimationFrame(tick);}
  function tick(now) {
    raf=null;if(status!=='ready'||manualSuspended||visibilitySuspended)return;
    try {
      if(view) {const start=env.now();backend.draw(view.frame);const end=env.now();frames++;on_frame({now_ms:now,draw_ms:end-start,frame_interval_ms:lastFrame===null?null:now-lastFrame,input_to_next_draw_ms:lastInput===null?null:end-lastInput,rendered_revision:revision,commands:view.frame.commands.length});lastFrame=now;lastInput=null;}
      schedule();
    }catch(error){status='failed';cancelInput();cancelRaf();report(error);cleanup();}
  }
  function listen(target,name,handler,config) {target.addEventListener(name,handler,config);removers.push(()=>target.removeEventListener(name,handler,config));}
  function cleanup() {while(removers.length)removers.pop()();if(backend){backend.dispose();lastResources=backend.snapshot();backend=null;}}
  function point(event) {const bounds=backend.canvas.getBoundingClientRect();requireThat(bounds.width>0&&bounds.height>0,'invalid_canvas_bounds','input');return [(event.clientX-bounds.left)*viewport.width/bounds.width,(event.clientY-bounds.top)*viewport.height/bounds.height];}
  function button(number) {return number===2?'secondary':number===1?'middle':'primary';}
  function listeners() {
    const canvas=backend.canvas;canvas.tabIndex=0;canvas.setAttribute?.('aria-label','Rust-projected board prototype');canvas.setAttribute?.('role','application');canvas.style.touchAction='none';
    listen(canvas,'pointerdown',event=>{if(!active()){dropped++;return;}if(pointer){dropped++;return;}try{const position=point(event);pointer={id:event.pointerId,position,button:button(event.button)};canvas.focus({preventScroll:true});canvas.setPointerCapture?.(event.pointerId);emit({kind:'pointer',position,button:pointer.button,phase:'down'});event.preventDefault();}catch(error){report(error);}});
    listen(canvas,'pointermove',event=>{if(pointer&&pointer.id!==event.pointerId){dropped++;return;}try{const position=point(event);if(pointer)pointer.position=position;emit({kind:'pointer',position,button:pointer?.button??button(event.button),phase:'move'});}catch(error){report(error);}});
    listen(canvas,'pointerup',event=>{if(!pointer||pointer.id!==event.pointerId){dropped++;return;}try{const position=point(event);emit({kind:'pointer',position,button:pointer.button,phase:'up'});canvas.releasePointerCapture?.(pointer.id);pointer=null;}catch(error){report(error);cancelInput();}});
    listen(canvas,'pointercancel',()=>cancelInput());listen(canvas,'lostpointercapture',()=>{if(pointer)cancelInput();});
    for(const name of ['keydown','keyup'])listen(canvas,name,event=>{const key=event.key===' '?'Space':event.key;if(!['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Enter','Space','Escape','Tab'].includes(key)||event.repeat){dropped++;return;}emit({kind:'key',key,pressed:name==='keydown'});if(key!=='Tab'&&active())event.preventDefault();});
    listen(canvas,'focus',()=>emit({kind:'focus',focused:true}));listen(canvas,'blur',()=>cancelInput());
    listen(canvas,'webglcontextlost',event=>{event.preventDefault();cancelInput();manualSuspended=true;cancelRaf();report(new BoundaryError('context_lost','canvas'));});
    listen(canvas,'webglcontextrestored',()=>{report(new BoundaryError('context_restored_remount_required','canvas'));});
    listen(env.window,'blur',()=>cancelInput());
    listen(env.document,'visibilitychange',()=>{visibilitySuspended=env.document.visibilityState==='hidden';if(visibilitySuspended){cancelInput();cancelRaf();lastFrame=null;}else schedule();});
  }
  const controller={
    ready:null,
    set_view(envelope) {assertActive();const accepted=validateView(envelope,identity,revision,contract,assets,themes);view=accepted;revision=accepted.revision;backend?.set_theme?.(accepted.frame.theme);backend?.canvas.setAttribute?.('aria-label',accepted.frame.a11y);schedule();return revision;},
    resize(value) {assertActive();viewport=structuredClone(validateViewport(value));cancelInput();backend?.resize(viewport);schedule();},
    set_preferences(value) {
      assertActive();const next=structuredClone(validatePreferences(value));
      if(next.modal&&!preferences.modal){focusBeforeModal=env.document.activeElement;cancelInput();backend?.canvas.blur?.();}
      const closeModal=!next.modal&&preferences.modal;preferences=next;
      if(closeModal&&focusBeforeModal===backend?.canvas)backend.canvas.focus({preventScroll:true});
      // No audio fixture exists. Record settings without creating AudioContext or claiming playback.
      schedule();return {audio_supported:false,reduced_motion:preferences.reduced_motion};
    },
    set_theme(theme) {assertActive();requireThat(Object.hasOwn(themes,theme),'unknown_theme','theme');options.theme=theme;backend?.set_theme(theme);schedule();},
    suspend() {assertActive();manualSuspended=true;cancelInput();cancelRaf();lastFrame=null;},
    resume() {assertActive();manualSuspended=false;schedule();},
    dispose() {
      if(status==='disposed')return;status='disposed';cancelInput();cancellation.abort();cancelRaf();cleanup();view=null;
      if(owners.get(container)===controller)owners.delete(container);
    },
    snapshot() {return {status,session_id:identity.session_id,generation:identity.generation,revision,listeners:removers.length,pending_raf:raf===null?0:1,frames,inputs,dropped_inputs:dropped,errors,manual_suspended:manualSuspended,visibility_suspended:visibilitySuspended,modal:preferences.modal,audio_supported:false,preferences:{...preferences},viewport:{...viewport},resources:backend?.snapshot()??lastResources};},
  };
  owners.set(container,controller);
  controller.ready=(async()=>{
    try {
      const created=await backendFactory({...options,viewport,assets,themes,signal:cancellation.signal});
      if(status==='disposed'||cancellation.signal.aborted||owners.get(container)!==controller){created.dispose();return controller;}
      backend=created;backend.attach(container);backend.resize(viewport);backend.set_theme?.(view?.frame.theme??options.theme??'light');listeners();status='ready';schedule();return controller;
    }catch(error){if(status!=='disposed'){status='failed';report(error);cleanup();}return controller;}
  })();
  return controller;
}
