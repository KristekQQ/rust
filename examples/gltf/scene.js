/** @param {import('../../js/engine.js').Scene} scene @param {'fox'|'helmet'} kind */
export async function createScene(scene, kind = 'fox') {
    scene.clear();
    const model = await scene.loadModel(new URL(kind === 'fox'
        ? '../../assets/models/fox/Fox.glb'
        : '../../assets/models/damaged-helmet/DamagedHelmet.glb', import.meta.url));
    if (kind === 'fox') model.setScale([0.035,0.035,0.035]).setRotation([0,Math.PI/2,0]);
    else model.setScale([2,2,2]).setRotation([0,Math.PI,0]).setPosition([0,2.1,0]);
    const floor = scene.createPlane([0,-0.02,0]).setScale([16,1,16])
        .setRenderOptions({isStatic:true,castsShadow:false});
    scene.createLight({position:[-3,6,-4],color:[1.6,1.3,1]});
    scene.createLight({position:[4,4,2],color:[0.4,0.6,0.85]});
    if (model.info.animations.length) model.playAnimation('Walk');
    return { model, floor };
}
