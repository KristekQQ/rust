import { swordGeometry, fruitGeometries, wildSymbol } from './geometry.js';
export const NEON_GLOW=Object.freeze({strength:1.6,radius:.045});
export const FLOOR_SSR=Object.freeze({strength:.92});
export const FLOOR_SSR_BOUNDS={min:/** @type {import('../../js/engine.d.ts').Vector3} */([-1.5,-.03,-3]),max:/** @type {import('../../js/engine.d.ts').Vector3} */([1.5,.03,.75])};
// Gentle hits only; fast attacks keep their full effect. Tune from the Quest URL (?hitSize=0.08) without rebuilding WASM.
const requestedHitSize=Number(new URLSearchParams(location.search).get('hitSize')??'0.15');
export const CUT_EFFECT_SIZE=Number.isFinite(requestedHitSize)&&requestedHitSize>=.01&&requestedHitSize<=2?requestedHitSize:.15;


/** Vlastní scéna v metrech. Černé okolí a lesklá kachličková podlaha.
 * @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    // One static draw for 80 individually bevelled tiles, with unreflective grout below.
    const tilePositions=[],tileIndices=[],tileColors=[];
    for(let row=0;row<10;row++)for(let col=0;col<8;col++){
        const x=(col-3.5)*.75,z=1.875-row*.75,b=tilePositions.length/3;
        for(const [half,y] of [[.363,0],[.371,-.012]])
            tilePositions.push(x-half,y,z-half,x+half,y,z-half,x+half,y,z+half,x-half,y,z+half);
        tileIndices.push(b,b+2,b+1,b,b+3,b+2);
        const shade=(row+col)%2===0?[.052,.073,.10]:[.032,.047,.070];
        tileColors.push(...shade,...shade);
        for(let side=0;side<4;side++){
            const next=(side+1)%4;
            tileIndices.push(b+side,b+next,b+4+next,b+side,b+4+next,b+4+side);
            tileColors.push(.021,.033,.047,.021,.033,.047);
        }
        tileIndices.push(b+4,b+5,b+6,b+4,b+6,b+7);
        tileColors.push(.012,.018,.025,.012,.018,.025);
    }
    const floorTiles=scene.createMesh({positions:tilePositions,indices:tileIndices,triangleColors:tileColors})
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,reflectivity:.94});
    const floorGrout=scene.createMesh({positions:[0,-.025].flatMap(y=>[-3,y,-5.25,3,y,-5.25,3,y,2.25,-3,y,2.25]),
        indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,1,2,6,1,6,5,2,3,7,2,7,6,3,0,4,3,4,7],color:[.006,.012,.020]})
        .setPosition([0,-.025,0])
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});
    const fruits = fruitGeometries();
    const geometry = fruits[0].geometry;
    const playOrder=['cherry','lime','lemon','orange','banana','plum','pear','grapes','watermelon'];
    const wild=wildSymbol();
    // Each readable wave has five matching fruit and a wildcard as its third target.
    const playFruits=playOrder.flatMap(name=>{
        const fruit=fruits.find(f=>f.name===name);
        if(!fruit)throw new Error(`Missing fruit: ${name}`);
        return [fruit,fruit,wild,fruit,fruit,fruit];
    });
    // One shared authoring plane: upper edge leans away, face tilts toward the eyes.
    const boardTilt=-Math.PI/10;
    /** @param {number} x @param {number} y @param {number} [depth]
     * @returns {import('../../js/engine.d.ts').Vector3} */
    const onBoard=(x,y,depth=0)=>[x,1.2+Math.cos(boardTilt)*(y-1.2)-Math.sin(boardTilt)*depth,
        -2.65+Math.sin(boardTilt)*(y-1.2)+Math.cos(boardTilt)*depth];
    const comboRules = scene.createTextPanel({title:'3 STEJNE - BODY',lines:['WILD NAHRADI KAZDE OVOCE'],position:onBoard(0,2.36,-.05),width:1.65,height:.13}).setRotation([boardTilt,0,0]);
    const catalog=[...fruits].sort((a,b)=>FRUIT_POINTS[a.name].points-FRUIT_POINTS[b.name].points).concat(wild);
    const rowPositions=catalog.map((_,i)=>/** @type {import('../../js/engine.d.ts').Vector3} */ ([0,2.15-i*.215,-2.65]));
    const galleryPositions=catalog.flatMap((_,row)=>[-.46,-.16,.14].map(x=>
        /** @type {import('../../js/engine.d.ts').Vector3} */ (onBoard(x,rowPositions[row][1]))));
    const galleryIcons=catalog.flatMap((fruit,row)=>[0,1,2].map(slot=>scene.createMesh(fruit.geometry)
        .setPosition(galleryPositions[row*3+slot]).setRotation([boardTilt,0,0]).setScale([.073,.073,.073]).setColorCutout(.25).spin([0,.65,0])
        .setRenderOptions({castsShadow:false,receivesShadow:false})
        .bindComboIndicator(FRUIT_POINTS[fruit.name].label,/** @type {0|1|2} */(slot))));
    const galleryRows=catalog.map((fruit,i)=>({name:fruit.name,kind:FRUIT_POINTS[fruit.name].label,icons:galleryIcons.slice(i*3,i*3+3)}));
    const galleryLabels=catalog.map((fruit,i)=>{
        const [,y,z]=rowPositions[i],rule=FRUIT_POINTS[fruit.name];
        return scene.createTextPanel({title:String(rule.points),lines:[],position:onBoard(.57,y-.125,.04),width:.33,height:.44}).setRotation([boardTilt,0,0]);
    });
    // Rectangular neon rings frame each rotating sculpture without obscuring it.
    const ring=[-.82,-.10,.82,-.10,.82,.10,-.82,.10,-.812,-.092,.812,-.092,.812,.092,-.812,.092];
    const front=[0,1,5,0,5,4,1,2,6,1,6,5,2,3,7,2,7,6,3,0,4,3,4,7];
    const frameIndices=[...front,...Array.from({length:front.length/3},(_,i)=>[front[i*3]+8,front[i*3+2]+8,front[i*3+1]+8]).flat()];
    /** @type {import("../../js/engine.d.ts").TriangleMeshOptions} */
    const frameGeometry={positions:[.012,-.012].flatMap(z=>Array.from({length:8},(_,i)=>[ring[i*2],ring[i*2+1],z]).flat()),
        indices:frameIndices,color:[.08,.65,.85]};
    for(let i=0;i<4;i++){
        const n=(i+1)%4;
        frameIndices.push(i,n+8,n,i,i+8,n+8,4+i,4+n,12+n,4+i,12+n,12+i);
    }
    const galleryFrames=rowPositions.map(([x,y,z])=>scene.createMesh(frameGeometry)
        .setPosition(onBoard(x,y,-.08)).setRotation([boardTilt,0,0]).setEmission(1.4)
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false}));
    const earnedFrameGeometry={...frameGeometry,color:/** @type {import('../../js/engine.d.ts').Vector3} */ ([.5,.30,.015])};
    const galleryEarnedFrames=catalog.flatMap((fruit,row)=>[0,1,2].map(slot=>scene.createMesh(earnedFrameGeometry)
        .setPosition(onBoard([-.46,-.16,.14][slot],rowPositions[row][1],-.015))
        .setRotation([boardTilt,0,0]).setScale([.128,.75,1])
        .setRenderOptions({castsShadow:false,receivesShadow:false})
        .bindComboIndicator(FRUIT_POINTS[fruit.name].label,/** @type {0|1|2} */(slot))));
    // Existing preview/verification handles refer to the actual gallery fruit.
    const apples=fruits.map(fruit=>galleryIcons[catalog.indexOf(fruit)*3]);
    const previewPositions=fruits.map(fruit=>galleryPositions[catalog.indexOf(fruit)*3]);
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
    const monumentOrigin = /** @type {import('../../js/engine.d.ts').Vector3} */ ([3.0,0,-2.8]);
    const monumentScale = /** @type {import('../../js/engine.d.ts').Vector3} */ ([1.05,.6,1.05]);
    const monumentYaw = Math.atan2(-monumentOrigin[0],-monumentOrigin[2]);
    monument.setPosition(monumentOrigin).setRotation([0,monumentYaw,0]).setScale(monumentScale).setRenderOptions({isStatic:true,castsShadow:false});
    scoreDisplay.setPosition(onBoard(0,2.65,-.25)).setRotation([boardTilt,0,0]).setRenderOptions({isStatic:true,castsShadow:false});
    /** Keep the plaque and inscription on the stone's front surface, in metres.
     * @param {number} y @param {number} z @returns {import('../../js/engine.d.ts').Vector3} */
    const onStone=(y,z)=>[monumentOrigin[0]+Math.sin(monumentYaw)*z*monumentScale[2],y*monumentScale[1],monumentOrigin[2]+Math.cos(monumentYaw)*z*monumentScale[2]];
    const infoPanel = scene.createInfoPanel('diagnostics', {position:onStone(1.105,.248),width:1.08*monumentScale[0],height:.88*monumentScale[1]})
        .setRotation([0,monumentYaw,0]);
    const credits = scene.createTextPanel({title:'CODEX',lines:['POSTAVENO SPOLECNE','S HONZOU'],position:onStone(2.01,.228),width:1.08*monumentScale[0],height:.48*monumentScale[1]})
        .setRotation([0,monumentYaw,0]);
    const scorePanel = scene.createInfoPanel('round', {position:onBoard(0,2.65,-.158),width:1.42,height:.65}).setRotation([boardTilt,0,0]);
    const logo = scene.createTextPanel({title:'APOLLO GAMES',lines:[],position:onBoard(0,3.11,-.158),width:2.20,height:.65}).setRotation([boardTilt,0,0]);
    const restartButton = scene.createInfoPanel('restart', {position:[0,.20,-1.65],width:.75,height:.22});
    /** @type {import("../../js/engine.d.ts").TriangleMeshOptions} */
    const pointerGeometry={positions:[0,1,0,0,-1,0,-1,0,0,1,0,0,0,0,1,0,0,-1],
        indices:[0,4,3,0,3,5,0,5,2,0,2,4,1,3,4,1,5,3,1,2,5,1,4,2],color:[.15,1,.35]};
    const pointers = ['left','right'].map(()=>scene.createMesh(pointerGeometry).setPosition([100,100,100]).setScale([.012,.012,.012])
        .setRenderOptions({castsShadow:false,receivesShadow:false}));
    return {floorTiles, floorGrout, apples, fruits, playFruits, previewPositions, sword, leftSword, geometry, monument, scoreDisplay, logo, credits, infoPanel, scorePanel, comboRules, wild, boardTilt, galleryEarnedFrames, galleryRows, galleryIcons, galleryLabels, galleryFrames, galleryPositions, restartButton, pointers};
}

// Points per completed triple; adapted ranking, not the slot's cash payouts.
export const FRUIT_POINTS = /** @type {Record<string, {label:string,points:number}>} */ ({
    cherry:{label:'TRESEN',points:10}, lime:{label:'LIMETKA',points:15},
    lemon:{label:'CITRON',points:20}, orange:{label:'POMERANC',points:25},
    banana:{label:'BANAN',points:40}, plum:{label:'SVESTKA',points:50},
    pear:{label:'HRUSKA',points:60}, grapes:{label:'HROZNY',points:100},
    watermelon:{label:'MELOUN',points:120}, wild:{label:'ZOLIK',points:120},
});

/** Vystřelení je příkaz; dráhu, rotaci, gravitaci a odstranění počítá Rust.
 * Start je ve výšce rukou: plod musí být v obou očích vidět již v prvním snímku.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit
 * @param {number} side */
export function launchApple(scene, fruit, side) {
    return scene.createMesh(fruit.geometry).setPosition([side*.38,1.15,-1.10]).setScale([.21,.21,.21])
        .setEmission(2.4).setColorCutout(.25).setRenderOptions({castsShadow:false,receivesShadow:false})
        .setCuttable(true,{capColor:cutFaceColor(fruit)})
        .setComboTarget(FRUIT_POINTS[fruit.name].label,FRUIT_POINTS[fruit.name].points,{wild:fruit.name==='wild'})
        .projectile([-side*.035,1.2,.08],{gravity:1.6,bounce:0})
        .spin([.12,.45,.06]).removeAfter(1.9);
}

/** Neon fruit has no visible cut filling. The existing .25 color cutout
 * discards these caps before color/depth writes; CPU physics keeps a closed solid.
 * @param {ReturnType<typeof fruitGeometries>[number]} _fruit
 * @returns {import('../../js/engine.d.ts').Vector3} */
export function cutFaceColor(_fruit) {
    return [0,0,0];
}

/** A real desktop mesh split, using the same fruit and presentation as XR.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit @param {number} [size] */
export function previewSlice(scene,fruit,size=CUT_EFFECT_SIZE) {
    const source=scene.createMesh(fruit.geometry).setPosition([0,1.35,-1.15]).setScale([.25,.25,.25])
        .setEmission(2.4).setColorCutout(.25).setCuttable(true,{capColor:cutFaceColor(fruit)})
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    const cutter=scene.createCube([0,1.75,-.45]).setScale([.01,.01,.01])
        .setCutter({base:[0,0,-10],tip:[0,0,-95]});
    try {
        const report=scene.cutWith(cutter,{motion:[0,-.85,0],separation:.03});
        if(!report.removed.length)throw new Error('Náhledový řez neprotnul ovoce.');
        return {report,feedback:scene.playCutFeedback(report.pieces,{mirrorText:true,size})};
    } finally {cutter.remove();if(source.exists)source.remove();}
}
