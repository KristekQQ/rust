import { createScene as createShadowScene } from "../shadows/scene.js";
import { staticCube } from "../shared/scene-helpers.js";

// Tato funkce scénu sestaví jednou. Každý další frame obsluhuje Rust.
/** @typedef {import("../../js/engine.js").Vector3} Vector3 */
/** @param {import("../../js/engine.js").Scene} scene */
export function createScene(scene) {
    createShadowScene(scene); // Rozšíříme scénu z předchozí samostatné ukázky.
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
    for(const [position,scale] of frame) staticCube(scene,position,scale);
}
