
// Tato funkce scénu sestaví jednou. Každý další frame obsluhuje Rust.
/** @typedef {import("../../js/engine.js").Vector3} Vector3 */
/** @param {import("../../js/engine.js").Scene} scene */
export function createScene(scene) {
    scene.clear(); // Odstraní předchozí objekty, světla a jejich naplánované akce.
    scene.createPlane([0,-0.6,0]).setScale([8,1,8]);
    scene.createCube();
    scene.createSphere([1.8,0.4,0.2]).setScale([0.6,0.6,0.6]);
    scene.createLight({position:[2.5,3,2.5],color:[1,1,1]});
    scene.createLight({position:[-2,1.5,-1.5],color:[0.2,0.6,1]});
}
