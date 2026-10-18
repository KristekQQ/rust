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
        if(stats.passes!==0||stats.targets!==0)throw new Error('Unexpected postprocessing GPU work');
        if(performance.getEntriesByType('resource').some(r=>/\/(?:color|emission)-(?:px|nx|py|ny|pz|nz)\.png/.test(r.name)))throw new Error('Environment images were loaded');
        // A blue solid behind the pear must be visible through the gaps, not just a black painted face.
        const target=world.apples[1],origin=world.previewPositions[1];
        const geometry=world.fruits[1].geometry;
        marker=engine.scene.createMesh({positions:geometry.positions,indices:geometry.indices,color:[0,.08,1]})
            .setPosition([origin[0]*1.12,1.65+(origin[1]-1.65)*1.12,1.6+(origin[2]-1.6)*1.12])
            .setScale([.145,.145,.145]).setEmission(2);
        const blue=(/** @type {ImageData} */ pixels)=>{
            let count=0;for(let i=0;i<pixels.data.length;i+=4)
                if(pixels.data[i+2]>pixels.data[i]*1.5+20 && pixels.data[i+2]>pixels.data[i+1]*1.4)count++;
            return count;
        };
        target.setColorCutout(0);await wait();const opaqueBlue=blue(await capturePixels(canvas));
        target.setColorCutout(.25);await wait();const openBlue=blue(await capturePixels(canvas));
        if(openBlue<opaqueBlue+100)throw new Error(`Neon mesh does not reveal the object behind it (${opaqueBlue} -> ${openBlue})`);
        marker.remove();marker=undefined;await wait();
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
        world.apples.forEach(fruit=>fruit.setColorCutout(.25));
        world.apples.forEach((apple,index)=>apple.setPosition(world.previewPositions[index]));
        engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
        engine.set_simulation_paused(false);await wait();
    }
}
