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
    scene.createCube([0,0.1,0]).setScale([1.2,1.5,1]).spin([0.25,0.8,0.1]);
    scene.createSphere([1.8,0.5,-1.2]).setScale([0.5,1,0.5]).spin([0,0,0.55]);
    scene.createLight({position:[-2.5,3,-1.5],color:[0.8,0.224,0.056]});
    scene.createLight({position:[2.5,2,2],color:[0.052,0.234,0.65]});
    const glass=scene.createPlane([0,0.5,-3]).setRotation([Math.PI/2,0,0]).setScale([5.5,1,3])
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,reflectivity:0.94});
    // Zaregistruje rovinu jako zrcadlo. Rovina, odražená kamera i render do
    // textury se počítají v Rustu. Pomocné značky/grid odrazový průchod vynechá.
    scene.setMirror(glass);
    /** @type {[Vector3,Vector3][]} */
    const frame=[
        [[0,2.08,-3],[5.8,0.16,0.18]], [[0,-1.08,-3],[5.8,0.16,0.18]],
        [[-2.83,0.5,-3],[0.16,3.3,0.18]], [[2.83,0.5,-3],[0.16,3.3,0.18]],
    ];
    for(const [position,scale] of frame) {
        scene.createCube(position).setScale(scale).setRenderOptions({isStatic:true});
    }
}
