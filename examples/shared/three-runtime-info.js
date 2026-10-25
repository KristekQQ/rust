import {mountRuntimeInfo} from './runtime-info-ui.js';
// Bounded observational samples; the existing Three animation loop remains the owner.
export function installThreeRuntimeInfo(renderer,{readWorld=()=>({}),panel}={}){
 const frames=[],cpu=[];let previous=0,lastSnapshot;
 const percentile=values=>{if(!values.length)return undefined;const s=[...values].sort((a,b)=>a-b);return s[Math.ceil(s.length*.95)-1];};
 const mean=values=>values.length?values.reduce((a,b)=>a+b,0)/values.length:undefined;
 function readSnapshot(){
  const canvas=renderer.domElement,layer=renderer.xr.getSession()?.renderState?.baseLayer;
  const frameMs=mean(frames),world=readWorld();
  lastSnapshot={backend:'WebGL2',renderer:'Three.js',mode:renderer.xr.isPresenting?'XR aplikace':'desktop',
   resolution:{width:layer?.framebufferWidth??canvas.width,height:layer?.framebufferHeight??canvas.height,dpr:devicePixelRatio,scope:layer?'XR framebuffer':'canvas'},
   fps:frameMs?1000/frameMs:undefined,frameMs,frameP95Ms:percentile(frames),
   cpu:cpu.length?{'celý callback (update + render submit)':{meanMs:mean(cpu),p95Ms:percentile(cpu)}}:{},
   gpu:{supported:false,available:false,error:'timestamp měření není zapojené'},
   drawCalls:renderer.info.render.calls,triangles:renderer.info.render.triangles,
   bvhNodes:world.nodes,objectTests:world.tests,cuts:world.cuts};
  panel?.setMetrics(lastSnapshot);return lastSnapshot;
 }
 const ui=mountRuntimeInfo({readSnapshot,isPresenting:()=>renderer.xr.isPresenting});
 return {begin(time){if(previous&&time>previous&&time-previous<=1000){frames.push(time-previous);if(frames.length>120)frames.shift();}previous=time;return performance.now();},
  end(start){cpu.push(performance.now()-start);if(cpu.length>120)cpu.shift();},
  get visible(){return ui.visible;},describe:readSnapshot,dispose:()=>ui.dispose()};
}
