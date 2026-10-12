/** @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    const floor = scene.createPlane([0, -1, 0]).setScale([18, 1, 18]);
    floor.setRenderOptions({ isStatic: true, castsShadow: false, reflectivity: 0.85 });

    // JS authors the scene; Rust parses glTF, computes bounds and renders its mesh.
    const bridge = await scene.loadModel(new URL(
        '../../assets/models/kenney-nature-bridge-stone-round/bridge_stone_round.glb', import.meta.url));
    bridge.setPosition([-2 * Math.SQRT1_2, -0.6, -2 * Math.SQRT1_2])
        .setScale([6, 8, 4]).setRotation([0, Math.PI / 4, 0])
        .setRenderOptions({ isStatic: true });
    /** @param {import('../../js/engine.js').Vector3} position
     * @param {import('../../js/engine.js').Vector3} scale */
    const stone = (position, scale) => scene.createSphere(position).setScale(scale)
        .setRenderOptions({ isStatic: true });
    const axis = Math.SQRT1_2;
    stone([-3.7, -0.3, -2.5], [1.1, 0.8, 1.2]);
    stone([3.6, -0.35, -2.2], [1.2, 0.7, 0.9]);
    stone([-1.5, -0.35, 0.3], [0.8, 0.65, 0.9]);
    scene.createSphere([1.5, -0.15, -0.5]).setScale([0.8, 0.85, 0.6]).spin([0, 0.2, 0]);

    // First light behind the passage: its real shadow maps shape the fog beams.
    scene.createLight({ position: [-4.8 * axis, 0.6, -4.8 * axis], color: [1, 0.76, 0.46] });
    scene.createLight({ position: [3, 2, 3], color: [0.12, 0.2, 0.32] });
    return { floor, bridge };
}
