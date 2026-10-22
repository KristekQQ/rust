/** @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    await scene.loadModel(new URL('../../assets/models/neon-enemies/studio-stage.glb',import.meta.url));
    scene.createLight({position:[-3,5,5],color:[2.8,3.1,3.5]});
    scene.createLight({position:[4,2,1],color:[.18,.8,1.9]});
    scene.createLight({position:[-3,3,-3],color:[.6,.08,.3]});
}
/** @param {import('../../js/engine.js').Scene} scene @param {string} slug @param {string} color */
export async function loadEnemy(scene,slug,color) {
    const model=await scene.loadModel(new URL(`../../assets/models/neon-enemies/${slug}-${color}.glb`,import.meta.url));
    model.setScale([-1,1,1]);
    return model;
}
