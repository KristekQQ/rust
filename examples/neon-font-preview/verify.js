import {capturePixels,assertChanged} from '../../tests/pixels.js';
/** @param {import('../../js/engine.d.ts').Engine} engine
 * @param {HTMLCanvasElement} canvas
 * @param {{strength:number,radius:number}} glow */
export async function verifyFont(engine,canvas,glow) {
    const original=engine.cameraViewState();
    /** @type {import('../../js/engine.d.ts').SceneObject | undefined} */
    let probe;
    try {
        const on=await capturePixels(canvas);
        const font=engine.debugStats().numericFont;
        if(font.glyphMeshes!==10||font.liveGlyphs!==10||font.panels!==2)throw new Error(`Invalid font cache: ${JSON.stringify(font)}`);
        engine.setNeonGlow({strength:0});const off=await capturePixels(canvas);
        assertChanged(off,on,'all digits with actual glow',100);
        // The emissive core must stay bright along an oblique horizontal tube,
        // rather than disappearing as a specular highlight would.
        probe=engine.scene.createNumericText('3',{position:[10,1.5,0],width:1.2,height:.8})
            .setScale([-1,1,1]).setEmission(5);
        engine.setOrbitView([12.1,1.7,3],[10,1.5,0]);
        const pixels=await capturePixels(canvas),mesh=probe.meshSnapshot;
        const frontVertices=mesh.positions.filter((_,i)=>mesh.normals[i][2]>.999);
        const top=Math.max(...frontVertices.map(p=>p[1]));
        const topBar=frontVertices.filter(p=>Math.abs(p[1]-top)<1e-5);
        const left=Math.min(...topBar.map(p=>p[0])),right=Math.max(...topBar.map(p=>p[0]));
        const zs=mesh.positions.map(p=>p[2]);
        const radius=(Math.max(...zs)-Math.min(...zs))*.5;
        for(const t of [.3,.5,.7]){
            const x=10-(left+(right-left)*t),y=1.5+top;
            const center=engine.worldToScreen([x,y,.003]);
            const rim=engine.worldToScreen([x,y+radius,.003]);
            if(!center||!rim)throw new Error('Oblique core probe outside view');
            const px=Math.round(center[0]*canvas.width),py=Math.round(center[1]*canvas.height);
            const reach=Math.ceil(Math.abs(rim[1]-center[1])*canvas.height)+3;
            let peak=0;
            for(let dy=-reach;dy<=reach;dy++)for(let dx=-5;dx<=5;dx++){
                const sx=px+dx,sy=py+dy;
                if(sx>=0&&sy>=0&&sx<canvas.width&&sy<canvas.height){
                    const i=(sy*canvas.width+sx)*4;
                    peak=Math.max(peak,Math.min(pixels.data[i],pixels.data[i+1],pixels.data[i+2]));
                }
            }
            if(peak<180)throw new Error(`Neon core fades along an oblique tube at ${t}: ${peak}`);
        }
    } finally {
        probe?.remove();engine.setNeonGlow(glow);
        if(original.orbitTarget)engine.setOrbitView(original.position,original.orbitTarget);
    }
}
