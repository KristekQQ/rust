import {capturePixels,assertForeground,assertChanged} from '../../tests/pixels.js';
/** @param {import('./scene.js').ArenaLayout} layout */
export function navigationGraph(layout){
    const {landingPads:pads,walkableBounds:b,obstacles}=layout;
    /** @param {number[]} position */
    const clear=([x,,z])=>x>=b[0]+.4&&x<=b[1]-.4&&z>=b[2]+.4&&z<=b[3]-.4&&!obstacles.some(({center:c,halfExtents:h})=>Math.abs(x-c[0])<=h[0]+.42&&Math.abs(z-c[2])<=h[2]+.42);
    for(const pad of pads)if(!clear(pad.position)||pad.position[1]!==0)throw new Error(`Neprůchozí místo: ${pad.id}`);
    /** @type {number[][]} */
    const graph=pads.map(()=>[]);
    for(let i=0;i<pads.length;i++)for(let j=i+1;j<pads.length;j++){
        const a=pads[i].position,b=pads[j].position,d=Math.hypot(a[0]-b[0],a[2]-b[2]);
        if(d>7)continue;
        let valid=true;
        for(let t=0;t<=1;t+=.025)if(!clear(a.map((v,k)=>v+(b[k]-v)*t))){valid=false;break;}
        if(valid){graph[i].push(j);graph[j].push(i);}
    }
    const reached=new Set([0]),queue=[0];
    while(queue.length)for(const next of graph[/** @type {number} */(queue.shift())])if(!reached.has(next)){reached.add(next);queue.push(next);}
    if(reached.size!==pads.length)throw new Error(`Průchozí síť: ${reached.size}/${pads.length}`);
    return graph;
}
/** @param {import('../../js/engine.js').Engine} engine @param {Awaited<ReturnType<typeof import('./scene.js').createScene>>} world @param {HTMLCanvasElement} canvas */
export async function verifyScene(engine,world,canvas){
    if(!world.environment.exists||world.environment.info.primitives<4||world.environment.info.primitives>40)throw new Error('Chybí lesní GLB nebo překročil limit částí.');
    if(world.enemies.length<3||world.enemies.some(m=>!m.exists||!m.info.playback?.playing))throw new Error('Lesní tvorové musí být načtení a animovaní.');
    if(new Set(world.layout.modelSpawns.map(s=>s.slug)).size!==3)throw new Error('Chybí tři odlišné druhy tvorů.');
    if(!engine.xr.player.state.locomotion.enabled)throw new Error('Pohyb není aktivován.');
    const b=world.layout.walkableBounds;
    if((b[1]-b[0])*(b[3]-b[2])<11000)throw new Error('Aréna musí být výrazně větší než město.');
    navigationGraph(world.layout);
    try{
        engine.setCameraView([0,2.1,4],[0,5,-30]);const clearing=await capturePixels(canvas);assertForeground(clearing,'forest clearing',2000);
        engine.setOrbitView([72,85,92],[0,0,-20]);const overview=await capturePixels(canvas);assertForeground(overview,'forest overview',2000);assertChanged(clearing,overview,'real large woodland geometry',2000);
        engine.setCameraView([8,2,-52],[0,4,-62]);const ruins=await capturePixels(canvas);assertForeground(ruins,'ruins',2000);assertChanged(clearing,ruins,'remote forest location',2000);
    }finally{engine.setCameraView([0,2.1,4],[0,5,-30]);}
}
