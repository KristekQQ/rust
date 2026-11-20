/** Original Apollo species, bounded ambient passes; all simulation stays in Rust. */
export const BIRD_FLIGHT_ASSET='./assets/slot-bird-flight.gltf';
export const BIRD_ATLAS=Object.freeze({columns:4,rows:6,frames:4});
export const BIRD_SPECIES=Object.freeze([
    {name:'Sup',asset:BIRD_FLIGHT_ASSET,fps:15},
    {name:'Modrý pták',asset:'./assets/slot-bird-blue.gltf',fps:6},
    {name:'Kachna',asset:'./assets/slot-bird-duck.gltf',fps:6},
    {name:'Zelený pták',asset:'./assets/slot-bird-green.gltf',fps:6},
    {name:'Kohout',asset:'./assets/slot-bird-cock.gltf',fps:6},
    {name:'Pistáciový pták',asset:'./assets/slot-bird-pistachio.gltf',fps:6},
]);
// Six semantic bands alias four physical rows: front, side, rear, underside.
export const BIRD_VIEWS=Object.freeze({
    directionalView:{frames:4,frontStartFrame:0,frontObliqueStartFrame:4,sideStartFrame:8,rearObliqueStartFrame:12,rearStartFrame:16},
    elevatedView:{startFrame:20,elevation:-35,maxFrontPitch:25,pitch:-65},
    atlasBands:[0,1,1,2,2,3].map((row,i)=>({firstFrame:i*4,frameCount:4,columns:4,origin:[0,row*.25],tileSize:[.25,.25]})),
});
// Preserve all original vulture FLY frames (including its elastic neck).
const VULTURE_LAYOUT={columns:8,rows:12,frames:16,
 directionalView:{frames:16,frontStartFrame:0,frontObliqueStartFrame:16,sideStartFrame:32,rearObliqueStartFrame:48,rearStartFrame:64},
 elevatedView:{startFrame:80,elevation:-35,maxFrontPitch:25,pitch:-65},
 atlasBands:[{"firstFrame":0,"frameCount":16,"columns":4,"origin":[0,0],"tileSize":[0.25,0.12628255722178375]},{"firstFrame":16,"frameCount":16,"columns":4,"origin":[0,0.6288476716653513],"tileSize":[0.244921875,0.12371744277821625],"tileMap":[0,0,1,1,2,2,3,3]},{"firstFrame":32,"frameCount":16,"columns":4,"origin":[0,0.6288476716653513],"tileSize":[0.244921875,0.12371744277821625],"tileMap":[0,0,1,1,2,2,3,3]},{"firstFrame":48,"frameCount":16,"columns":4,"origin":[0,0.7525651144435674],"tileSize":[0.244921875,0.12371744277821625],"tileMap":[0,0,1,1,2,2,3,3]},{"firstFrame":64,"frameCount":16,"columns":4,"origin":[0,0.7525651144435674],"tileSize":[0.244921875,0.12371744277821625],"tileMap":[0,0,1,1,2,2,3,3]},{"firstFrame":80,"frameCount":16,"columns":4,"origin":[0,0.8762825572217837],"tileSize":[0.244921875,0.12371744277821625],"tileMap":[0,0,1,1,2,2,3,3]}]
};
// Reference formation in meters at the player crossing. Scale both lanes and sway.
const FLOCK_WIDTH_SCALE = .75;
const FLOCK_LANES = [
    [0, -.62, 1.27, -2.30, .68, 2.15],
    [-1.35, .60, -.68, 2.30, 0, -2.10],
    [2.25, -.66, 0, .65, -2.20, 1.40],
];
const FLYOVER_START_Z = -24;
const FLYOVER_DISTANCE = 30;
const PLAYER_CROSSING = -FLYOVER_START_Z / FLYOVER_DISTANCE;
// One vanishing point, three staggered rows of all six species.
// The spread develops towards the player; flight is along -Z -> +Z, never lateral.
export const BIRD_FLIGHTS=Object.freeze(Array.from({length:18},(_,i)=>{
    const species=BIRD_SPECIES[i%6],row=Math.floor(i/6);
    const endX=FLOCK_LANES[row][i%6]*FLOCK_WIDTH_SCALE/PLAYER_CROSSING;
    const height=.1+((i*7)%5)*.12;
    return {...species,position:[0,3.25,FLYOVER_START_Z],scale:.6+(i%3)*.06,
        config:{key:2+i,...BIRD_ATLAS,...BIRD_VIEWS,...(i%6===0?VULTURE_LAYOUT:{}),fps:species.fps*(.9+(i%4)*.07),duration:12,
            swordTarget:{points:100,radius:.48,reachHeight:.36+(i%3)*.055},
            cutEffectKey:900+i%6,groundDrop:{interval:6,floorY:.012},
            ambientDelay:1+row*1.9+(i%6)*.23,ambientInterval:12,flockJitter:3,cooldown:0,
            travel:[endX,height,FLYOVER_DISTANCE],travelControl:[endX*.5,height*.5,FLYOVER_DISTANCE*.5],
            travelSway:{amplitude:[(.18+(i%4)*.045)*FLOCK_WIDTH_SCALE,.08+(i%3)*.025,0],cycles:1.8+(i%5)*.23,phase:i*2.399963},
            oneWay:true,flipWithTravel:false}};
}));
/** Reuse on the observer via bindSprite(key); do not run a second flight clock. */
export const FEATHER_SPECIES=['vulture','blue','duck','green','cock','pistachio'];
/** @param {import('../../js/engine.js').Scene} scene */
export async function loadFeatherTemplates(scene) {
    for(const [i,name] of FEATHER_SPECIES.entries()) {
        const model=await scene.loadModel(new URL('./assets/slot-bird-feathers-'+name+'.gltf',import.meta.url));
        model.configureSprite({key:900+i,columns:8,rows:5,frames:40,fps:30,duration:40/30,
            ambientInterval:0,cooldown:0,oneWay:true,travel:[0,-.12,0]});
    }
}
/** @param {import('../../js/engine.js').Scene} scene */
export async function createBirdFlights(scene) {
    await loadFeatherTemplates(scene);
    const birds=[],templates=new Map();
    for(const {asset,position,scale,config} of BIRD_FLIGHTS) {
        const template=templates.get(asset);
        const bird=template?.createInstance ? template.createInstance() : await scene.loadModel(new URL(asset,import.meta.url));
        if(!template)templates.set(asset,bird);
        bird.setPosition(position).setScale([scale,scale,scale])
            .setRenderOptions({castsShadow:false,receivesShadow:false,reflectivity:0})
            .configureSprite(config);
        birds.push(bird);
    }
    return birds;
}
