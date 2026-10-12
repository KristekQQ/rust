/** @param {import('../../js/engine.js').Scene} scene */
export function createScene(scene) {
    scene.clear();
    const floor = scene.createPlane([0, -1, 0]).setScale([18, 1, 18]);
    floor.setRenderOptions({ isStatic: true, castsShadow: false, reflectivity: 0.85 });

    // Neutral stone geometry instead of repetitive colored slats.
    // These are ordinary SDK objects; shadowing and volumetric light are Rust/WGSL.
    /** @param {import('../../js/engine.js').Vector3} position
     * @param {import('../../js/engine.js').Vector3} scale */
    const stone = (position, scale) => scene.createSphere(position).setScale(scale)
        .setRenderOptions({ isStatic: true });
    // Rotate the passage as a whole to face the default diagonal orbit view.
    // These coordinates are authored once; camera and light calculations stay in Rust.
    const axis = Math.SQRT1_2;
    /** @param {number} x @param {number} y @param {number} z
     * @param {import('../../js/engine.js').Vector3} scale */
    const gateStone = (x, y, z, scale) => stone([(x + z) * axis, y, (z - x) * axis], scale)
        .setRotation([0, Math.PI / 4, 0]);
    gateStone(-2, 0.6, -2.8, [0.65, 1.6, 0.6]);
    gateStone(2, 0.6, -2.8, [0.7, 1.6, 0.6]);
    gateStone(0, 2.4, -2.8, [2.7, 0.4, 0.6]);
    // A broken lintel splits the light, instead of making uniform stripes.
    gateStone(-1.2, 0.9, -2.8, [1.1, 0.25, 0.55]).setRotation([0, Math.PI / 4, -0.45]);
    stone([-3.7, -0.3, -2.5], [1.1, 0.8, 1.2]);
    stone([3.6, -0.35, -2.2], [1.2, 0.7, 0.9]);
    stone([-1.5, -0.35, 0.3], [0.8, 0.65, 0.9]);
    scene.createSphere([1.5, -0.15, -0.5]).setScale([0.8, 0.85, 0.6]).spin([0, 0.2, 0]);

    // First light behind the passage: its real shadow maps shape the fog beams.
    scene.createLight({ position: [(-1.4 - 4.8) * axis, 2, (-4.8 + 1.4) * axis], color: [1, 0.76, 0.46] });
    scene.createLight({ position: [3, 2, 3], color: [0.12, 0.2, 0.32] });
    return { floor };
}
