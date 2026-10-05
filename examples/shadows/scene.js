import { floor, staticCube, coloredLights } from "../shared/scene-helpers.js";

// Tato funkce scénu sestaví jednou. Každý další frame obsluhuje Rust.
/** @typedef {import("../../js/engine.js").Vector3} Vector3 */
/** @param {import("../../js/engine.js").Scene} scene */
export function createScene(scene) {
    scene.clear();
    floor(scene);
    /** @type {Vector3[]} */
    const blocks=[[-2.3,-0.3,0],[2.2,-0.5,1.2],[-1.5,-0.5,-1.8]];
    for(const position of blocks) staticCube(scene,position);
    // spin([x,y,z]) nastavuje rychlost rotace v radiánech za sekundu.
    scene.createCube([0,0.1,0]).setScale([1.2,1.5,1]).spin([0.25,0.8,0.1]);
    scene.createSphere([1.8,0.5,-1.2]).setScale([0.5,1,0.5]).spin([0,0,0.55]);
    coloredLights(scene);
}
