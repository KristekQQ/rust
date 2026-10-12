/** @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    // Real downloaded architecture; offline OBJ -> GLB conversion keeps geometry only.
    // The Rust importer owns vertices, normals, bounds, visibility and shadow rendering.
    const architecture = await scene.loadModel(new URL(
        '../../assets/models/sibenik-cathedral/sibenik-geometry.glb', import.meta.url));
    architecture.setScale([0.8, 0.8, 0.8]).setPosition([0, 11.2498456, 0])
        .setRenderOptions({ isStatic: true });

    // Optional wet-floor surface lies just above the model's existing floor.
    const floor = scene.createPlane([0, -0.98, 0]).setScale([25, 1, 12]);
    floor.setRenderOptions({ isStatic: true, castsShadow: false, reflectivity: 0.85 });
    // A light outside the rose window lets its real stone tracery split the light.
    scene.createLight({ position: [-19, 12, 0.2], color: [1.4, 1.12, 0.812] });
    scene.createLight({ position: [7, 4, 1], color: [0.055, 0.075, 0.11] });
    return { floor, architecture };
}
