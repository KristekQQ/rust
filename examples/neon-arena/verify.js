import {NEON_GLOW,FLOOR_SSR} from './scene.js';
/** @param {import('../../js/engine.d.ts').Engine} engine
 * @param {Awaited<ReturnType<typeof import('./scene.js').createScene>>} world
 * @param {HTMLCanvasElement} canvas */
export async function verifyArena(engine, world, canvas) {
    const {capturePixels,assertForeground,assertChanged} = await import('../../tests/pixels.js');
    const wait=async()=>{for(let i=0;i<5;i++)await new Promise(requestAnimationFrame);};
    const originalView=engine.cameraViewState();
    engine.set_simulation_paused(true);
    /** @type {import('../../js/engine.js').SceneObject | undefined} */
    let marker;
    /** @type {import('../../js/engine.js').SceneObject[]} */
    const alphaMarkers=[];
    try {
        await wait();
        const image=await capturePixels(canvas);
        assertForeground(image,'fruit and world bitmap panels');
        let black=0;
        for(let i=0;i<image.data.length;i+=4)if(image.data[i]<3&&image.data[i+1]<3&&image.data[i+2]<3)black++;
        if(black<image.width*image.height*.25)throw new Error('Background is not black');
        const stats=engine.postprocessing.stats();
        if(stats.passes!==2||stats.targets===0)throw new Error('Tiled floor SSR is not active');
        if(performance.getEntriesByType('resource').some(r=>/\/(?:color|emission)-(?:px|nx|py|ny|pz|nz)\.png/.test(r.name)))throw new Error('Environment images were loaded');
        if(!engine.debugStats().neonGlow.enabled||engine.debugStats().neonGlow.draws===0)throw new Error('Neon edge glow is not being drawn');
        // Compare only the floor under a real fruit: moving the source must move
        // its SSR contribution independently of the unchanged tile geometry.
        const reflectedFruit=world.fruits.find(fruit=>fruit.name==='watermelon')??world.fruits[0];
        marker=engine.scene.createMesh(reflectedFruit.geometry).setPosition([-.55,.65,-1.35])
            .setScale([.28,.28,.28]).setEmission(2.4).setColorCutout(.25);
        engine.setOrbitView([0,1.5,2.4],[0,.5,-1.35]);
        await wait();const reflectedA=await capturePixels(canvas);
        engine.postprocessing.disable('ssr');await wait();const plainA=await capturePixels(canvas);
        marker.setPosition([.55,.65,-1.35]);await wait();const plainB=await capturePixels(canvas);
        engine.postprocessing.enable('ssr',FLOOR_SSR);await wait();const reflectedB=await capturePixels(canvas);
        const corners=[[-1,-1.02,-1.35],[-.1,-.28,-1.35]].map(p=>engine.worldToScreen(/** @type {[number,number,number]} */(p)));
        if(!corners[0]||!corners[1])throw new Error('Floor reflection projection unavailable');
        const a=[corners[0][0]*canvas.width,corners[0][1]*canvas.height];
        const b=[corners[1][0]*canvas.width,corners[1][1]*canvas.height];
        let reflectedPixels=0,movedPixels=0;
        for(let y=Math.max(0,Math.floor(Math.min(a[1],b[1])));y<Math.min(canvas.height,Math.ceil(Math.max(a[1],b[1])));y++)
            for(let x=Math.max(0,Math.floor(Math.min(a[0],b[0])));x<Math.min(canvas.width,Math.ceil(Math.max(a[0],b[0])));x++){
                const i=(y*canvas.width+x)*4;
                let delta=0,moved=0;
                for(let c=0;c<3;c++){
                    const first=reflectedA.data[i+c]-plainA.data[i+c];
                    delta+=Math.abs(first);
                    moved+=Math.abs(first-(reflectedB.data[i+c]-plainB.data[i+c]));
                }
                if(delta>20)reflectedPixels++;
                if(moved>20)movedPixels++;
            }
        if(reflectedPixels<40||movedPixels<20)throw new Error(`Fruit SSR missing or static on floor (${reflectedPixels} reflected, ${movedPixels} moving pixels)`);
        const oldReflection=document.querySelector('#reflection-evidence');
        const reflectionEvidence=oldReflection instanceof HTMLCanvasElement?oldReflection:document.createElement('canvas');
        reflectionEvidence.id='reflection-evidence';reflectionEvidence.hidden=true;
        if(!reflectionEvidence.isConnected)document.body.append(reflectionEvidence);
        reflectionEvidence.width=reflectedA.width;reflectionEvidence.height=reflectedA.height;
        reflectionEvidence.getContext('2d')?.putImageData(reflectedA,0,0);
        marker.remove();marker=undefined;
        if(originalView.orbitTarget)engine.setOrbitView(originalView.position,originalView.orbitTarget);
        // The remaining probes isolate glow, cutout and alpha from SSR.
        engine.postprocessing.disable('ssr');await wait();
        // A controlled foreground fruit must bleed colored light into formerly black pixels.
        const glowFruit=world.fruits.find(fruit=>fruit.name==='watermelon')??world.fruits[0];
        marker=engine.scene.createMesh(glowFruit.geometry).setPosition([0,1.35,-1.15]).setScale([.30,.30,.30])
            .setEmission(2).setColorCutout(.25);
        engine.setNeonGlow({strength:0});await wait();const noGlow=await capturePixels(canvas);
        if(engine.debugStats().neonGlow.bufferBytes!==0)throw new Error('Disabling glow did not release edge buffers');
        engine.setNeonGlow(NEON_GLOW);await wait();const withGlow=await capturePixels(canvas);
        let halo=0;
        for(let i=0;i<noGlow.data.length;i+=4)if(Math.max(...noGlow.data.slice(i,i+3))<6&&Math.max(...withGlow.data.slice(i,i+3))>12)halo++;
        if(halo<30)throw new Error(`No soft halo outside the actual neon surface (${halo} pixels)`);
        const wall=engine.scene.createCube([0,1.35,-.6]).setScale([4,4,.1]);alphaMarkers.push(wall);
        await wait();const wallGlow=await capturePixels(canvas);
        engine.setNeonGlow({strength:0});await wait();const wallPlain=await capturePixels(canvas);
        let leaks=0;
        for(let y=Math.floor(canvas.height*.3);y<canvas.height*.7;y++)for(let x=Math.floor(canvas.width*.3);x<canvas.width*.7;x++){
            const i=(y*canvas.width+x)*4;
            if(Math.abs(wallGlow.data[i]-wallPlain.data[i])+Math.abs(wallGlow.data[i+1]-wallPlain.data[i+1])+Math.abs(wallGlow.data[i+2]-wallPlain.data[i+2])>10)leaks++;
        }
        if(leaks>4)throw new Error(`Glow leaked through a solid foreground wall (${leaks} pixels)`);
        wall.remove();alphaMarkers.splice(alphaMarkers.indexOf(wall),1);marker.remove();marker=undefined;engine.setNeonGlow(NEON_GLOW);await wait();
        // A subdivided straight neon tube exposes the old additive endpoint bulbs:
        // its midpoint join must be as bright as the uninterrupted edge on both sides.
        const tubePositions=[-.6,0,.6].flatMap(x=>[[x,-.006,-.006],[x,.006,-.006],[x,.006,.006],[x,-.006,.006]].flat());
        const tubeIndices=[0,2,1,0,3,2,8,9,10,8,10,11];
        for(let section=0;section<2;section++)for(let edge=0;edge<4;edge++){
            const l=section*4,r=l+4,n=(edge+1)%4;
            tubeIndices.push(l+edge,l+n,r+n,l+edge,r+n,r+edge);
        }
        marker=engine.scene.createMesh({positions:tubePositions,indices:tubeIndices,color:[0,1,.6]})
            .setPosition([8,1.35,-1.15]).setEmission(2);
        engine.setOrbitView([8,1.35,1.3],[8,1.35,-1.15]);
        engine.setNeonGlow({strength:0});await wait();const tubeOff=await capturePixels(canvas);
        engine.setNeonGlow(NEON_GLOW);await wait();const tubeOn=await capturePixels(canvas);
        const sampleBand=(/** @type {ImageData} */ pixels,/** @type {number} */ x)=>{
            const point=engine.worldToScreen([8+x,1.366,-1.15]);if(!point)throw new Error('Neon tube projection unavailable');
            let value=0;
            for(let dy=-1;dy<=1;dy++)for(let dx=-1;dx<=1;dx++){
                const i=((Math.round(point[1]*pixels.height)+dy)*pixels.width+Math.round(point[0]*pixels.width)+dx)*4;
                value=Math.max(value,pixels.data[i+1]);
            }
            return value;
        };
        const edgeSamples=[-.3,0,.3].map(x=>sampleBand(tubeOn,x));
        if([-.3,0,.3].some(x=>sampleBand(tubeOff,x)>5))throw new Error('Neon edge probe must sample outside the opaque tube');
        if(Math.min(...edgeSamples)<30||Math.max(...edgeSamples)>Math.min(...edgeSamples)*1.3)
            throw new Error(`Neon edge is dim or has an endpoint hotspot: ${edgeSamples}`);
        marker.remove();marker=undefined;
        if(originalView.orbitTarget)engine.setOrbitView(originalView.position,originalView.orbitTarget);
        await wait();
        engine.postprocessing.enable('bloom',{strength:.1});await wait();
        if(engine.postprocessing.stats().passes!==1)throw new Error('Glow must also preserve auxiliary depth targets');
        engine.postprocessing.disable('bloom');await wait();
        // A blue solid behind the pear must be visible through the gaps, not just a black painted face.
        // Isolate mesh cutout: the deliberately wide halo can cover the tiny gaps in color.
        engine.setNeonGlow({strength:0});
        const target=world.apples[1],origin=world.previewPositions[1];
        const geometry=world.fruits[1].geometry;
        // Inspect the smaller row symbol close enough for the existing pixel threshold.
        engine.setOrbitView([origin[0],origin[1],origin[2]+1.8],origin);
        marker=engine.scene.createMesh({positions:geometry.positions,indices:geometry.indices,color:[0,.08,1]})
            .setPosition([origin[0],origin[1],origin[2]-.14])
            .setScale([.071,.071,.071]).setEmission(2);
        const blue=(/** @type {ImageData} */ pixels)=>{
            let count=0;for(let i=0;i<pixels.data.length;i+=4)
                if(pixels.data[i+2]>pixels.data[i]*1.5+20 && pixels.data[i+2]>pixels.data[i+1]*1.4)count++;
            return count;
        };
        target.setColorCutout(0);await wait();const opaqueBlue=blue(await capturePixels(canvas));
        target.setColorCutout(.25);await wait();const openBlue=blue(await capturePixels(canvas));
        if(openBlue<opaqueBlue+100)throw new Error(`Neon mesh does not reveal the object behind it (${opaqueBlue} -> ${openBlue})`);
        marker.remove();marker=undefined;
        engine.setNeonGlow(NEON_GLOW);
        if(originalView.orbitTarget)engine.setOrbitView(originalView.position,originalView.orbitTarget);
        await wait();
        world.apples.forEach(apple=>apple.setPosition([100,100,100]));await wait();
        assertChanged(image,await capturePixels(canvas),'actual fruit preview',100);
        const {previewCut}=await import('./main.js');
        const demo=previewCut(1); // Verify the alpha material at full diagnostic size.
        // Warm the first-use texture pipeline while native effect time is frozen.
        await wait();
        engine.set_simulation_paused(false);await wait();engine.set_simulation_paused(true);
        // An oblique view proves the flat disc surface, not only its edge-on silhouette.
        engine.setOrbitView([1.7,1.95,1.4],[0,1.35,-1.15]);await wait();
        const hitFrame=await capturePixels(canvas);
        const effectPositions=demo.feedback.objects.map(object=>object.meshInfo.position);
        demo.feedback.objects.forEach(object=>object.setPosition([100,100,100]));await wait();
        assertChanged(hitFrame,await capturePixels(canvas),'real cut shockwave, sparks and bitmap hit popup',80);
        // Look directly through the newly cut face: a solid blue marker behind
        // the remaining half must reach the framebuffer, including its depth.
        const halves=demo.report.pieces.map(piece=>({piece,position:piece.meshInfo.position}));
        const openHalf=halves.reduce((a,b)=>a.position[0]<b.position[0]?a:b);
        halves.filter(half=>half!==openHalf).forEach(half=>half.piece.setPosition([100,100,100]));
        marker=engine.scene.createMesh({positions:geometry.positions,indices:geometry.indices,color:[0,0,1]}).setPosition([-.6,1.35,-1.15]).setScale([.2,.4,.4]).setEmission(2);
        engine.setCameraView([1.2,1.35,-1.15],[0,1.35,-1.15]);await wait();
        const openFace=await capturePixels(canvas);
        let throughCut=0,totalCut=0;
        const radius=Math.max(3,Math.floor(Math.min(openFace.width,openFace.height)*.01));
        for(let y=Math.floor(openFace.height/2)-radius;y<=Math.floor(openFace.height/2)+radius;y++)
            for(let x=Math.floor(openFace.width/2)-radius;x<=Math.floor(openFace.width/2)+radius;x++){
                const i=(y*openFace.width+x)*4;totalCut++;
                if(openFace.data[i+2]>80&&openFace.data[i+2]>openFace.data[i]*1.5+20&&openFace.data[i+2]>openFace.data[i+1]*1.4)throughCut++;
            }
        if(throughCut<totalCut*.7)throw new Error(`Cut face still occludes the object behind it (${throughCut}/${totalCut})`);
        marker.remove();marker=undefined;
        halves.forEach(half=>half.piece.setPosition(half.position));
        engine.setOrbitView([1.7,1.95,1.4],[0,1.35,-1.15]);await wait();
        demo.feedback.objects.forEach((object,i)=>object.setPosition(effectPositions[i]));
        if(demo.report.pieces.length!==2||!demo.report.pieces.every(piece=>piece.meshInfo.closed))throw new Error('Preview must show two real closed fruit halves');
        // Changing a colored surface BEHIND the FX must change the composite,
        // while cyan texture remains visible: proves real blending, not holes or opaque paint.
        const ring=demo.feedback.objects[0],info=ring.meshInfo,mesh=ring.meshSnapshot;
        const behind=[info.position[0]-.06,info.position[1],info.position[2]];
        const [rx,,rz]=info.rotation;
        const bottom=Math.min(...mesh.positions.map(([x,y,z])=>Math.cos(rx)*(Math.sin(rz)*x+Math.cos(rz)*y)-Math.sin(rx)*z));
        const uniformScale=(info.bottom-info.position[1])/bottom;
        const scale=/** @type {[number,number,number]} */([uniformScale,uniformScale,uniformScale]);
        for(const color of [[1,0,0],[0,0,1]])alphaMarkers.push(engine.scene.createMesh({
            positions:mesh.positions.flat(),indices:mesh.triangles.flat(),color:/** @type {[number,number,number]} */(color)
        }).setRotation(info.rotation).setScale(scale).setPosition([100,100,100]).setEmission(1));
        alphaMarkers[0].setPosition(/** @type {[number,number,number]} */(behind));await wait();
        const redComposite=await capturePixels(canvas);
        alphaMarkers[0].setPosition([100,100,100]);alphaMarkers[1].setPosition(/** @type {[number,number,number]} */(behind));await wait();
        const blueComposite=await capturePixels(canvas);
        let blended=0;
        for(let i=0;i<redComposite.data.length;i+=4){
            const r=redComposite.data,b=blueComposite.data;
            if(r[i]-b[i]>20&&b[i+2]-r[i+2]>20&&r[i+1]>25&&b[i+1]>25)blended++;
        }
        if(blended<40)throw new Error(`Cut texture does not alpha-blend with geometry behind it (${blended})`);
        alphaMarkers.forEach(object=>object.remove());
        // Store the actual cut frame for the maintained runner, without rerendering a mock.
        const existingEvidence=document.querySelector('#cut-evidence');
        const evidence=existingEvidence instanceof HTMLCanvasElement?existingEvidence:document.createElement('canvas');
        evidence.id='cut-evidence';evidence.hidden=true;
        if(!evidence.isConnected)document.body.append(evidence);
        evidence.width=hitFrame.width;evidence.height=hitFrame.height;
        evidence.getContext('2d')?.putImageData(hitFrame,0,0);
        engine.scene.clearCutFeedback();
        engine.resize(640,480);await wait();
        if(engine.postprocessing.stats().targets!==0)throw new Error('Resize allocated postprocess targets');
    } finally {
        if(originalView.orbitTarget)engine.setOrbitView(originalView.position,originalView.orbitTarget);
        alphaMarkers.forEach(object=>{if(object.exists)object.remove();});
        if(marker?.exists)marker.remove();
        engine.scene.clearCutFeedback();
        engine.setNeonGlow(NEON_GLOW);
        engine.postprocessing.enable('ssr',FLOOR_SSR);
        world.apples.forEach(fruit=>fruit.setColorCutout(.25));
        world.apples.forEach((apple,index)=>apple.setPosition(world.previewPositions[index]));
        engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
        engine.set_simulation_paused(false);await wait();
    }
}
