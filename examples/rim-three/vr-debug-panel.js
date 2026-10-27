import * as THREE from 'three';

// A single spatial mesh shared by both eyes. Player reference rotation, never gaze yaw.
export function createVRDebugPanel(scene,renderer){
 const offset=new THREE.Vector3(.9,.35,-1),canvas=document.createElement('canvas');
 canvas.width=768;canvas.height=448;
 const ctx=canvas.getContext('2d'),texture=new THREE.CanvasTexture(canvas);texture.colorSpace=THREE.SRGBColorSpace;
 const material=new THREE.MeshBasicMaterial({map:texture,toneMapped:false,fog:false});
 const mesh=new THREE.Mesh(new THREE.PlaneGeometry(.66,.40),material);mesh.name='DEBUG · pravá noha';mesh.visible=false;mesh.castShadow=false;mesh.receiveShadow=false;scene.add(mesh);
 const state={fps:null,frameMs:null,frames:0,elapsed:0,refreshes:0};let paintElapsed=.25,snapshot=null,stressInfo=null;
 const local=new THREE.Vector3(),lastHead=new THREE.Vector3(),metrics={x:0,y:0,z:0,solids:0,nodes:0};
 function paint(){
  ctx.fillStyle='#102126';ctx.fillRect(0,0,768,448);ctx.fillStyle='#e7c982';ctx.font='bold 43px monospace';ctx.fillText('DEBUG · AURELIA',28,63);
  ctx.fillStyle='#edf4ed';ctx.font='28px monospace';
  const n=v=>v===null?'--':v.toFixed(1),render=renderer.info.render;
  const lines=[`FPS APP ${n(state.fps)}   MS ${n(state.frameMs)}`,`DRAW ${render.calls}   TRI ${render.triangles}`,`BVH ${metrics.nodes}   SOLID ${metrics.solids}`,`X ${metrics.x.toFixed(1)}  Y ${metrics.y.toFixed(1)}  Z ${metrics.z.toFixed(1)}`];
  if(snapshot){
   const s=snapshot,r=s.resolution,c=Object.values(s.cpu??{})[0];
   lines.splice(0,lines.length,`THREE.JS WEBGL2 / XR`,
    `FPS ${n(s.fps??null)} MS ${n(s.frameMs??null)} P95 ${n(s.frameP95Ms??null)}`,
    `CPU CALLBACK ${n(c?.meanMs??null)} P95 ${n(c?.p95Ms??null)}`,
    `GPU MS N/A (NO TIMESTAMPS)`,
    `DRAW ${render.calls} TRI ${render.triangles}`,
    `TARGET ${r.width}x${r.height} ${r.scope==='XR framebuffer'?'XR':'CANVAS'}`,
    `BVH ${metrics.nodes} SOLID ${metrics.solids}`,
    `ZATEZ ${stressInfo?.count??0} ${stressInfo?.representation==='objects'?'OBJECTS':'INSTANCED'}`);
  }
  lines.forEach((line,i)=>ctx.fillText(line,28,112+i*42));
  texture.needsUpdate=true;state.refreshes++;
 }
 paint();
 return {
  mesh,state,
  setMetrics(value){snapshot=value;},
  setStressInfo(value){stressInfo={...value};},
  sample(dt){
   if(!Number.isFinite(dt)||dt<=0||dt>1)return;
   state.frames++;state.elapsed+=dt;paintElapsed+=dt;
   if(paintElapsed>=.25){state.fps=state.frames/state.elapsed;state.frameMs=1000*state.elapsed/state.frames;paintElapsed=0;state.frames=0;state.elapsed=0;paint();}
  },
  sync(head,rig,visible,world){
   mesh.visible=visible;
   if(!visible)return;
   lastHead.copy(head);local.copy(offset).applyQuaternion(rig.quaternion);
   mesh.position.set(head.x+local.x,rig.position.y+offset.y,head.z+local.z);
   mesh.lookAt(lastHead);mesh.updateMatrixWorld(true);
   metrics.x=head.x;metrics.y=rig.position.y;metrics.z=head.z;metrics.solids=world.stats.shapes;metrics.nodes=world.stats.nodes;
  },
  hide(){mesh.visible=false;},
  describe(){return {visible:mesh.visible,position:mesh.position.toArray(),offset:offset.toArray(),fps:state.fps,frameMs:state.frameMs,refreshes:state.refreshes};},
  dispose(){mesh.removeFromParent();mesh.geometry.dispose();material.dispose();texture.dispose();}
 };
}
