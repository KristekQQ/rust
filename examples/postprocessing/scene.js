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
    // A light outside the side wall lets real windows shape the volumetric pass.
    scene.createLight({ position: [-5, 10, -9], color: [1, 0.8, 0.58] });
    scene.createLight({ position: [4, 6, 2], color: [0.25, 0.32, 0.45] });
    return { floor, architecture };
}
