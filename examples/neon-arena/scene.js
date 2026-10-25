import {VR_DEBUG_PANEL_WIDTH,VR_DEBUG_PANEL_HEIGHT} from '../shared/vr-debug-panel.js';
export const ARENA_REVISION='cynatar-arena-47';
import { swordGeometry, neonFrameGeometry, fruitGeometries, createFruitPatternCatalog, FRUIT_PATTERN_COUNT } from './geometry.js';
export const NEON_GLOW=Object.freeze({strength:.4,radius:.045});
export const PAYTABLE_ORIGIN=/** @type {import('../../js/engine.d.ts').Vector3} */([-2.75,1.2,-.40]);
export const PAYTABLE_YAW=Math.PI/2;
export const COMBO_DISPLAY_ORIGIN=/** @type {import('../../js/engine.d.ts').Vector3} */([0,1.42,-2.15]);
export const FRUIT_SCALE=.09; // Comfortable playable targets; gallery/UI scale is independent.
export const FRUIT_OPACITY=.06;
export const FRUIT_OPACITY_VARIATION=1;
export const RESTART_POSITION=/** @type {import('../../js/engine.d.ts').Vector3} */([0,1.20,.95]);
export const FLOOR_SSR=Object.freeze({strength:.92});
export const FLOOR_SSR_BOUNDS={min:/** @type {import('../../js/engine.d.ts').Vector3} */([-1.5,-.03,-3]),max:/** @type {import('../../js/engine.d.ts').Vector3} */([1.5,.03,.75])};
// Gentle hits only; fast attacks keep their full effect. Tune from the Quest URL (?hitSize=0.08) without rebuilding WASM.
const requestedHitSize=Number(new URLSearchParams(location.search).get('hitSize')??'0.15');
export const CUT_EFFECT_SIZE=Number.isFinite(requestedHitSize)&&requestedHitSize>=.01&&requestedHitSize<=2?requestedHitSize:.15;


/** Scene-scoped handles only; geometry, palettes and simulation belong to Rust.
 * @type {WeakMap<import('../../js/engine.js').Scene,WeakMap<object,{asset:import('../../js/engine.js').MeshVariants,index:number}>>} */
const fruitAssets=new WeakMap();
/** Use the shared asset when this fruit belongs to the scene's catalog.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit */
export function createFruitMesh(scene,fruit) {
    const recipe=fruitAssets.get(scene)?.get(fruit.geometry);
    return recipe?recipe.asset.createInstance(recipe.index):scene.createMesh(fruit.geometry);
}

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
    const {fruits,wild,variants:fruitPatterns}=createFruitPatternCatalog();
    const recipes=new WeakMap();
    fruitAssets.set(scene,recipes);
    for(const patterns of Object.values(fruitPatterns)) {
        const base=patterns[0].geometry;
        const asset=scene.createMeshVariants({positions:base.positions,indices:base.indices,
            variants:patterns.map(pattern=>pattern.geometry.triangleColors??[]),smooth:base.smooth});
        patterns.forEach((pattern,index)=>recipes.set(pattern.geometry,{asset,index}));
    }
    const geometry = fruits[0].geometry;
    const playOrder=['cherry','lime','lemon','orange','banana','plum','pear','grapes','watermelon'];
    // Catalog order separates similar fill masks, including the wraparound.
    // Per-kind cursors survive round restarts and visit all fifteen patterns.
    const patternCursors=new Map();
    /** @param {ReturnType<typeof fruitGeometries>[number]} fruit */
    const nextFruitVariant=fruit=>{
        const cursor=patternCursors.get(fruit.name)??0;
        patternCursors.set(fruit.name,(cursor+1)%FRUIT_PATTERN_COUNT);
        return fruitPatterns[fruit.name][cursor];
    };
    // Each readable wave has five matching fruit and a wildcard as its third target.
    const playFruits=playOrder.flatMap(name=>[0,1,2,3,4,5].map(slot=>{
        if(slot===2)return wild;
        const fruit=fruits.find(f=>f.name===name);
        if(!fruit)throw new Error(`Missing fruit: ${name}`);
        return fruit;
    }));
    // One shared authoring plane: upper edge leans away, face tilts toward the eyes.
    const boardTilt=-Math.PI/10;
    /** @param {number} x @param {number} y @param {number} [depth]
     * @returns {import('../../js/engine.d.ts').Vector3} */
    const onBoard=(x,y,depth=0)=>{
        const localZ=Math.sin(boardTilt)*(y-1.2)+Math.cos(boardTilt)*depth;
        return [PAYTABLE_ORIGIN[0]+Math.cos(PAYTABLE_YAW)*x+Math.sin(PAYTABLE_YAW)*localZ,
            PAYTABLE_ORIGIN[1]+Math.cos(boardTilt)*(y-1.2)-Math.sin(boardTilt)*depth,
            PAYTABLE_ORIGIN[2]-Math.sin(PAYTABLE_YAW)*x+Math.cos(PAYTABLE_YAW)*localZ];
    };
    const comboRules = scene.createTextPanel({title:'BONUS ZA TROJICI',lines:['WILD NAHRADI KAZDE OVOCE'],position:onBoard(0,2.36,-.05),width:1.65,height:.13}).setRotation([boardTilt,PAYTABLE_YAW,0]);
    const catalog=[...fruits].sort((a,b)=>FRUIT_POINTS[a.name].points-FRUIT_POINTS[b.name].points).concat(wild);
    const rowPositions=catalog.map((_,i)=>/** @type {import('../../js/engine.d.ts').Vector3} */ ([0,2.15-i*.215,-2.65]));
    const galleryPositions=catalog.flatMap((_,row)=>[-.46,-.16,.14].map(x=>
        /** @type {import('../../js/engine.d.ts').Vector3} */ (onBoard(x,rowPositions[row][1]))));
    const galleryPatterns=catalog.flatMap(fruit=>[0,1,2].map(pattern=>fruitPatterns[fruit.name][pattern]));
    const galleryIcons=catalog.flatMap((fruit,row)=>[0,1,2].map(slot=>createFruitMesh(scene,galleryPatterns[row*3+slot])
        .setPosition(galleryPositions[row*3+slot]).setRotation([boardTilt,PAYTABLE_YAW,0]).setScale([.073,.073,.073]).setColorCutout(.25).spin([0,.65,0])
        .setRenderOptions({castsShadow:false,receivesShadow:false})
        .bindComboIndicator(FRUIT_POINTS[fruit.name].label,/** @type {0|1|2} */(slot))));
    const galleryRows=catalog.map((fruit,i)=>({name:fruit.name,kind:FRUIT_POINTS[fruit.name].label,icons:galleryIcons.slice(i*3,i*3+3)}));
    const galleryLabels=catalog.map((fruit,i)=>{
        const [,y,z]=rowPositions[i],rule=FRUIT_POINTS[fruit.name];
        return scene.createTextPanel({title:String(rule.points),lines:[],position:onBoard(.57,y-.125,.04),width:.33,height:.44}).setRotation([boardTilt,PAYTABLE_YAW,0]);
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
    const frameAsset=scene.createMeshVariants({positions:frameGeometry.positions,indices:frameIndices,
        variants:[frameIndices.map((_,i)=>(frameGeometry.color??[0,0,0])[i%3])]});
    const galleryFrames=rowPositions.map(([x,y,z])=>frameAsset.createInstance()
        .setPosition(onBoard(x,y,-.08)).setRotation([boardTilt,PAYTABLE_YAW,0]).setEmission(1.4)
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false}));
    // One shared circular tube frame in metres, avoiding flattened cross-sections.
    const earnedFrameGeometry=neonFrameGeometry();
    const earnedFrameAsset=scene.createMeshVariants({positions:earnedFrameGeometry.positions,indices:earnedFrameGeometry.indices,
        variants:[earnedFrameGeometry.triangleColors??[]],smooth:true});
    const galleryEarnedFrames=catalog.flatMap((fruit,row)=>[0,1,2].map(slot=>earnedFrameAsset.createInstance()
        .setPosition(onBoard([-.46,-.16,.14][slot],rowPositions[row][1],-.015))
        .setRotation([boardTilt,PAYTABLE_YAW,0]).setScale([.62,.62,.62])
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
    const scoreDisplay=await scene.loadModel(new URL('./assets/score-display.gltf', import.meta.url));
    scoreDisplay.setPosition(onBoard(0,2.65,-.25)).setRotation([boardTilt,PAYTABLE_YAW,0]).setRenderOptions({isStatic:true,castsShadow:false});
    const infoPanel=scene.createInfoPanel('diagnostics',{position:[100,-10000,100],width:VR_DEBUG_PANEL_WIDTH,height:VR_DEBUG_PANEL_HEIGHT});
    const timerPanel=scene.createInfoPanel('countdown',{position:[0,2.36,-2.15],width:1.18,height:.50})
        .setEmission(8).setRenderOptions({castsShadow:false,receivesShadow:false});
    const scorePanel=scene.createInfoPanel('score',{position:[0,1.90,-2.15],width:1.55,height:.25})
        .setEmission(4).setRenderOptions({castsShadow:false,receivesShadow:false});
    const scoreLabel=scene.createTextPanel({title:'SKORE',lines:[],position:[0,2.075,-2.15],width:.50,height:.10})
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    const currentLabel=scene.createTextPanel({title:'DVA ZASOBNIKY',lines:['REZ +10 / SERIE = BONUS'],position:[0,1.07,-2.15],width:1.35,height:.17})
        .setEmission(1.2).setRenderOptions({castsShadow:false,receivesShadow:false});
    const awardPanels=[0,1,2].map(row=>scene.createInfoPanel(row<2?'debug-text':'combo-award',{
        position:[.90,COMBO_DISPLAY_ORIGIN[1]-(row+1)*.35,-2.15],width:row<2?.18:.55,height:row<2?.05:.18})
        .setRenderOptions({castsShadow:false,receivesShadow:false}));
    const stockScorePanels=[-.32,.32].map(x=>scene.createInfoPanel('combo-award',{position:[x,COMBO_DISPLAY_ORIGIN[1]-.22,COMBO_DISPLAY_ORIGIN[2]+.04],width:.28,height:.11})
        .setEmission(4).setRenderOptions({castsShadow:false,receivesShadow:false}));
    const comboDisplay={mode:/** @type {'chains'} */('chains'),symbols:[...fruits,wild].map(fruit=>({kind:FRUIT_POINTS[fruit.name].label,
        asset:recipes.get(fruit.geometry).asset.id,variant:0})),frameAsset:earnedFrameAsset.id,
        origin:COMBO_DISPLAY_ORIGIN,spacing:.34,rowGap:.35,fruitScale:.085,
        frameScale:/** @type {import('../../js/engine.d.ts').Vector3} */([1,1,1]),
        historySeconds:2,timerPanel,scorePanel,awardPanels,stockScorePanels};
    const logo = scene.createTextPanel({title:'APOLLO GAMES',lines:[],position:onBoard(0,3.11,-.158),width:2.20,height:.65}).setRotation([boardTilt,PAYTABLE_YAW,0]);
    const phasePanels=[['VOLBA','VYBER DRAZSI TROJICI'],['DOKONCI','DVA TED - TRETI ZA CHVILI'],['PRESNOST','MALY CIL - VELKA ODMENA'],['NACASOVANI','POCKEJ NA SROVNANI'],['SERIE','DRZ SVOJE DVA DRUHY'],['FINALE','DRZ DRUH A DOKONCI TROJICI']].map(([title,hint])=>({title,object:scene.createTextPanel({title,lines:[hint],position:[100,100,100],width:1.7,height:.23})
        .setEmission(2).setRenderOptions({castsShadow:false,receivesShadow:false})}));
    const restartButton = scene.createTextPanel({title:'START',lines:[],position:RESTART_POSITION,width:.50,height:.22})
        .setRotation([0,-Math.atan2(RESTART_POSITION[0],-RESTART_POSITION[2]),0]);
    /** @type {import("../../js/engine.d.ts").TriangleMeshOptions} */
    const pointerGeometry={positions:[0,1,0,0,-1,0,-1,0,0,1,0,0,0,0,1,0,0,-1],
        indices:[0,4,3,0,3,5,0,5,2,0,2,4,1,3,4,1,5,3,1,2,5,1,4,2],color:[.15,1,.35]};
    const pointers = ['left','right'].map(()=>scene.createMesh(pointerGeometry).setPosition([100,100,100]).setScale([.012,.012,.012])
        .setRenderOptions({castsShadow:false,receivesShadow:false}));
    // Shared launch glints: presentation only, with no projectile/collider proxy.
    const launchGlintAssets=[[.15,1,.75],[1,.25,.65],[1,.8,.2]].map(color=>scene.createMeshVariants({
        positions:pointerGeometry.positions,indices:pointerGeometry.indices,
        variants:[Array.from({length:8},()=>color).flat()]}));
    const cynatar=await scene.loadFont3D(new URL('../../assets/fonts/cynatar/glyphs.json',import.meta.url));
    const fontPanels=[timerPanel,scorePanel,awardPanels[2],...stockScorePanels];
    /** @type {'cynatar'|'neon'} */
    let selectedFont='cynatar';
    const arenaFont={
        get selected(){return selectedFont;},
        /** @param {'cynatar'|'neon'} value */
        set(value){
            if(value!=='cynatar'&&value!=='neon')throw new Error('Neznámý font arény.');
            fontPanels.forEach(panel=>panel.setFont3D(value==='cynatar'?cynatar:null,{color:[.02,.8,1],sideIntensity:.32}));
            selectedFont=value;
        },
    };
    arenaFont.set('cynatar');
    return {arenaFont,stockScorePanels,launchGlintAssets,phasePanels,scoreLabel,timerPanel,currentLabel,awardPanels,comboDisplay,floorTiles, floorGrout, apples, fruits, playFruits, fruitPatterns, nextFruitVariant, previewPositions, sword, leftSword, geometry, scoreDisplay, logo, infoPanel, scorePanel, comboRules, wild, boardTilt, galleryEarnedFrames, galleryRows, galleryIcons, galleryPatterns, galleryLabels, galleryFrames, galleryPositions, restartButton, pointers};
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
 * Authored minute launches start below the strike zone; Rust integrates the rise and fall.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit
 * @param {number} side @param {boolean} [solid]
 * @param {{position:import("../../js/engine.d.ts").Vector3,velocity:import("../../js/engine.d.ts").Vector3,gravity:number,lifetime:number,scale?:number}} [recipe] */
export function launchApple(scene, fruit, side, solid=true, recipe) {
    return createFruitMesh(scene,fruit).setPosition(recipe?.position??[side*.38,1.15,-1.10]).setScale([recipe?.scale??FRUIT_SCALE,recipe?.scale??FRUIT_SCALE,recipe?.scale??FRUIT_SCALE])
        .setEmission(2.4).setColorCutout(solid?0:.25).setOpacity(solid?FRUIT_OPACITY:1,{opaqueAbove:.25,variation:solid?FRUIT_OPACITY_VARIATION:0,coverage:fruit.coverage??.5}).setRenderOptions({castsShadow:false,receivesShadow:false})
        .setCuttable(true,{capColor:cutFaceColor(fruit,solid)})
        .setComboTarget(FRUIT_POINTS[fruit.name].label,FRUIT_POINTS[fruit.name].points,{wild:fruit.name==='wild'})
        .projectile(recipe?.velocity??[-side*.035,1.2,.08],{gravity:recipe?.gravity??1.6,bounce:0})
        .spin(recipe&&recipe.gravity>=2?[.55+Math.abs(recipe.velocity[0])*.8,.8+Math.abs(recipe.position[0])*.7,recipe.velocity[0]*1.1]:[.12,.45,.06])
        .removeAfter(recipe?.lifetime??1.9);
}

/** Open style discards dark caps; solid style renders the fruit's colored flesh.
 * Both styles keep closed CPU meshes for physics.
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit @param {boolean} [solid]
 * @returns {import('../../js/engine.d.ts').Vector3} */
export function cutFaceColor(fruit,solid=false) {
    return solid?[fruit.capColor[0]*.22,fruit.capColor[1]*.22,fruit.capColor[2]*.22]:[0,0,0];
}

/** A real desktop mesh split, using the same fruit and presentation as XR.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {ReturnType<typeof fruitGeometries>[number]} fruit @param {number} [size] @param {boolean} [solid] */
export function previewSlice(scene,fruit,size=CUT_EFFECT_SIZE,solid=true) {
    const source=createFruitMesh(scene,fruit).setPosition([0,1.35,-1.15]).setScale([.125,.125,.125])
        .setEmission(2.4).setColorCutout(solid?0:.25).setOpacity(solid?FRUIT_OPACITY:1,{opaqueAbove:.25,variation:solid?FRUIT_OPACITY_VARIATION:0,coverage:fruit.coverage??.5}).setCuttable(true,{capColor:cutFaceColor(fruit,solid)})
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    const cutter=scene.createCube([0,1.75,-.45]).setScale([.01,.01,.01])
        .setCutter({base:[0,0,-10],tip:[0,0,-95]});
    try {
        const report=scene.cutWith(cutter,{motion:[0,-.85,0],separation:.03});
        if(!report.removed.length)throw new Error('Náhledový řez neprotnul ovoce.');
        return {report,feedback:scene.playCutFeedback(report.pieces,{mirrorText:true,size})};
    } finally {cutter.remove();if(source.exists)source.remove();}
}
