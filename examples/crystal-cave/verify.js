import {capturePixels,assertForeground,assertChanged} from '../../tests/pixels.js';
/** @param {import('../../js/engine.js').Engine} engine @param {Awaited<ReturnType<typeof import('./scene.js').createScene>>} world @param {HTMLCanvasElement} canvas */
export async function verifyScene(engine,world,canvas){
    const info=world.environment.info;
    if(!world.environment.exists||info.primitives<2||info.primitives>8)throw new Error('Jeskynní GLB chybí nebo překračuje rozpočet 8 materiálových částí.');
    if(!engine.xr.player.state.locomotion.enabled)throw new Error('VR pohyb není aktivní.');
    const {landingPads:pads,walkableBounds:b,obstacles}=world.layout;
    if(pads.length<6)throw new Error('Chybí průchozí trasa jeskyní.');
    for(const pad of pads){
        const [x,y,z]=pad.position;
        if(y!==0||x<b[0]+.4||x>b[1]-.4||z<b[2]+.4||z>b[3]-.4||obstacles.some(({center:c,halfExtents:h})=>Math.abs(x-c[0])<=h[0]+.4&&Math.abs(z-c[2])<=h[2]+.4))throw new Error(`Neprůchozí bod: ${pad.id}`);
    }
    try{
        engine.setCameraView([0,1.7,1],[0,3,-18]);const entrance=await capturePixels(canvas);assertForeground(entrance,'cave entrance',5000);
        engine.setCameraView([0,1.7,-20],[0,3,-31]);const heart=await capturePixels(canvas);assertForeground(heart,'crystal chamber',5000);assertChanged(entrance,heart,'depth traversal',4000);
        engine.setCameraView([0,1.7,-12],[0,9,-15]);const roof=await capturePixels(canvas);assertForeground(roof,'enclosing rock roof',5000);assertChanged(heart,roof,'actual rock ceiling',4000);
    }finally{engine.setCameraView([0,1.7,1],[0,3,-18]);}
}
