/** Optional, deterministic static drone load for renderer stress measurements. */
export const MAX_STRESS_MODELS = 2048;
export const STRESS_LEVELS = Object.freeze([0, 16, 32, 64, 128, 256, 512, 1024, 2048]);

const FOV_RADIANS = 65 * Math.PI / 180;
const GRID_SIZE = 8;
const LAYER_DEPTH = 1.25;

/** Deterministic gate-camera position for a stress drone index.
 * @param {number} index
 * @returns {import('../../js/engine.js').Vector3} */
export function stressPosition(index) {
    if (!Number.isInteger(index) || index < 0 || index >= MAX_STRESS_MODELS) {
        throw new RangeError(`Index modelu musí být celé číslo od 0 do ${MAX_STRESS_MODELS - 1}.`);
    }
    const layer = Math.floor(index / (GRID_SIZE * GRID_SIZE));
    const slot = index % (GRID_SIZE * GRID_SIZE);
    const column = slot % GRID_SIZE;
    const row = Math.floor(slot / GRID_SIZE);
    const depth = 4 + layer * LAYER_DEPTH;
    const halfHeight = depth * Math.tan(FOV_RADIANS / 2);
    const halfWidth = halfHeight * 16 / 9;
    const spanX = halfWidth * .72;
    const spanY = halfHeight * .58;
    const centerY = Math.max(.35 + spanY, 1.7);
    return [
        ((column + .5) / GRID_SIZE * 2 - 1) * spanX,
        centerY + ((row + .5) / GRID_SIZE * 2 - 1) * spanY,
        42 - depth,
    ];
}

/**
 * Create an opt-in, static load from the already-loaded shared drone asset.
 * Instances fill stable 8x8 tiles that expand with camera distance.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {Awaited<ReturnType<typeof import('./scene.js').createScene>>} content
 */
export function createStressLoad(scene, content) {
    const source = content.characters.find(character => character.spec.slug === 'drone-orbiter');
    if (!source) throw new Error('Chybí původní model drone-orbiter.');
    const droneModel = source.model;
    const droneScale = source.spec.root.scale;

    /** @type {import('../../js/engine.js').ModelObject[]} */
    const objects = [];
    let disposed = false;

    /** @param {number} index
     * @returns {{position: import('../../js/engine.js').Vector3,scale: import('../../js/engine.js').Vector3}} */
    function transform(index) {
        return {
            position: stressPosition(index),
            scale: droneScale,
        };
    }

    /** @param {number} count */
    function setCount(count) {
        if (disposed) throw new Error('Stress load je již ukončen.');
        if (!Number.isInteger(count) || count < 0 || count > MAX_STRESS_MODELS) {
            throw new RangeError(`Počet modelů musí být celé číslo od 0 do ${MAX_STRESS_MODELS}.`);
        }

        if (count > objects.length) {
            /** @type {import('../../js/engine.js').ModelObject[]} */
            const pending = [];
            try {
                for (let index = objects.length; index < count; index++) {
                    const pose = transform(index);
                    const model = droneModel.createInstance();
                    pending.push(model);
                    model.setPosition(pose.position)
                        .setScale(pose.scale)
                        .playAnimation('Idle', {loop: true})
                        .pauseAnimation()
                        .seekAnimation(0)
                        .setRenderOptions({castsShadow: false, receivesShadow: true, isStatic: true, opaqueReorderable: true});
                }
            } catch (error) {
                for (const model of pending) {
                    try { model.remove(); } catch { /* Preserve the creation failure. */ }
                }
                throw error;
            }
            try {
                scene.rimInput({stress_count: count});
            } catch (error) {
                for (const model of pending) {
                    try { model.remove(); } catch { /* Preserve the command failure. */ }
                }
                throw error;
            }
            objects.push(...pending);
        } else if (count < objects.length) {
            scene.rimInput({stress_count: count});
            while (objects.length > count) objects.pop()?.remove();
            return;
        } else {
            scene.rimInput({stress_count: count});
        }
    }

    function dispose() {
        if (disposed) return;
        disposed = true;
        while (objects.length) {
            try { objects.pop()?.remove(); } catch { /* Continue cleaning owned handles. */ }
        }
        try { scene.rimInput({stress_count: 0}); } catch { /* Disposal is best-effort after cleanup. */ }
    }

    return {
        setCount,
        get count() { return objects.length; },
        get objects() { return objects.slice(); },
        dispose,
    };
}
