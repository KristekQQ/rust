/** @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    const robot=await scene.loadModel(new URL('../../assets/models/neon-robot/neon-robot.glb',import.meta.url));
    robot.setScale([-1,1,1]);
    robot.playAnimation('Idle',{loop:true});
    await scene.loadModel(new URL('../../assets/models/neon-robot/studio-stage.glb',import.meta.url));
    scene.createLight({position:[-3,5,5],color:[3.4,2.7,3.1]});
    scene.createLight({position:[4,3,1],color:[.25,1.4,3.0]});
    scene.createLight({position:[-3,2,-3],color:[3.2,.12,1.5]});
    return {robot};
}
