import { swordGeometry, fruitGeometries } from './geometry.js';
// Gentle hits only; fast attacks keep their full effect. Tune from the Quest URL (?hitSize=0.08) without rebuilding WASM.
const requestedHitSize=Number(new URLSearchParams(location.search).get('hitSize')??'0.15');
export const CUT_EFFECT_SIZE=Number.isFinite(requestedHitSize)&&requestedHitSize>=.01&&requestedHitSize<=2?requestedHitSize:.15;


/** Vlastní scéna v metrech. Černé okolí bez geometrické podlahy.
 * @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    const fruits = fruitGeometries();
    const geometry = fruits[0].geometry;
    const playOrder=['cherry','banana','grapes','orange','lime','watermelon','plum','lemon','pear'];
    const playFruits=[...fruits].sort((a,b)=>playOrder.indexOf(a.name)-playOrder.indexOf(b.name));
    const previewPositions = fruits.map((_,i)=>/** @type {import('../../js/engine.d.ts').Vector3} */ ([(i<5?i-2:i-5-1.5)*.47,i<5?1.42:.64,-1.55]));
    const apples = fruits.map((fruit,i)=>scene.createMesh(fruit.geometry).setPosition(previewPositions[i])
        .setScale([.17,.17,.17]).setEmission(1.1).setColorCutout(.25).spin([.1,.4,0])
        .setRenderOptions({castsShadow:false,receivesShadow:false}));
    const blade = swordGeometry();
    const sword = scene.createMesh(blade).setPosition([0.45, 1.1, -0.2])
        .setCutter({base:[0,0,-0.1],tip:[0,0,-0.95]}).setBladePhysics({edge:[0,-1,0]}).setCutAssist(.35).setBladeContactResponse(true).setBladeTrail(true);
    const leftSword = scene.createMesh(blade).setPosition([-0.45, 1.1, -0.2])
        .setCutter({base:[0,0,-0.1],tip:[0,0,-0.95]}).setBladePhysics({edge:[0,-1,0]}).setCutAssist(.35).setBladeContactResponse(true).setBladeTrail(true);
    scene.createLight({ position: [2, 5, 1], color: [1, 0.95, 0.9] });
    scene.createLight({ position: [-3, 3, -2], color: [0.25, 0.4, 0.7] });
    const [monument, scoreDisplay] = await Promise.all([
        scene.loadModel(new URL('./assets/codex-monument.gltf', import.meta.url)),
        scene.loadModel(new URL('./assets/score-display.gltf', import.meta.url)),
    ]);
    const monumentOrigin = /** @type {import('../../js/engine.d.ts').Vector3} */ ([1.65,0,-2.8]);
    const monumentScale = /** @type {import('../../js/engine.d.ts').Vector3} */ ([1.05,.6,1.05]);
    const monumentYaw = Math.atan2(-monumentOrigin[0],-monumentOrigin[2]);
    monument.setPosition(monumentOrigin).setRotation([0,monumentYaw,0]).setScale(monumentScale).setRenderOptions({isStatic:true,castsShadow:false});
    scoreDisplay.setPosition([0,1.97,-2.9]).setRenderOptions({isStatic:true,castsShadow:false});
    /** Keep the plaque and inscription on the stone's front surface, in metres.
     * @param {number} y @param {number} z @returns {import('../../js/engine.d.ts').Vector3} */
    const onStone=(y,z)=>[monumentOrigin[0]+Math.sin(monumentYaw)*z*monumentScale[2],y*monumentScale[1],monumentOrigin[2]+Math.cos(monumentYaw)*z*monumentScale[2]];
    const infoPanel = scene.createInfoPanel('diagnostics', {position:onStone(1.105,.248),width:1.08*monumentScale[0],height:.88*monumentScale[1]})
        .setRotation([0,monumentYaw,0]);
    const credits = scene.createTextPanel({title:'CODEX',lines:['POSTAVENO SPOLECNE','S HONZOU'],position:onStone(2.01,.228),width:1.08*monumentScale[0],height:.48*monumentScale[1]})
        .setRotation([0,monumentYaw,0]);
    const scorePanel = scene.createInfoPanel('round', {position:[0,1.97,-2.808],width:1.42,height:.65});
    const logo = scene.createTextPanel({title:'APOLLO GAMES',lines:[],position:[0,2.43,-2.808],width:2.20,height:.65});
    const restartButton = scene.createInfoPanel('restart', {position:[0,1.05,-1.65],width:.75,height:.22});
    /** @type {import("../../js/engine.d.ts").TriangleMeshOptions} */
    const pointerGeometry={positions:[0,1,0,0,-1,0,-1,0,0,1,0,0,0,0,1,0,0,-1],
        indices:[0,4,3,0,3,5,0,5,2,0,2,4,1,3,4,1,5,3,1,2,5,1,4,2],color:[.15,1,.35]};
    const pointers = ['left','right'].map(()=>scene.createMesh(pointerGeometry).setPosition([100,100,100]).setScale([.012,.012,.012])
        .setRenderOptions({castsShadow:false,receivesShadow:false}));
    return {apples, fruits, playFruits, previewPositions, sword, leftSword, geometry, monument, scoreDisplay, logo, credits, infoPanel, scorePanel, restartButton, pointers};
}

/** Vystřelení je příkaz; dráhu, rotaci, gravitaci a odstranění počítá Rust.
 * Start je ve výšce rukou: plod musí být v obou očích vidět již v prvním snímku.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit
 * @param {number} side */
export function launchApple(scene, fruit, side) {
    return scene.createMesh(fruit.geometry).setPosition([side*.38,1.3,-1.15]).setScale([.19,.19,.19])
        .setEmission(1.1).setColorCutout(.25).setRenderOptions({castsShadow:false,receivesShadow:false})
        .setCuttable(true,{capColor:cutFaceColor(fruit)})
        .projectile([-side*.18,3.2,0],{gravity:9.81,bounce:.55})
        .spin([.5,1.2,.3]).removeAfter(2.4);
}

/** Saturated visible flesh; exterior holes stay transparent.
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit
 * @returns {import('../../js/engine.d.ts').Vector3} */
export function cutFaceColor(fruit) {
    return [fruit.capColor[0]*.45,fruit.capColor[1]*.45,fruit.capColor[2]*.45];
}

/** A real desktop mesh split, using the same fruit and presentation as XR.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit @param {number} [size] */
export function previewSlice(scene,fruit,size=CUT_EFFECT_SIZE) {
    const source=scene.createMesh(fruit.geometry).setPosition([0,1.35,-1.15]).setScale([.25,.25,.25])
        .setEmission(1.1).setColorCutout(.25).setCuttable(true,{capColor:cutFaceColor(fruit)})
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    const cutter=scene.createCube([0,1.75,-.45]).setScale([.01,.01,.01])
        .setCutter({base:[0,0,-10],tip:[0,0,-95]});
    try {
        const report=scene.cutWith(cutter,{motion:[0,-.85,0],separation:.03});
        if(!report.removed.length)throw new Error('Náhledový řez neprotnul ovoce.');
        return {report,feedback:scene.playCutFeedback(report.pieces,{mirrorText:true,size})};
    } finally {cutter.remove();if(source.exists)source.remove();}
}
