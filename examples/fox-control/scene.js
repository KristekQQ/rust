/** @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();

    const ground = createGround(scene);
    const platforms = createPlatforms(scene);
    const wall = createSolidBox(scene, [-5, 1.5, 7], [1, 3, 8]);
    const blocks = [
        createSolidBox(scene, [5, 0.6, 4], [2, 1.2, 2]),
        createSolidBox(scene, [6, 1, 9], [3, 2, 3]),
    ];
    createLights(scene);
    const fox = await createFox(scene);

    return { fox, ground, platforms, wall, blocks };
}

/** @param {import('../../js/engine.js').Scene} scene */
function createGround(scene) {
    const ground = scene.createPlane([0, -0.02, 0]);
    ground.setScale([80, 1, 80]);
    ground.setRenderOptions({ isStatic: true, castsShadow: false });
    // Tenký pevný box má horní hranu přesně na y=0.
    ground.setCollider({ halfExtents: [0.5, 0.02, 0.5] });
    return ground;
}

/** @param {import('../../js/engine.js').Scene} scene */
function createPlatforms(scene) {
    // Každá další plošina je o 0,6 m vyšší. Mezery vyžadují nový skok.
    /** @type {{position: import('../../js/engine.js').Vector3, size: import('../../js/engine.js').Vector3}[]} */
    const steps = [
        { position: [0, 0.3, 3], size: [2.6, 0.6, 2] },
        { position: [0, 0.6, 5.7], size: [2.6, 1.2, 2] },
        { position: [0, 0.9, 8.4], size: [2.6, 1.8, 2] },
        { position: [0, 1.2, 11.5], size: [5, 2.4, 3] },
    ];
    return steps.map(step => createSolidBox(scene, step.position, step.size));
}

/** @param {import('../../js/engine.js').Scene} scene
 * @param {import('../../js/engine.js').Vector3} position
 * @param {import('../../js/engine.js').Vector3} size */
function createSolidBox(scene, position, size) {
    const box = scene.createCube(position);
    box.setScale(size);
    box.setRenderOptions({ isStatic: true });
    box.setCollider(); // Výchozí jednotkový box se škáluje stejně jako objekt.
    return box;
}

/** @param {import('../../js/engine.js').Scene} scene */
function createLights(scene) {
    scene.createLight({ position: [-4, 8, -3], color: [1.4, 1.2, 1] });
    scene.createLight({ position: [5, 5, 7], color: [0.4, 0.6, 0.85] });
}

/** @param {import('../../js/engine.js').Scene} scene */
async function createFox(scene) {
    const url = new URL('../../assets/models/fox/Fox.glb', import.meta.url);
    const fox = await scene.loadModel(url);
    fox.setScale([0.025, 0.025, 0.025]);
    fox.setPosition([0, 0, 0]);
    return fox;
}
