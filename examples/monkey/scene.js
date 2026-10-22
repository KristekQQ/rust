/** @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    const monkey = await scene.loadModel(new URL('../../assets/models/monkey/monkey.glb', import.meta.url));
    monkey.playAnimation('Hello', {loop: true});
    scene.createLight({position: [-3, 5, 4], color: [1.1, 0.95, 0.78]});
    scene.createLight({position: [3, 3, 1], color: [0.55, 0.73, 0.85]});
    scene.createLight({position: [0, 4, -4], color: [0.7, 1, 0.75]});
    return {monkey};
}
