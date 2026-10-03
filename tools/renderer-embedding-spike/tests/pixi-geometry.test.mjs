import test from 'node:test';import assert from 'node:assert/strict';import * as Pixi from 'pixi.js';
import {createPixiBackend} from '../pixi-backend.mjs';import {Target} from './helpers.mjs';
class Application {
 static latest;
 constructor(){Application.latest=this;this.stage=new Pixi.Container();this.canvas=new Target();this.renderer={events:{setTargetElement(){}},background:{},resize(){}};}
 async init(){}stop(){}render(){this.stage.getBounds();}destroy(){this.stage.destroy({children:true});}
}
test('actual Pixi local stroke geometry scales with camera zoom and inherited transform',async()=>{
 const backend=await createPixiBackend({viewport:{width:900,height:720,dpi:1},assets:{files:[],resources:[]},themes:{light:{background:[255,255,255,255],text_styles:{}}},baseUrl:'http://localhost:8760/',pixi:{...Pixi,Application}});
 // Actual Pixi scene/geometry; application/GPU initialization is substituted, not browser evidence.
 const frame={camera:{origin:[0,0],zoom:2},commands:[{kind:'push_transform',matrix:[3,0,0,3,0,0]},{kind:'path',points:[[0,0],[10,0]],closed:false,fill:null,stroke:{width:2,color:[0,0,0,255]}},{kind:'pop_transform'}]};
 backend.draw(frame);assert.equal(backend.snapshot().draw_count,1);
 // getBounds reaches actual Pixi scene transform and stroke geometry, independently of our matrix math.
 const object=Application.latest.stage.children[0],bounds=object.getBounds();assert.equal(bounds.minY,-6);assert.equal(bounds.maxY,6);
 backend.dispose();assert.equal(backend.snapshot().sources,0);
});
