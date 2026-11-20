/** @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    const cabinet=await scene.loadModel(new URL('./assets/apollo-stellar.glb?v=3',import.meta.url));
    cabinet.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});
    scene.createLight({position:[-3,5,4],color:[1.2,1.15,1.3]});
    scene.createLight({position:[3,3,-2],color:[.6,.4,.9]});
    scene.createLight({position:[0,4,-4],color:[.8,.85,1]});
    return {cabinet};
}
