// Scene recipes are JavaScript commands. Rust owns the resulting scene and simulation.
// No JS animation loop, physics, camera matrices or visibility lists are needed.
/** @typedef {import("../js/engine.js").Vector3} Vector3 */
/** @param {import("../js/engine.js").Scene} scene */
export function basic(scene) {
    scene.clear();
    scene.createPlane([0,-0.6,0]).setScale([8,1,8]);
    scene.createCube();
    scene.createSphere([1.8,0.4,0.2]).setScale([0.6,0.6,0.6]);
    scene.createLight({position:[2.5,3,2.5],color:[1,1,1]});
    scene.createLight({position:[-2,1.5,-1.5],color:[0.2,0.6,1]});
}

/** @param {import("../js/engine.js").Scene} scene
 * @param {Vector3} position
 * @param {Vector3} scale */
function staticCube(scene, position, scale=[1,1,1]) {
    return scene.createCube(position).setScale(scale).setRenderOptions({isStatic:true});
}
/** @param {import("../js/engine.js").Scene} scene */
function floor(scene) {
    scene.createPlane([0,-1,0]).setScale([12,1,12])
        .setRenderOptions({isStatic:true,castsShadow:false});
}
/** @param {import("../js/engine.js").Scene} scene */
function coloredLights(scene) {
    scene.createLight({position:[-2.5,3,-1.5],color:[0.8,0.224,0.056]});
    scene.createLight({position:[2.5,2,2],color:[0.052,0.234,0.65]});
}
/** @param {import("../js/engine.js").Scene} scene */
export function shadows(scene) {
    scene.clear();
    floor(scene);
    /** @type {Vector3[]} */
    const blocks=[[-2.3,-0.3,0],[2.2,-0.5,1.2],[-1.5,-0.5,-1.8]];
    for(const position of blocks) staticCube(scene,position);
    scene.createCube([0,0.1,0]).setScale([1.2,1.5,1]).spin([0.25,0.8,0.1]);
    scene.createSphere([1.8,0.5,-1.2]).setScale([0.5,1,0.5]).spin([0,0,0.55]);
    coloredLights(scene);
}
/** @param {import("../js/engine.js").Scene} scene */
export function mirror(scene) {
    shadows(scene);
    const glass=scene.createPlane([0,0.5,-3]).setRotation([Math.PI/2,0,0]).setScale([5.5,1,3])
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,reflectivity:0.94});
    scene.setMirror(glass);
    /** @type {[Vector3,Vector3][]} */
    const frame=[
        [[0,2.08,-3],[5.8,0.16,0.18]], [[0,-1.08,-3],[5.8,0.16,0.18]],
        [[-2.83,0.5,-3],[0.16,3.3,0.18]], [[2.83,0.5,-3],[0.16,3.3,0.18]],
    ];
    for(const [position,scale] of frame) staticCube(scene,position,scale);
}
/** @param {import("../js/engine.js").Scene} scene */
export function animatedLights(scene) {
    scene.clear();
    floor(scene);
    /** @type {Vector3[]} */
    const blocks=[[-1.8,-0.5,0],[0,-0.5,-1.8],[1.8,-0.5,0]];
    for(const position of blocks) staticCube(scene,position);
    scene.createSphere([0,0.2,0.5]).setScale([0.7,1.1,0.7]);
    // JS sends orbit parameters once; Rust moves the lights and invalidates their maps.
    scene.createLight({color:[0.8,0.22,0.05]}).orbit({radius:3,height:2.6,speed:0.55,color:[0.8,0.22,0.05]});
    scene.createLight({color:[0.05,0.23,0.65]}).orbit({radius:3,height:2.2,speed:-0.4,phase:Math.PI,color:[0.05,0.23,0.65]});
}
/** @param {import("../js/engine.js").Scene} scene */
export function culling(scene) {
    scene.clear();
    // This is a scene description, not a JS culling implementation.
    for(let x=0;x<20;x++) for(let y=0;y<25;y++) for(let z=0;z<20;z++) {
        scene.createCube([(x-9.5)*3,(y-12)*1.8,(z-9.5)*3]).setScale([0.65,0.65,0.65]);
    }
    /** @type {Vector3[]} */
    const lightPositions=[[-20,20,-20],[20,20,20],[-20,-20,20],[20,-20,-20]];
    for(const position of lightPositions) {
        scene.createLight({position,color:[0.35,0.35,0.35]});
    }
}
export const examples = [
    {id:'basic',title:'01 · Základní scéna',description:'Krychle, koule, rovina a dvě světla. Definice scény je v JS.',build:basic,shadows:false,mirror:false,grid:true,helpers:false,camera:'orbit'},
    {id:'shadows',title:'02 · Dva barevné stíny',description:'Statické bloky a dva rotující objekty. Každé světlo má vlastní statickou a dynamickou mapu.',build:shadows,shadows:true,mirror:false,grid:false,helpers:true,camera:'orbit'},
    {id:'mirror',title:'03 · Zrcadlo a stíny',description:'Odraz do textury, vlastní frustum odrazu a dvě stínující světla. Pomůcky se neodrážejí.',build:mirror,shadows:true,mirror:true,grid:false,helpers:true,camera:'orbit'},
    {id:'lights',title:'04 · Pohyblivá světla',description:'JS nastaví orbitu jedním příkazem; Rust počítá pohyb a aktualizuje stíny světel.',build:animatedLights,shadows:true,mirror:false,grid:false,helpers:true,camera:'orbit'},
    {id:'culling',title:'05 · 10 000 krychlí',description:'Proleť prostor kamerou. Rust provádí culling a dávkuje viditelné krychle. WASD, Q/E, Shift.',build:culling,shadows:false,mirror:false,grid:false,helpers:false,camera:'free'},
];
