// Shared static placement: the rear of the cabinet faces the player at the origin.
/** @param {import('../../js/engine.js').Scene} scene @param {URL} [url] */
export async function createArenaCabinet(scene,url=new URL('./assets/apollo-stellar.glb',import.meta.url)) {
 const cabinet=await scene.loadModel(url);
 cabinet.setPosition([2.1,0,-1.1]).setRotation([0,Math.atan2(2.1,-1.1),0]);
 cabinet.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,reflectivity:0,opaqueReorderable:true});
 return cabinet;
}
