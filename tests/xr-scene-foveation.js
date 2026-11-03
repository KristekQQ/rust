import {initEngine} from '../js/engine.js';
import {installXRSimulator} from './xr/simulator.js';
import {XRTestRun,publishXRReport,pixelDifference} from './xr/testkit.js';
import {assertForeground} from './pixels.js';
const asymmetric=new URLSearchParams(location.search).has('asymmetric');
const sim=installXRSimulator({clockTimers:false,profile:{eyeWidth:192,eyeHeight:208,...(asymmetric?{viewports:[{x:0,y:8,width:192,height:208},{x:203,y:0,width:176,height:192}]}:{})}});
const run=new XRTestRun('xr-scene-foveation',sim,publishXRReport),canvas=document.querySelector('canvas');
const previousBinding=Object.getOwnPropertyDescriptor(globalThis,'XRWebGLBinding');
let engine,failAt=0,positiveCalls=0,failReset=false,restored=0,calls=[],undo;
const levels=new Map();
class Binding {
 constructor(session,gl){this.session=session;this.gl=gl;session.addEventListener('end',()=>{for(const value of levels.values())if(value.session===session)value.level=0;},{once:true});}
 foveateBoundTexture(target,level){
  const gl=this.gl,texture=gl.getParameter(gl.TEXTURE_BINDING_2D);
  if(target!==gl.TEXTURE_2D||!texture||!gl.isTexture(texture))throw Error('Not a live bound scene texture');
  calls.push({texture,level,session:this.session});
  // The observer below checks restoration even when this call throws. No
  // reduction of actual GPU shading is simulated or claimed by this fixture.
  const state=gl.__lastFoveationBind;
  gl.activeTexture(gl.TEXTURE15);
  gl.__expectedFoveationRestore=state;
  levels.set(texture,{level,session:this.session});
  if(level===1&&++positiveCalls===failAt)throw Error('Injected texture foveation failure');
  if(level===0&&failReset)throw Error('Injected rollback failure');
 }
}
Object.defineProperty(globalThis,'XRWebGLBinding',{configurable:true,value:Binding});
const report=await run.execute(async test=>{
 engine=await initEngine(canvas,{backend:'webgl'});document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 engine.resize(320,240);engine.set_simulation_paused(true);engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
 engine.setBackgroundColor([.08,.08,.08]);engine.scene.createLight({position:[-2,4,0],color:[8,8,8]});
 let cube=engine.scene.createCube([-.2,1.7,-2]).setScale([.4,.3,.35]);
 engine.scene.createCube([.4,1.4,-2.5]).setScale([.25,.2,.2]).setOpacity(.45);
 const gl=canvas.getContext('webgl2'),bind=gl.bindTexture;
 gl.bindTexture=function(target,texture){
  const before={texture:this.getParameter(this.TEXTURE_BINDING_2D),active:this.getParameter(this.ACTIVE_TEXTURE)};
  const expected=this.__expectedFoveationRestore;this.__expectedFoveationRestore=null;
  const result=bind.call(this,target,texture);
  if(expected){
   // WebGL coerces nullable undefined to null (wasm-bindgen Option::None).
   // Compare the actual GL state after binding, not uncoerced JS arguments.
   if(target!==this.TEXTURE_2D||this.getParameter(this.TEXTURE_BINDING_2D)!==expected.texture||this.getParameter(this.ACTIVE_TEXTURE)!==expected.active)throw Error('FFR failed to restore actual GL active unit/texture binding');
   restored++;
  }
  this.__lastFoveationBind=before;
  return result;
 };
 undo=()=>{gl.bindTexture=bind;};
 const draw=()=>{sim.frame({dt:0,head:{position:[0,1.6,0]}});test.assert(!engine.xr.lastError,'real stereo renderer completes',String(engine.xr.lastError));};
 const capture=async (enabled,{fail=false}={})=>{
  calls=[];positiveCalls=0;failAt=Number(fail);let settings;
  const listener=e=>{settings=e.detail;};engine.xr.addEventListener('firstframe',listener);
  try{
   await engine.xr.enter({webglPresentation:'shared-framebuffer',sceneFoveation:enabled,fixedFoveation:1});
   test.assert(engine.xr.presentationMode==='shared-framebuffer','uses actual renderer GL context');
   draw();draw();const count=calls.length;draw();
   if(enabled&&!fail&&!asymmetric)test.assert(calls.length===count,'unchanged allocations do not repeat foveation API calls');
   const images=['left','right'].map(eye=>sim.pixels(eye));images.forEach((image,i)=>assertForeground(image,'actual eye'+i));
   const stats=engine.xr.sceneFoveation;
   if(enabled){
    test.assert(stats.status===(fail?'failed':'configured'),'explicit invocation/failure status',stats);
    test.assert(stats.trackedTextures<=8&&stats.hardwareEffect==='unverified','bounded non-owning tracking does not claim hardware effect',stats);
    if(fail)test.assert(stats.failures===1&&calls.some(call=>call.level===0)&&[...levels].every(([texture,value])=>!gl.isTexture(texture)||value.level===0),'failure resets ALL prior live textures, including earlier eye/MRT attachment');
    else test.assert(stats.calls>=1&&calls.some(call=>call.level===1)&&calls.every(call=>call.level===0||call.level===1),'FFR sees actual renderer-owned textures; eviction may reset0');
   }
   test.assert(settings.sceneFoveation.status===(enabled?(fail?'failed':'configured'):'disabled'),'first-frame diagnostics report actual state',settings.sceneFoveation);
   await engine.xr.exit();
   test.assert(engine.xr.sceneFoveation.status==='disabled','session end drops FFR configuration');
   return images;
  }finally{engine.xr.removeEventListener('firstframe',listener);}
 };
 const compare=async label=>{
  const before=await capture(false),after=await capture(true);
  test.assert(before.every((image,i)=>pixelDifference(image,after[i],0)===0),label+' exact stereo pixels with observational FFR shim');
 };
 await compare('no effects');
 const plainReference=await capture(false),secondEyeFailure=await capture(true,{fail:2});
 test.assert(plainReference.every((image,i)=>pixelDifference(image,secondEyeFailure[i],0)===0),'second-eye failure preserves exact stereo output');
 engine.set_render_effects(true,true);engine.postprocessing.enable('fxaa',{strength:.6});engine.postprocessing.enable('bloom',{strength:.22});cube.setEmission(1.5);
 engine.postprocessing.configureSSR({resolutionScale:.5});engine.postprocessing.enable('ssr',{strength:.92});
 await compare('packed depth / SSR / FXAA / bloom / alpha');
 const before=await capture(false),failed=await capture(true,{fail:2});
 test.assert(before.every((image,i)=>pixelDifference(image,failed[i],0)===0),'throwing FFR preserves completed stereo output');
 // Same-session1->0 must reset inactive scene/packed-depth textures too.
 await engine.xr.enter({webglPresentation:'shared-framebuffer',sceneFoveation:true,fixedFoveation:1});positiveCalls=0;failAt=0;draw();
 for(const name of ['ssr','bloom','fxaa'])engine.postprocessing.disable(name);engine.set_render_effects(false,false);
 engine.xr.setSceneFoveation(0);
 test.assert([...levels].every(([texture,value])=>!gl.isTexture(texture)||value.level===0),'level0 resets inactive old targets before replacement');
 draw();engine.postprocessing.enable('fxaa',{strength:.6});draw();
 test.assert([...levels].every(([texture,value])=>!gl.isTexture(texture)||value.level===0),'old scene targets stay0 when effects return');
 await engine.xr.exit();
 // Refuse mixed preferences if the browser also rejects cleanup. Use two
 // independent eye targets so failure2 is reached even at equal dimensions.
 for(const name of ['ssr','bloom','fxaa'])engine.postprocessing.disable(name);engine.set_render_effects(false,false);
 await engine.xr.enter({webglPresentation:'shared-framebuffer',sceneFoveation:true,fixedFoveation:1});positiveCalls=0;failAt=2;failReset=true;
 sim.frame({dt:0,head:{position:[0,1.6,0]}});
 test.assert(!engine.xr.isPresenting&&/rollback failed/i.test(String(engine.xr.lastError)),'rollback failure ends XR instead of publishing mixed preferences',{positiveCalls,error:String(engine.xr.lastError),presenting:engine.xr.isPresenting});
 failReset=false;failAt=0;
 engine.scene.clear();cube=engine.scene.createCube([0,1.6,-2]).setScale([.3,.3,.3]);engine.scene.createLight({position:[-2,4,0],color:[8,8,8]});
 await compare('clear/reload and reentry');
 const method=Binding.prototype.foveateBoundTexture;delete Binding.prototype.foveateBoundTexture;
 let first;engine.xr.addEventListener('firstframe',e=>{first=e.detail;},{once:true});
 await engine.xr.enter({webglPresentation:'shared-framebuffer',sceneFoveation:true,fixedFoveation:1});draw();
 test.assert(first.sceneFoveation.status==='unavailable','missing browser method is explicitly unavailable');
 test.assert(engine.xr.presentationMode==='canvas-transfer','absent scene API preserves original presentation path');
 await engine.xr.exit();Binding.prototype.foveateBoundTexture=method;
 test.assert(restored>0&&!gl.__expectedFoveationRestore,'binding restoration verified for success, rollback and exceptions',restored);
});
undo?.();engine?.dispose();sim.dispose();
if(previousBinding)Object.defineProperty(globalThis,'XRWebGLBinding',previousBinding);else delete globalThis.XRWebGLBinding;
publishXRReport(report);
