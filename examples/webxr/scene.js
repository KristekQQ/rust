import {VR_DEBUG_PANEL_WIDTH,VR_DEBUG_PANEL_HEIGHT} from '../shared/vr-debug-panel.js';
import { appleGeometry, swordGeometry } from './geometry.js';

/** Vlastní scéna v metrech. Žádná runtime závislost na jiném příkladu.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {{practice?: boolean}} [options] */
export function createScene(scene, {practice = false} = {}) {
    scene.clear();
    scene.createPlane([0, -0.02, -1]).setScale([10, 1, 10]).setRenderOptions({isStatic: true});
    if (practice) scene.createCube([0, 0.8, -0.9]).setScale([1.15, 0.06, 0.55]).setRenderOptions({isStatic: true});
    scene.createCube([-0.65, 0.06, -1.2]).setScale([0.32, 0.12, 0.32]);
    scene.createCube([0.65, 0.06, -1.2]).setScale([0.32, 0.12, 0.32]);
    const geometry = appleGeometry(20, 12);
    const apples = (practice ? [-0.25, 0.25] : []).map(x => scene.createMesh(geometry).setPosition([x, 0.8, -0.9]).setScale([0.15,0.15,0.15]).setCuttable(true,{capColor:[1,0.83,0.46]}));
    const blade = swordGeometry();
    const sword = scene.createMesh(blade).setPosition([0.45, 1.1, -0.2])
        .setCutter({base:[0,0,-0.1],tip:[0,0,-0.95]}).setBladePhysics({edge:[0,-1,0]});
    const leftSword = scene.createMesh(blade).setPosition([-0.45, 1.1, -0.2])
        .setCutter({base:[0,0,-0.1],tip:[0,0,-0.95]}).setBladePhysics({edge:[0,-1,0]});
    scene.createLight({ position: [2, 5, 1], color: [1, 0.95, 0.9] });
    scene.createLight({ position: [-3, 3, -2], color: [0.25, 0.4, 0.7] });
    const infoPanel = scene.createInfoPanel('diagnostics', {position:[100,-10000,100],width:VR_DEBUG_PANEL_WIDTH,height:VR_DEBUG_PANEL_HEIGHT});
    const scorePanel = scene.createInfoPanel('round', {position:[.68,2.05,-2.3],width:1.2,height:.72});
    const restartButton = scene.createInfoPanel('restart', {position:[0,1.05,-1.65],width:.75,height:.22});
    /** @type {import("../../js/engine.d.ts").TriangleMeshOptions} */
    const pointerGeometry={positions:[0,1,0,0,-1,0,-1,0,0,1,0,0,0,0,1,0,0,-1],
        indices:[0,4,3,0,3,5,0,5,2,0,2,4,1,3,4,1,5,3,1,2,5,1,4,2],color:[.15,1,.35]};
    const pointers = ['left','right'].map(()=>scene.createMesh(pointerGeometry).setPosition([100,100,100]).setScale([.012,.012,.012])
        .setRenderOptions({castsShadow:false,receivesShadow:false}));
    return {apples, sword, leftSword, geometry, infoPanel, scorePanel, restartButton, pointers};
}

/** Vystřelení je příkaz; dráhu, rotaci, gravitaci a odstranění počítá Rust.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {import('../../js/engine.d.ts').TriangleMeshOptions} geometry
 * @param {number} side */
export function launchApple(scene, geometry, side) {
    return scene.createMesh(geometry).setPosition([side*0.65,0.13,-1.2]).setScale([0.13,0.13,0.13])
        .setCuttable(true,{capColor:[1,0.83,0.46]})
        .projectile([-side*0.4,4.6,0],{gravity:8,bounce:0})
        .spin([0.5,1.2,0.3]).removeAfter(3);
}
