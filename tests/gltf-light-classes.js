import './normal-map-probe.js';
const result=document.querySelector('#result'),probe=window.normalMapProbe;
document.documentElement.dataset.rendererBackend=probe.backend;
const assert=(condition,message)=>{if(!condition)throw Error(message);};
const lights=(mask,phase=0)=>Array.from({length:4},(_,i)=>({position:[-2+i*1.4+phase*.3,1+i*.2,4-i*.3],color:mask&(1<<i)?[.4+i*.2,.7-i*.1,.3+i*.15].map(v=>v*(phase?1.3:1)):[0,0,0]}));
try{
 await probe.capture({view:'front-single',normal:true,tangent:true,metal:true,roughness:.2,instances:true,lights:lights(15)},'mapped');
 const initial=await probe.mutateLights(lights(15),true),first=initial.stats;
 assert(Number.isInteger(first.instanceBuffersCreated)&&first.instancedDrawCalls>0,'Real instance counters required');
 let checked=0;const images=new Set();
 for(const directional of [true,false])for(const mask of [15,0,1,2,4,8,3,5,9,6,10,12,7,11,13,14]){
  const image=await probe.mutateLights(lights(mask),directional);images.add(image.png);checked++;
  assert(image.stats.instanceBuffersCreated===first.instanceBuffersCreated,'Light classes must reuse instance buffers');
  assert(image.stats.instanceBufferBytes===first.instanceBufferBytes,'Lighting must not duplicate retained instance storage');
 }
 const moved=await probe.mutateLights(lights(15,1),true);
 assert(moved.png!==initial.png,'Position/color mutation must change actual pixels');
 const restored=await probe.mutateLights(lights(15),true);
 assert(restored.png===initial.png,'Restoring light state must restore exact pixels');
 assert(images.size>=16,'Light masks must affect rendered pixels');
 window.lightClassEvidence={backend:probe.backend,states:checked,distinctImages:images.size,initial:first,final:restored.stats,exactRestoration:true};
 result.textContent=`PASS: glTF light classes · ${checked} mutable masks/types · exact restoration · stable instanced storage`;
}catch(error){result.textContent='FAIL: '+(error.stack||error);throw error;}
finally{probe.dispose();}
