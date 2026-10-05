// Sdílené příkazy pro podlahu, bloky a světla; bez UI a bez frame smyčky.
/** @typedef {import("../../js/engine.js").Vector3} Vector3 */

/** @param {import("../../js/engine.js").Scene} scene
 * @param {Vector3} position
 * @param {Vector3} [scale] */
export function staticCube(scene, position, scale=[1,1,1]) {
    return scene.createCube(position).setScale(scale).setRenderOptions({isStatic:true});
}

/** @param {import("../../js/engine.js").Scene} scene */
export function floor(scene) {
    scene.createPlane([0,-1,0]).setScale([12,1,12])
        .setRenderOptions({isStatic:true,castsShadow:false});
}

/** @param {import("../../js/engine.js").Scene} scene */
export function coloredLights(scene) {
    scene.createLight({position:[-2.5,3,-1.5],color:[0.8,0.224,0.056]});
    scene.createLight({position:[2.5,2,2],color:[0.052,0.234,0.65]});
}
