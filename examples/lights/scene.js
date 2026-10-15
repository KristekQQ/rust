// Tato funkce scénu sestaví jednou. Každý další frame obsluhuje Rust.
/** @typedef {import("../../js/engine.js").Vector3} Vector3 */
/** @param {import("../../js/engine.js").Scene} scene */
export function createScene(scene) {
    scene.clear();
    scene.createPlane([0,-1,0]).setScale([12,1,12])
        .setRenderOptions({isStatic:true,castsShadow:false});
    /** @type {Vector3[]} */
    const blocks=[[-1.8,-0.5,0],[0,-0.5,-1.8],[1.8,-0.5,0]];
    for(const position of blocks) {
        scene.createCube(position).setScale([1,1,1]).setRenderOptions({isStatic:true});
    }
    scene.createSphere([0,0.2,0.5]).setScale([0.7,1.1,0.7]);
    // radius = poloměr, height = výška, speed = radiány/s, phase = počáteční úhel.
    // Záporná speed obrátí směr. Rust při pohybu aktualizuje světelné stínové mapy.
    scene.createLight({color:[0.8,0.22,0.05]}).orbit({radius:3,height:2.6,speed:0.55,color:[0.8,0.22,0.05]});
    scene.createLight({color:[0.05,0.23,0.65]}).orbit({radius:3,height:2.2,speed:-0.4,phase:Math.PI,color:[0.05,0.23,0.65]});
}
