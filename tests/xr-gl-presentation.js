import {initEngine} from '../js/engine.js';
import {installXRSimulator} from './xr/simulator.js';
import {XRTestRun,publishXRReport,pixelDifference} from './xr/testkit.js';
import {assertForeground,capturePixels} from './pixels.js';
const sim=installXRSimulator({clockTimers:false,profile:{id:'asymmetric-gl',eyeWidth:192,eyeHeight:208,viewports:[{x:0,y:8,width:192,height:208},{x:203,y:0,width:176,height:192}]}}),run=new XRTestRun('xr-gl-presentation',sim,publishXRReport);
const canvas=document.querySelector('canvas');let engine;
window.presentationEvidence={pairs:[],captures:{},settings:[]};
function png(image){const c=document.createElement('canvas');c.width=image.width;c.height=image.height;c.getContext('2d').putImageData(image,0,0);return c.toDataURL();}
const report=await run.execute(async test=>{
 engine=await initEngine(canvas,{backend:'webgl'});document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 engine.resize(320,240);engine.set_simulation_paused(true);engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
 engine.setBackgroundColor([.08,.08,.08]);engine.setCameraView([0,1.6,0],[0,1.6,-2]);
 const cube=engine.scene.createCube([-.22,1.72,-2]);cube.setScale([.35,.35,.35]);
 engine.scene.createCube([.4,1.35,-2.4]).setScale([.25,.2,.2]);
 engine.scene.createLight({position:[-2,4,0],color:[8,8,8]});
 let invalidFramebuffersChecked=false;
 const draw=()=>{sim.frame({dt:0,head:{position:[0,1.6,0]}});test.assert(!engine.xr.lastError,'stereo frame completes',String(engine.xr.lastError));};
 const capture=async (mode,label)=>{
  let settings;const listener=e=>{settings=e.detail;};engine.xr.addEventListener('firstframe',listener);
  try{
   await engine.xr.enter({webglPresentation:mode,presentationDiagnostics:true});
   test.assert(engine.xr.presentationMode===mode,'explicit route selected',engine.xr.presentationMode);
   test.assert((sim.layer.gl===canvas.getContext('webgl2'))===(mode==='shared-framebuffer'),'actual GL context ownership matches route');
   // Enforce the opaque attachment restrictions using an ordinary simulator
   // FBO. These guards are a regression oracle, not a real headset substitute.
   const gl=sim.layer.gl,layerFb=sim.layer.framebuffer,restore=[];
   // Chromium opaque XR FBOs have not been publicly bound yet. The simulator
   // prebinds its regular FBO to attach storage, so inject that first-bind API
   // behavior independently of the renderer. This is not a hardware XR test.
   if(mode==='shared-framebuffer'){
    let pending=true,probe=null,restored=0;
    const isFramebuffer=gl.isFramebuffer,bindFramebuffer=gl.bindFramebuffer;
    gl.isFramebuffer=function(fb){
     if(fb===layerFb&&pending){probe={draw:this.getParameter(this.DRAW_FRAMEBUFFER_BINDING),read:this.getParameter(this.READ_FRAMEBUFFER_BINDING)};return false;}
     return isFramebuffer.call(this,fb);
    };
    gl.bindFramebuffer=function(target,fb){
     const result=bindFramebuffer.call(this,target,fb);
     if(fb===layerFb&&this.getParameter(this.DRAW_FRAMEBUFFER_BINDING)===layerFb)pending=false;
     else if(probe&&!pending&&target===this.DRAW_FRAMEBUFFER&&fb===probe.draw){
      test.assert(this.getParameter(this.DRAW_FRAMEBUFFER_BINDING)===probe.draw&&this.getParameter(this.READ_FRAMEBUFFER_BINDING)===probe.read,'cold ownership probe restores DRAW and preserves READ');restored++;probe=null;
     }
     return result;
    };
    restore.push(()=>{gl.isFramebuffer=isFramebuffer;gl.bindFramebuffer=bindFramebuffer;});
    test.assert(!gl.isFramebuffer(layerFb),'opaque first-bind oracle starts unrecognized');
    restore.push(()=>{test.assert(!pending&&restored===1,'cold probe first-binds exactly once before backend presentation');});
    if(!invalidFramebuffersChecked){
     // The public SDK is frozen. Reuse its already-loaded raw test module,
     // rather than modifying the production SDK or attempting to monkeypatch it.
     const url=performance.getEntriesByType('resource').findLast(e=>/\/pkg\/webgpu_wasm\.js\?/.test(e.name))?.name;
     if(!url)throw Error('Current WASM binding URL missing');
     const raw=await import(url),viewports=['left','right'].map(eye=>sim.layer.getViewport({eye}));
     raw.xr_begin_frame(0);
     for(const [index,v] of viewports.entries()){
      const f=1/Math.tan(65*Math.PI/360),projection=new Float32Array([f/(v.width/v.height),0,0,0,0,f,0,0,0,0,-1,-1,0,0,-.1,0]);
      const world=new Float32Array([1,0,0,0,0,1,0,0,0,0,1,0,index?.032:-.032,1.6,0,1]);
      raw.xr_render_gl_eye(projection,world,index,v.width,v.height);
     }
     const args=[layerFb,sim.layer.framebufferWidth,sim.layer.framebufferHeight,viewports.map(v=>[v.x,v.y,v.width,v.height])];
     const oldDraw=gl.getParameter(gl.DRAW_FRAMEBUFFER_BINDING),oldRead=gl.getParameter(gl.READ_FRAMEBUFFER_BINDING);
     const drawFb=gl.createFramebuffer(),readFb=gl.createFramebuffer(),deleted=gl.createFramebuffer(),incomplete=gl.createFramebuffer();
     const foreignGl=document.createElement('canvas').getContext('webgl2'),foreign=foreignGl.createFramebuffer();
     gl.deleteFramebuffer(deleted);gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER,drawFb);gl.bindFramebuffer(gl.READ_FRAMEBUFFER,readFb);
     try{
      test.assert(gl.getError()===gl.NO_ERROR,'negative ownership checks begin with clean GL errors');
      for(const [fb,expected,errorCode] of [[foreign,'must belong',gl.INVALID_OPERATION],[deleted,'must belong',gl.INVALID_OPERATION],[incomplete,'not active or complete',gl.NO_ERROR]]){
       let error;try{raw.xr_present_gl_frame(fb,...args.slice(1));}catch(e){error=e;}
       test.assert(String(error).includes(expected),'foreign/deleted/incomplete FBO rejected before consuming eyes',String(error));
       test.assert(gl.getParameter(gl.DRAW_FRAMEBUFFER_BINDING)===drawFb&&gl.getParameter(gl.READ_FRAMEBUFFER_BINDING)===readFb,'failed probe preserves distinct READ/DRAW bindings');
       test.assert(gl.getError()===errorCode&&gl.getError()===gl.NO_ERROR,'only expected invalid-object bind error is produced');
      }
      invalidFramebuffersChecked=true;
     }finally{
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER,oldDraw);gl.bindFramebuffer(gl.READ_FRAMEBUFFER,oldRead);
      for(const fb of [drawFb,readFb,incomplete])gl.deleteFramebuffer(fb);foreignGl.deleteFramebuffer(foreign);
     }
     raw.xr_present_gl_frame(...args);raw.xr_finish_frame();
    }
   }
   for(const name of ['framebufferTexture2D','framebufferRenderbuffer','getFramebufferAttachmentParameter','deleteFramebuffer']){
    const original=gl[name];gl[name]=function(...args){
     if(name==='deleteFramebuffer'?args[0]===layerFb:this.getParameter(this.FRAMEBUFFER_BINDING)===layerFb)throw Error('Opaque XR attachment operation: '+name);
     return original.apply(this,args);
    };restore.push(()=>{gl[name]=original;});
   }
   try{draw();draw();draw();}finally{for(const undo of restore.reverse())undo();}
   test.assert(true,'opaque attachment guards accept backend presentation');
   const images=['left','right'].map(eye=>sim.pixels(eye));images.forEach((im,i)=>assertForeground(im,label+' eye'+i));
   window.presentationEvidence.captures[label+'-'+mode]=images.map(png);
   test.assert(images.every((im,i)=>pixelDifference(im,sim.pixels(i?'right':'left'),0)===0),'readback stable outside callback');
   test.assert(settings.antialias===false && settings.depth===false,'actual XR layer AA/depth configuration',settings);
   window.presentationEvidence.settings.push({label,mode,...settings});
   const profile=engine.xr.presentationProfile;test.assert(profile.frameCount>=3 && profile.frames.every(f=>Number.isFinite(f.cpuMs)),'opt-in bounded phase diagnostics record real callbacks');
   await engine.xr.exit();test.assert(!canvas.getContext('webgl2').isContextLost(),'XR cleanup preserves renderer GL context');
   test.assert(canvas.width===320&&canvas.height===240,'desktop dimensions restored');return images;
  }finally{engine.xr.removeEventListener('firstframe',listener);}
 };
 const compare=async label=>{
  const reference=await capture('canvas-transfer',label),candidate=await capture('shared-framebuffer',label);
  const differences=reference.map((a,i)=>{let max=0;for(let j=0;j<a.data.length;j++)max=Math.max(max,Math.abs(a.data[j]-candidate[i].data[j]));return {eye:i,changedPixels:pixelDifference(a,candidate[i],0),max,width:a.width,height:a.height};});
  window.presentationEvidence.pairs.push({case:label,differences});
  test.assert(differences.every(d=>d.changedPixels===0&&d.max===0),'independent original-route exact stereo pixels: '+label,differences);
 };
 await compare('opaque-color-depth-orientation');
 // Backface color, transparency, overlap and near clipping go through the same
 // production render path; the independent canvas route remains the oracle.
 const glass=engine.scene.createCube([0,1.6,-1.6]).setScale([.3,.3,.1]).setOpacity(.42);
 const clipped=engine.scene.createCube([0,1.6,-.025]).setScale([.3,.3,.003]);
 await compare('alpha-blending-depth-near-clip');clipped.remove();glass.remove();
 engine.set_render_effects(true,true);engine.postprocessing.enable('fxaa',{strength:.6});
 engine.postprocessing.enable('bloom',{strength:.22});cube.setEmission(1.5);
 await compare('shadows-fxaa-bloom');
 engine.postprocessing.configureSSR({resolutionScale:.5});engine.postprocessing.enable('ssr',{strength:.92});
 await compare('ssr-postprocessing');
 engine.postprocessing.disable('ssr');engine.postprocessing.disable('bloom');engine.postprocessing.disable('fxaa');engine.set_render_effects(false,false);
 engine.scene.clear();engine.scene.createLight({position:[-2,4,0],color:[12,12,12]});
 const source=await engine.scene.loadModel(new URL('../assets/models/damaged-helmet/DamagedHelmet.glb',import.meta.url));
 [source,source.createInstance(),source.createInstance()].forEach((m,i)=>m.setPosition([-.55+i*.55,1.6,-2]).setScale([.18,.18,.18]));
 await compare('shared-gltf-geometry');
 // Every capture has reentered XR. Explicit clear/reload checks stage ownership.
 engine.scene.clear();engine.scene.createCube([0,1.6,-2]).setScale([.3,.3,.3]);
 await compare('clear-reload');
 await new Promise(r=>setTimeout(r,100));assertForeground(await capturePixels(canvas),'return desktop');
 // Losing only the private presentation context must preserve the engine.
 await engine.xr.enter();const privateGl=sim.layer.gl;const loss=new Promise(resolve=>privateGl.canvas.addEventListener('webglcontextlost',resolve,{once:true}));privateGl.getExtension('WEBGL_lose_context').loseContext();await loss;
 await new Promise(r=>setTimeout(r,20));test.assert(!engine.xr.isPresenting&&!canvas.getContext('webgl2').isContextLost(),'private XR context loss ends session and preserves renderer');
 await engine.xr.enter({webglPresentation:'shared-framebuffer'});draw();await engine.xr.exit();
 // Real shared context loss cannot transparently restore wgpu resources. Session
 // ends and explicit engine disposal/reinitialization is required and tested.
 await engine.xr.enter({webglPresentation:'shared-framebuffer'});draw();const gl=canvas.getContext('webgl2');const lost=new Promise(resolve=>canvas.addEventListener('webglcontextlost',resolve,{once:true}));gl.getExtension('WEBGL_lose_context').loseContext();await lost;await new Promise(r=>setTimeout(r,20));
 test.assert(!engine.xr.isPresenting && /reinitialize/.test(String(engine.xr.lastError)),'shared context loss ends XR with explicit reinitialization requirement');
 engine.dispose();canvas.replaceWith(canvas.cloneNode());
 const freshCanvas=document.querySelector('canvas');engine=await initEngine(freshCanvas,{backend:'webgl'});engine.scene.createCube([0,1.6,-2]);engine.setCameraView([0,1.6,0],[0,1.6,-2]);
 await engine.xr.enter({webglPresentation:'shared-framebuffer'});sim.frame({dt:0,head:{position:[0,1.6,0]}});test.assert(!engine.xr.lastError,'fresh engine can reenter shared XR after context loss');await engine.xr.exit();
});
engine?.dispose();sim.dispose();report.presentation=window.presentationEvidence;publishXRReport(report);
