// Tato funkce scénu sestaví jednou. Každý další frame obsluhuje Rust.
/** @typedef {import("../../js/engine.js").Vector3} Vector3 */
/** @param {import("../../js/engine.js").Scene} scene */
export function createScene(scene) {
    scene.clear();
    scene.createPlane([0,-1,0]).setScale([12,1,12])
        .setRenderOptions({isStatic:true,castsShadow:false});
    /** @type {Vector3[]} */
    const blocks=[[-2.3,-0.3,0],[2.2,-0.5,1.2],[-1.5,-0.5,-1.8]];
    for(const position of blocks) {
        scene.createCube(position).setScale([1,1,1]).setRenderOptions({isStatic:true});
    }
    // spin([x,y,z]) nastavuje rychlost rotace v radiánech za sekundu.
    scene.createCube([0,0.1,0]).setScale([1.2,1.5,1]).spin([0.25,0.8,0.1]);
    scene.createSphere([1.8,0.5,-1.2]).setScale([0.5,1,0.5]).spin([0,0,0.55]);
    scene.createLight({position:[-2.5,3,-1.5],color:[0.8,0.224,0.056]});
    scene.createLight({position:[2.5,2,2],color:[0.052,0.234,0.65]});
}
