/** @param {import('../../js/engine.js').Scene} scene */
export function createScene(scene) {
    scene.clear();
    // SSR uses this surface's reflectivity, not a registered planar mirror.
    const floor = scene.createPlane([0, -1, 0]).setScale([18, 1, 18]);
    floor.setRenderOptions({ isStatic: true, castsShadow: false, reflectivity: 0.85 });
    scene.createCube([-1.3, 0, 0]).spin([0, 0.35, 0]);
    scene.createSphere([1.6, 0, -1]).setScale([0.9, 0.9, 0.9]);
    // First light is the screen-space shaft source. Slats occlude it in depth.
    scene.createLight({ position: [-2, 2.4, -4.5], color: [1, 0.7, 0.3] });
    scene.createLight({ position: [3, 2, 3], color: [0.15, 0.3, 0.65] });
    for (let x = -4; x <= 4; x += 0.8) {
        scene.createCube([x, 2.2, -3.5]).setScale([0.18, 3.6, 0.35])
            .setRenderOptions({ isStatic: true });
    }
    return { floor };
}
