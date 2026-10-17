/** @param {import('../../js/engine.js').Scene} scene */
export function createScene(scene) {
    scene.clear();
    scene.createPlane([0,0,0]).setScale([14,1,14]);
    // Two ground launchers: the projectile pool is fired from these fixed mouths.
    scene.createCube([-3,.12,0]).setScale([1.1,.18,.8]);
    scene.createCube([3,.12,0]).setScale([1.1,.18,.8]);
    scene.createLight({position:[3,7,5],color:[1.2,1.1,.9]});
    scene.createLight({position:[-4,3,-2],color:[.25,.45,1]});
}
