import {capturePixels,assertForeground,assertChanged} from '../../tests/pixels.js';
/** @param {import('../../js/engine.js').Engine} engine @param {Awaited<ReturnType<typeof import('./scene.js').createScene>>} world @param {HTMLCanvasElement} canvas */
export async function verifyScene(engine,world,canvas){
    if(!world.environment.exists||world.environment.info.primitives<4||world.environment.info.primitives>40)throw new Error('Chybí městský GLB nebo překročil limit vykreslovaných částí.');
    if(world.enemies.length!==3||world.enemies.some(m=>!m.exists))throw new Error('Tři monstra musí být načtená.');
    if(engine.scene.meleeState&&world.enemies.filter(enemy=>enemy.info.playback?.playing).length!==1)throw new Error('Současně musí být vidět právě jeden protivník.');
    if(engine.effects_stats().shadowLightCount!==0)throw new Error('Quest arena must not render four dynamic shadow maps.');
    if(!engine.xr.player.state.locomotion.enabled)throw new Error('Pohyb není aktivován.');
    const {landingPads:pads,walkableBounds:b,obstacles}=world.layout;
    if(pads.length<8)throw new Error('Chybí síť teleportových míst.');
    for(const pad of pads){
        const [x,y,z]=pad.position;
        if(y!==0||x<b[0]||x>b[1]||z<b[2]||z>b[3]||obstacles.some(({center:c,halfExtents:h})=>Math.abs(x-c[0])<=h[0]+.4&&Math.abs(z-c[2])<=h[2]+.4))throw new Error(`Neprůchozí místo: ${pad.id}`);
    }
    const reached=new Set([0]);let grew=true;
    while(grew){grew=false;for(const i of [...reached])for(const [j,pad]of pads.entries())if(!reached.has(j)&&Math.hypot(pads[i].position[0]-pad.position[0],pads[i].position[2]-pad.position[2])<=10){reached.add(j);grew=true;}}
    if(reached.size!==pads.length)throw new Error('Teleportová místa nejsou propojená do 10m.');
    try{
        engine.setCameraView([0,1.85,6],[0,5,-18]);const street=await capturePixels(canvas);assertForeground(street,'cyberpunk street',2000);
        engine.setOrbitView([0,23,18],[0,4,-15]);const overview=await capturePixels(canvas);assertForeground(overview,'city overview',2000);assertChanged(street,overview,'real 3D architecture',2000);
        engine.setCameraView([-7,1.8,-14],[2,3,-20]);const side=await capturePixels(canvas);assertForeground(side,'side route',2000);assertChanged(street,side,'side lane perspective',2000);
    }finally{engine.setCameraView([0,1.85,6],[0,5,-18]);}
}
