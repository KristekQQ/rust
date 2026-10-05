
// Tato funkce scénu sestaví jednou. Každý další frame obsluhuje Rust.
/** @typedef {import("../../js/engine.js").Vector3} Vector3 */
/** @param {import("../../js/engine.js").Scene} scene */
export function createScene(scene) {
    scene.clear();
    // Tyto tři cykly sestaví scénu JEDNOU. Při každém snímku Rust už jen
    // vybere viditelné krychle podle kamery a odešle je jako instanční dávku.
    for (let x = 0; x < 20; x++) {
        for (let y = 0; y < 25; y++) {
            for (let z = 0; z < 20; z++) {
                const position = /** @type {Vector3} */ ([(x - 9.5) * 3, (y - 12) * 1.8, (z - 9.5) * 3]);
                scene.createCube(position).setScale([0.65, 0.65, 0.65]);
            }
        }
    }
    /** @type {Vector3[]} */
    const lightPositions=[[-20,20,-20],[20,20,20],[-20,-20,20],[20,-20,-20]];
    for(const position of lightPositions) {
        scene.createLight({position,color:[0.35,0.35,0.35]});
    }
}
