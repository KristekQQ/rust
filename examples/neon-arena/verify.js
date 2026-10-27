import {ARENA_REVISION,NEON_GLOW,FLOOR_SSR,FRUIT_OPACITY,FRUIT_OPACITY_VARIATION,createFruitMesh} from './scene.js';
/** @param {import('../../js/engine.d.ts').Engine} engine
 * @param {Awaited<ReturnType<typeof import('./scene.js').createScene>>} world
 * @param {HTMLCanvasElement} canvas
 * @param {typeof import('./main.js').previewCut} previewCut */
export async function verifyArena(engine, world, canvas, previewCut) {
    const {capturePixels,assertForeground,assertChanged} = await import('../../tests/pixels.js');
    const wait=async()=>{for(let i=0;i<5;i++)await new Promise(requestAnimationFrame);};
    const originalView=engine.cameraViewState();
    const originalFont=world.arenaFont.selected;
    const fontSelect=document.getElementById('arena-font');
    if(!(fontSelect instanceof HTMLSelectElement)||fontSelect.disabled||fontSelect.value!==originalFont)throw new Error('Arena font selector unavailable');
    const selectFont=async(/** @type {'cynatar'|'neon'} */ value)=>{
        fontSelect.value=value;fontSelect.dispatchEvent(new Event('change'));await wait();
        if(world.arenaFont.selected!==value)throw new Error('Font selector did not update native panels');
    };
    const style=document.getElementById('solid-fruit');
    if(!(style instanceof HTMLInputElement)||!style.checked)throw new Error('Solid fruit must be selectable and enabled by default');
    engine.set_simulation_paused(true);
    /** @type {import('../../js/engine.js').SceneObject | undefined} */
    let marker;
    /** @type {import('../../js/engine.js').SceneObject[]} */
    const alphaMarkers=[];
    try {
        await selectFont('cynatar');
        const boundPanels=[world.timerPanel,world.scorePanel,world.awardPanels[2],...world.stockScorePanels];
        const roots=boundPanels.map(panel=>panel.id);
        if([world.timerPanel,world.scorePanel].some(panel=>panel.font3DInfo.glyphCount<1||panel.font3DInfo.depth<=0))throw new Error('Cynatar panels lack solid glyphs');
        const meshFont=engine.debugStats().font3d;
        if(meshFont.runtime.texts!==5||meshFont.runtime.liveGlyphs<3||meshFont.instanceGeometryBytes!==0)throw new Error(`Arena mesh font is not shared: ${JSON.stringify(meshFont)}`);
        const toggle=document.getElementById('ssr');
        const build=document.getElementById('build');
        if(!(toggle instanceof HTMLInputElement)||!build?.textContent?.includes(ARENA_REVISION))throw new Error('Missing loaded-version or SSR control');
        const mainModules=performance.getEntriesByType('resource').filter(resource=>new URL(resource.name).pathname.endsWith('/neon-arena/main.js'));
        if(mainModules.length!==1||!new URL(mainModules[0].name).searchParams.has('reload'))throw new Error('Entry module must have a fresh URL and initialize exactly once');
        toggle.click();await wait();
        if(engine.postprocessing.stats().targets!==0||!build.textContent.includes('vypnuto'))throw new Error('SSR control did not release offscreen targets');
        toggle.click();await wait();
        if(engine.postprocessing.stats().passes!==2||!build.textContent.includes('zapnuto'))throw new Error('SSR control did not restore reflections');
        engine.setNeonGlow({strength:0});await wait();const plainFont=await capturePixels(canvas);
        engine.setNeonGlow(NEON_GLOW);await wait();
        const forwardImage=await capturePixels(canvas);
        const tp=world.timerPanel.meshInfo.position;
        const fontCorners=[[-.55,.28],[.55,-.28]].map(([x,y])=>engine.worldToScreen([tp[0]+x,tp[1]+y,tp[2]]));
        if(!fontCorners[0]||!fontCorners[1])throw new Error('Countdown must be inside the forward camera');
        let fontHalo=0,fontCore=0;
        for(let y=Math.max(0,Math.floor(Math.min(fontCorners[0][1],fontCorners[1][1])*canvas.height));y<Math.min(canvas.height,Math.ceil(Math.max(fontCorners[0][1],fontCorners[1][1])*canvas.height));y++)
            for(let x=Math.max(0,Math.floor(Math.min(fontCorners[0][0],fontCorners[1][0])*canvas.width));x<Math.min(canvas.width,Math.ceil(Math.max(fontCorners[0][0],fontCorners[1][0])*canvas.width));x++){
                const i=(y*canvas.width+x)*4;
                if(Math.max(plainFont.data[i],plainFont.data[i+1],plainFont.data[i+2])<6&&forwardImage.data[i+2]>12&&forwardImage.data[i+1]>forwardImage.data[i]*1.2&&forwardImage.data[i+2]>forwardImage.data[i]*1.2)fontHalo++;
                if(plainFont.data[i+1]>150&&plainFont.data[i+2]>180)fontCore++;
            }
        if(fontHalo<20)throw new Error(`Countdown has no actual neon halo outside glyph pixels (${fontHalo})`);
        if(fontCore<3)throw new Error(`Countdown lacks a rendered luminous Cynatar face (${fontCore})`);
        assertForeground(forwardImage,'forward neon countdown and score');
        const stats=engine.postprocessing.stats();
        if(stats.passes!==2||stats.targets===0)throw new Error('Tiled floor SSR is not active in forward view');
        if(world.timerPanel.meshInfo.position[0]!==0||world.timerPanel.meshInfo.position[1]<=world.scorePanel.meshInfo.position[1]||world.galleryIcons.some(icon=>icon.meshInfo.position[0]>-2.4))
            throw new Error('Countdown/score are not ahead with the paytable to the left');

        // Keep actual frontal and oblique font renders for visual inspection.
        const fontEvidence=document.createElement('canvas');fontEvidence.id='font-evidence';fontEvidence.hidden=true;
        document.getElementById(fontEvidence.id)?.remove();
        fontEvidence.width=canvas.width*2;fontEvidence.height=canvas.height;
        const fontContext=fontEvidence.getContext('2d');
        if(!fontContext)throw new Error('Font evidence canvas unavailable');
        for(const [view,position] of [[0,2.1,-.75],[.85,2.15,-.9]].entries()){
            engine.setOrbitView(/** @type {[number,number,number]} */(position),[0,2.1,-2.15]);
            fontContext.putImageData(await capturePixels(canvas),view*canvas.width,0);
        }
        document.body.append(fontEvidence);
        await selectFont('neon');
        if(engine.debugStats().font3d.runtime.liveGlyphs!==0||boundPanels.some((panel,index)=>panel.id!==roots[index]))throw new Error('Font switch leaked glyphs or replaced panel handles');
        // Inspect the off-axis paytable intentionally rather than requiring it
        // to remain in the forward field of view.
        engine.setOrbitView([.25,1.3,-.4],[-2.75,1.3,-.4]);await wait();
        const image=await capturePixels(canvas);
        const font=engine.debugStats().numericFont;
        if(!font||font.glyphMeshes!==10||font.liveGlyphs!==3||font.panels!==2||font.geometryBytes<=0)
            throw new Error(`Countdown/score do not use cached instanced digits: ${JSON.stringify(font)}`);
        for(const panel of [world.timerPanel,world.scorePanel]) {
            const glyphs=panel.meshSnapshot;
            const zs=glyphs.positions.map(point=>point[2]);
            if(Math.max(...zs)-Math.min(...zs)<.01)throw new Error('Numeric font is still a flat billboard');
            const blues=glyphs.colors.filter(color=>color[2]>.5&&color[0]<.15&&color[1]<.5);
            if(!blues.length||!glyphs.normals.some(normal=>normal[2]>.1&&normal[2]<.9))
                throw new Error('Neon tubes need a blue base and a rounded surface for the shader profile');
        }
        const shared=engine.debugStats().sharedMeshes;
        const glintAssets=world.launchGlintAssets.length;
        if(glintAssets!==3)throw new Error('Launch show must reuse exactly three small color assets');
        if(!shared||shared.assets!==12+glintAssets+font.glyphMeshes||shared.gpuMeshes!==12+glintAssets+font.glyphMeshes||shared.liveInstances!==70+font.liveGlyphs+(engine.xr.comboDisplayState?.chains?.reduce((sum,c)=>sum+c.frames.length+c.icons.length,0)??0)||shared.colorTableBytes<=0)
            throw new Error(`Fruit geometry is not shared by kind: ${JSON.stringify(shared)}`);
        // Thirty fruit icons use ten groups; forty frames use two more.
        // Their independent transforms and combo highlights remain instanced.
        const expectedGlowDraws=shared.gpuMeshes-font.glyphMeshes-glintAssets;
        if(engine.debugStats().neonGlow.draws!==expectedGlowDraws)
            throw new Error(`Gallery glow did not batch shared fruit (${engine.debugStats().neonGlow.draws} / ${expectedGlowDraws})`);
        if(shared.geometryBytes<=0||shared.instanceGeometryBytes-font.instanceGeometryBytes<(shared.geometryBytes-font.geometryBytes)*3)
            throw new Error(`Gallery does not share repeated geometry: ${JSON.stringify(shared)}`);
        if(shared.colorTableBytes>=shared.geometryBytes)
            throw new Error(`Palette table still duplicates unchanged seams: ${JSON.stringify(shared)}`);
        await selectFont('cynatar');
        if([world.timerPanel,world.scorePanel].some(panel=>panel.font3DInfo.glyphCount<1))throw new Error('Cynatar did not restore');
        await selectFont('neon');
        const alpha=engine.debugStats().translucency;
        if(!alpha||alpha.cachedMeshes<1||alpha.fullIndices<=0||alpha.submittedIndices>=alpha.fullIndices*.35)
            throw new Error(`Alpha rendering still processes the opaque seam triangles: ${JSON.stringify(alpha)}`);
        assertForeground(image,'fruit and world bitmap panels');
        let black=0;
        for(let i=0;i<image.data.length;i+=4)if(image.data[i]<3&&image.data[i+1]<3&&image.data[i+2]<3)black++;
        if(black<image.width*image.height*.25)throw new Error('Background is not black');

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
        target.setOpacity(1).setColorCutout(0);await wait();const opaqueBlue=blue(await capturePixels(canvas));
        target.setColorCutout(.25);await wait();const openBlue=blue(await capturePixels(canvas));
        if(openBlue<opaqueBlue+100)throw new Error(`Neon mesh does not reveal the object behind it (${opaqueBlue} -> ${openBlue})`);
        marker.remove();marker=undefined;
        engine.setNeonGlow(NEON_GLOW);
        if(originalView.orbitTarget)engine.setOrbitView(originalView.position,originalView.orbitTarget);
        await wait();
        engine.setOrbitView([.25,1.3,-.4],[-2.75,1.3,-.4]);await wait();
        const galleryBefore=await capturePixels(canvas);
        world.apples.forEach(apple=>apple.setPosition([100,100,100]));await wait();
        assertChanged(galleryBefore,await capturePixels(canvas),'actual left fruit preview',100);
        if(originalView.orbitTarget)engine.setOrbitView(originalView.position,originalView.orbitTarget);
        // Exercise the UI before the open-style occlusion probes below.
        style.click();await wait();
        if(style.checked)throw new Error('Fruit style did not switch to open seams');
        const demo=previewCut(1,false); // Verify the alpha material at full diagnostic size.
        // Freeze the depth probe pose before wall-clock feedback animation moves/rotates it.
        const cutPoses=new Map(demo.report.pieces.map(piece=>[piece.id,piece.meshInfo]));
        // Native spawn initializes the real feedback at t=0. Keep that pose paused
        // while warming textures: five slow CI frames can outlive the 0.65s burst.
        await wait();
        // An oblique view proves the flat disc surface, not only its edge-on silhouette.
        engine.setOrbitView([1.7,1.95,1.4],[0,1.35,-1.15]);await wait();
        const hitFrame=await capturePixels(canvas);
        const effectPositions=demo.feedback.objects.map(object=>object.meshInfo.position);
        demo.feedback.objects.forEach(object=>object.setPosition([100,100,100]));await wait();
        assertChanged(hitFrame,await capturePixels(canvas),'real cut shockwave, sparks and bitmap hit popup',80);
        // Look directly through the newly cut face: a solid blue marker inside
        // the half must reach the framebuffer only when the cap is discarded.
        const halves=demo.report.pieces.map(piece=>({piece,position:piece.meshInfo.position,rotation:piece.meshInfo.rotation}));
        const openHalf=halves.reduce((a,b)=>a.position[0]<b.position[0]?a:b);
        halves.filter(half=>half!==openHalf).forEach(half=>half.piece.setPosition([100,100,100]));
        const cutPose=cutPoses.get(openHalf.piece.id);
        if(!cutPose)throw new Error("Missing original cut pose");
        openHalf.piece.setPosition(cutPose.position).setRotation(cutPose.rotation);
        // Put the marker inside the half, immediately behind its cap. A marker
        // behind the entire fruit also intersects opaque neon seams on the rind.
        if(cutPose.rotation.some(angle=>Math.abs(angle)>1e-6))throw new Error('Cut depth fixture must start unrotated');
        const vertices=openHalf.piece.meshSnapshot.positions;
        const minX=Math.min(...vertices.map(vertex=>vertex[0])),capX=Math.max(...vertices.map(vertex=>vertex[0]));
        const capVertices=vertices.filter(vertex=>Math.abs(vertex[0]-capX)<1e-5);
        if(capVertices.length<3)throw new Error('Missing planar positive-X cut cap');
        const capMin=[1,2].map(axis=>Math.min(...capVertices.map(vertex=>vertex[axis])));
        const capMax=[1,2].map(axis=>Math.max(...capVertices.map(vertex=>vertex[axis])));
        const capCenter=/** @type {[number,number,number]} */([
            cutPose.position[0]+capX*cutPose.scale[0],
            cutPose.position[1]+(capMin[0]+capMax[0])*.5*cutPose.scale[1],
            cutPose.position[2]+(capMin[1]+capMax[1])*.5*cutPose.scale[2],
        ]);
        const depth=(capX-minX)*cutPose.scale[0];
        marker=engine.scene.createMesh({
            positions:[-.5,-.5,-.5,.5,-.5,-.5,.5,.5,-.5,-.5,.5,-.5,-.5,-.5,.5,.5,-.5,.5,.5,.5,.5,-.5,.5,.5],
            indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5],color:[0,0,1],
        }).setPosition([capCenter[0]-depth*.25,capCenter[1],capCenter[2]])
            .setScale([depth*.1,(capMax[0]-capMin[0])*cutPose.scale[1]*.5,(capMax[1]-capMin[1])*cutPose.scale[2]*.5])
            .setEmission(2);
        engine.setCameraView([capCenter[0]+1.2,capCenter[1],capCenter[2]],capCenter);await wait();
        engine.setNeonGlow({strength:0});await wait();
        const openFace=await capturePixels(canvas);
        openHalf.piece.setColorCutout(0);await wait();
        const closedFace=await capturePixels(canvas);
        openHalf.piece.setColorCutout(.25);
        let throughCut=0,throughClosed=0,totalCut=0;
        const radius=Math.max(3,Math.floor(Math.min(openFace.width,openFace.height)*.01));
        const isBlue=(/** @type {Uint8ClampedArray} */ data,/** @type {number} */ i)=>
            data[i+2]>80&&data[i+2]>data[i]*1.5+20&&data[i+2]>data[i+1]*1.4;
        for(let y=Math.floor(openFace.height/2)-radius;y<=Math.floor(openFace.height/2)+radius;y++)
            for(let x=Math.floor(openFace.width/2)-radius;x<=Math.floor(openFace.width/2)+radius;x++){
                const i=(y*openFace.width+x)*4;totalCut++;
                if(isBlue(openFace.data,i))throughCut++;
                if(isBlue(closedFace.data,i))throughClosed++;
            }
        if(throughCut<totalCut*.7||throughClosed>totalCut*.3)throw new Error(`Cut cap depth failed (${throughCut} open, ${throughClosed} closed / ${totalCut})`);
        marker.remove();marker=undefined;
        engine.setNeonGlow(NEON_GLOW);
        halves.forEach(half=>half.piece.setPosition(half.position).setRotation(half.rotation));
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
        style.click();await wait();
        const solidDemo=previewCut(1);
        const solidHalves=solidDemo.report.pieces;
        if(solidHalves.length!==2||!solidHalves.every(piece=>piece.meshInfo.closed))throw new Error('Solid style lost closed cut halves');
        const flesh=[1,.075,.18].map(c=>c*.22);
        if(!solidHalves.every(piece=>piece.meshSnapshot.colors.some(color=>color.every((c,i)=>Math.abs(c-flesh[i])<1e-5))))throw new Error('Solid cut has no colored flesh');
        const leftHalf=solidHalves.reduce((a,b)=>a.meshInfo.position[0]<b.meshInfo.position[0]?a:b);
        solidHalves.filter(piece=>piece!==leftHalf).forEach(piece=>piece.setPosition([100,100,100]));
        solidDemo.feedback.objects.forEach(object=>object.setPosition([100,100,100]));
        marker=engine.scene.createMesh({positions:geometry.positions,indices:geometry.indices,color:[0,0,1]}).setPosition([-.6,1.35,-1.15]).setScale([.2,.4,.4]).setEmission(2);
        engine.setCameraView([1.2,1.35,-1.15],[0,1.35,-1.15]);await wait();
        engine.setNeonGlow({strength:0});await wait();
        // Keep a controlled alpha here, independently of the artistic facet range.
        leftHalf.setOpacity(.58,{opaqueAbove:.25});await wait();
        const filledFace=await capturePixels(canvas);
        leftHalf.setOpacity(1);await wait();const opaqueFace=await capturePixels(canvas);
        let transmitted=0,tinted=0;
        for(let y=Math.floor(filledFace.height/2)-radius;y<=Math.floor(filledFace.height/2)+radius;y++)
            for(let x=Math.floor(filledFace.width/2)-radius;x<=Math.floor(filledFace.width/2)+radius;x++){
                const i=(y*filledFace.width+x)*4;
                if(filledFace.data[i+2]>opaqueFace.data[i+2]+20)transmitted++;
                if(filledFace.data[i]>25)tinted++;
            }
        if(transmitted<totalCut*.35||tinted<totalCut*.5)throw new Error(`Cut flesh is not colored translucent glass (${transmitted} transmitted, ${tinted} tinted / ${totalCut})`);
        leftHalf.setOpacity(FRUIT_OPACITY,{opaqueAbove:.25});await wait();
        const uniformAlpha=await capturePixels(canvas);
        leftHalf.setOpacity(FRUIT_OPACITY,{opaqueAbove:.25,variation:FRUIT_OPACITY_VARIATION});await wait();
        const variedAlpha=await capturePixels(canvas);await wait();const stableAlpha=await capturePixels(canvas);
        let variationPixels=0,unstablePixels=0;
        for(let y=Math.floor(variedAlpha.height/2)-radius;y<=Math.floor(variedAlpha.height/2)+radius;y++)
            for(let x=Math.floor(variedAlpha.width/2)-radius;x<=Math.floor(variedAlpha.width/2)+radius;x++){
                const i=(y*variedAlpha.width+x)*4;
                if(Math.abs(variedAlpha.data[i+2]-uniformAlpha.data[i+2])>3)variationPixels++;
                if(Math.abs(variedAlpha.data[i+2]-stableAlpha.data[i+2])>1)unstablePixels++;
            }
        if(variationPixels<totalCut*.3||unstablePixels>0)throw new Error(`Facet alpha variation is missing or flickers (${variationPixels} varied, ${unstablePixels} unstable)`);
        engine.setNeonGlow(NEON_GLOW);
        marker.remove();marker=undefined;
        solidHalves.forEach(piece=>piece.remove());
        await wait();const alphaCacheBefore=engine.debugStats().translucency;
        // One uniformly colored solid: different faces must transmit different
        // amounts of background, independently of surface color and lighting.
        engine.setNeonGlow({strength:0});
        const facetProbe=engine.scene.createMesh({
            positions:[-.5,-.5,-.5,.5,-.5,-.5,.5,.5,-.5,-.5,.5,-.5,-.5,-.5,.5,.5,-.5,.5,.5,.5,.5,-.5,.5,.5],
            indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5],
            color:[.025,.025,.025],smooth:false,
        }).setPosition([8,1.35,-1.15]).setScale([.5,.5,.5]).setOpacity(.25,{opaqueAbove:.25});
        alphaMarkers.push(facetProbe);
        engine.setCameraView([9.2,2.15,.05],[8,1.35,-1.15]);await wait();
        const faceSamples=[[8.25,1.35,-1.15],[8,1.35,-.90],[8,1.60,-1.15]].map(p=>engine.worldToScreen(/** @type {[number,number,number]} */(p)));
        const transmission=async()=>{
            engine.setBackgroundColor([0,0,0]);await wait();const blackFrame=await capturePixels(canvas);
            engine.setBackgroundColor([1,1,1]);await wait();const whiteFrame=await capturePixels(canvas);
            const linear=(/** @type {number} */ byte)=>{const v=byte/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;};
            return faceSamples.map(point=>{
                if(!point)throw new Error('Facet sample projection unavailable');
                const x=Math.round(point[0]*whiteFrame.width),y=Math.round(point[1]*whiteFrame.height);
                let sum=0;
                for(let dy=-1;dy<=1;dy++)for(let dx=-1;dx<=1;dx++){
                    const i=((y+dy)*whiteFrame.width+x+dx)*4+2;
                    sum+=linear(whiteFrame.data[i])-linear(blackFrame.data[i]);
                }
                return sum/9;
            });
        };
        const uniformTransmission=await transmission();
        facetProbe.setOpacity(.25,{opaqueAbove:.25,variation:1});
        const variedTransmission=await transmission();
        const spread=(/** @type {number[]} */ values)=>Math.max(...values)-Math.min(...values);
        if(spread(uniformTransmission)>.035||spread(variedTransmission)<.06)throw new Error(`Same-color faces lack independent alpha: ${uniformTransmission} -> ${variedTransmission}`);
        facetProbe.setOpacity(.06,{opaqueAbove:.25,variation:1,coverage:0});
        const sparseTransmission=await transmission();
        facetProbe.setOpacity(.06,{opaqueAbove:.25,variation:1,coverage:1});
        const denseTransmission=await transmission();
        if(sparseTransmission.some(value=>value<.65)||denseTransmission.some(value=>value>.03))throw new Error(`Panel coverage has no clear sparse/dense contrast: ${sparseTransmission} -> ${denseTransmission}`);
        engine.setBackgroundColor([0,0,0]);facetProbe.remove();
        // Compare actual transmission of three consecutive precomputed variants
        // of ONE fruit. RGB differences alone must not satisfy this check.
        engine.setCameraView([8,1.35,1.3],[8,1.35,-1.15]);
        const fruitTransmissions=[];
        for(const pattern of world.fruitPatterns.lemon.slice(0,3)){
            const fruit=createFruitMesh(engine.scene,pattern).setPosition([8,1.35,-1.15]).setScale([.28,.28,.28])
                .setEmission(1.1).setOpacity(FRUIT_OPACITY,{opaqueAbove:.25,variation:FRUIT_OPACITY_VARIATION,coverage:pattern.coverage??.5});
            alphaMarkers.push(fruit);
            engine.setBackgroundColor([0,0,0]);await wait();const black=await capturePixels(canvas);
            fruit.setPosition([100,100,100]);
            const original=engine.scene.createMesh(pattern.geometry).setPosition([8,1.35,-1.15]).setScale([.28,.28,.28])
                .setEmission(1.1).setOpacity(FRUIT_OPACITY,{opaqueAbove:.25,variation:FRUIT_OPACITY_VARIATION,coverage:pattern.coverage??.5});
            alphaMarkers.push(original);await wait();const reference=await capturePixels(canvas);
            let pixelError=0;
            for(let i=0;i<black.data.length;i++)pixelError+=Math.abs(black.data[i]-reference.data[i]);
            if(pixelError/black.data.length>.15)throw new Error(`Shared palette differs from original CPU mesh (${pixelError/black.data.length})`);
            original.remove();fruit.setPosition([8,1.35,-1.15]);
            engine.setBackgroundColor([1,1,1]);await wait();const white=await capturePixels(canvas);
            const values=new Float32Array(white.width*white.height);
            const linear=(/** @type {number} */ byte)=>{const v=byte/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;};
            for(let pixel=0;pixel<values.length;pixel++)values[pixel]=linear(white.data[pixel*4+2])-linear(black.data[pixel*4+2]);
            fruitTransmissions.push(values);fruit.remove();
        }
        for(let variant=1;variant<fruitTransmissions.length;variant++){
            const a=fruitTransmissions[variant-1],b=fruitTransmissions[variant];
            let changed=0,foreground=0;
            for(let pixel=0;pixel<a.length;pixel++)if(Math.min(a[pixel],b[pixel])<.9){changed+=Math.abs(a[pixel]-b[pixel]);foreground++;}
            if(!foreground||changed/foreground<.06)throw new Error(`Consecutive fruit variants look too similar on GPU (${variant}: ${changed/foreground})`);
        }
        engine.setBackgroundColor([0,0,0]);
        const sourcePatterns=world.fruitPatterns.lemon.slice(0,2);
        const sourceMesh=sourcePatterns[0].geometry;
        const temporaryAsset=engine.scene.createMeshVariants({positions:sourceMesh.positions,indices:sourceMesh.indices,
            variants:sourcePatterns.map(pattern=>pattern.geometry.triangleColors??[])});
        const survivor=temporaryAsset.createInstance(1).setPosition([8,1.35,-1.15]).setScale([.28,.28,.28]).setEmission(1.1);
        alphaMarkers.push(survivor);
        await wait();const sharedAllocated=engine.debugStats().sharedMeshes;
        if(sharedAllocated.assets!==shared.assets+1||sharedAllocated.gpuMeshes!==shared.gpuMeshes+1)throw new Error('Shared asset did not allocate once');
        const beforeRelease=await capturePixels(canvas);
        temporaryAsset.remove();await wait();
        if(!survivor.exists||engine.debugStats().sharedMeshes.gpuMeshes!==shared.gpuMeshes+1)throw new Error('Releasing asset invalidated live instance');
        const afterRelease=await capturePixels(canvas);
        assertForeground(afterRelease,'shared instance after asset handle release');
        let releasedPixelError=0;
        for(let i=0;i<beforeRelease.data.length;i++)releasedPixelError+=Math.abs(beforeRelease.data[i]-afterRelease.data[i]);
        if(releasedPixelError/beforeRelease.data.length>.01)throw new Error('Releasing asset changed surviving instance pixels');
        survivor.remove();await wait();const sharedClean=engine.debugStats().sharedMeshes;
        if(sharedClean.assets!==shared.assets||sharedClean.gpuMeshes!==shared.gpuMeshes||sharedClean.liveInstances!==shared.liveInstances||sharedClean.colorTableBytes!==shared.colorTableBytes)
            throw new Error(`Shared asset leaked after final instance: ${JSON.stringify(sharedClean)}`);
        engine.setBackgroundColor([0,0,0]);
        await wait();const alphaCacheAfter=engine.debugStats().translucency;
        if(alphaCacheAfter.cachedMeshes!==alphaCacheBefore.cachedMeshes||alphaCacheAfter.indexBufferBytes!==alphaCacheBefore.indexBufferBytes)
            throw new Error(`Temporary alpha index caches leaked: ${JSON.stringify(alphaCacheBefore)} -> ${JSON.stringify(alphaCacheAfter)}`);
        engine.setNeonGlow(NEON_GLOW);
        engine.resize(640,480);await wait();
        if(engine.postprocessing.stats().targets!==0)throw new Error('Resize allocated postprocess targets');
        return {sharedMeshes:shared,alpha};
    } finally {
        await selectFont(originalFont);
        if(originalView.orbitTarget)engine.setOrbitView(originalView.position,originalView.orbitTarget);
        alphaMarkers.forEach(object=>{if(object.exists)object.remove();});
        if(marker?.exists)marker.remove();
        engine.setBackgroundColor([0,0,0]);
        engine.scene.clearCutFeedback();
        engine.setNeonGlow(NEON_GLOW);
        engine.postprocessing.enable('ssr',FLOOR_SSR);
        if(!style.checked)style.click();
        world.galleryIcons.forEach((fruit,index)=>fruit.setColorCutout(0).setOpacity(FRUIT_OPACITY,{opaqueAbove:.25,variation:FRUIT_OPACITY_VARIATION,coverage:world.galleryPatterns[index].coverage??.5}));
        world.apples.forEach((apple,index)=>apple.setPosition(world.previewPositions[index]));
        engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
        engine.set_simulation_paused(false);await wait();
    }
}
