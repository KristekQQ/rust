import {pixelDifference} from './testkit.js';
import {assertForeground} from '../pixels.js';
import {createOverlayFixture} from '../gltf-overlay-fixture.js';

// Exercises real rigid glTF instances through the XR per-view render path.
export async function verifyGltfInstanceViews(engine, sim, test, {pixels, shiftLeft} = {}) {
    if (typeof pixels !== 'function') throw new Error('XR glTF instance check requires per-eye pixels');
    engine.scene.clear();
    engine.set_render_effects(false, false);
    engine.set_grid_visible(false);
    engine.set_light_helpers_visible(false);
    engine.postprocessing.disable('ssr');
    engine.postprocessing.disable('bloom');
    engine.setNeonGlow({strength:0});
    engine.scene.createLight({position:[-2,4,0],color:[12,12,12]});
    const frame = (headPosition=[0,1.6,0]) => {
        sim.frame({dt:0, head:{position:headPosition, orientation:[0,0,0,1]}});
        test.assert(!engine.xr.lastError, 'glTF instance stereo frame completes', String(engine.xr.lastError));
    };

    const model = await engine.scene.loadModel(new URL('../../assets/models/damaged-helmet/DamagedHelmet.glb', import.meta.url));
    const instances = [model, model.createInstance(), model.createInstance()];
    const place = x => instances.forEach((instance, index) => instance
        .setPosition([x[index],1.6,-2])
        .setScale([.18,.18,.18]));
    const baseline = [-.6,0,.6];
    place(baseline);
    const before = engine.debugStats().gltf;
    frame();
    const rendered = engine.debugStats().gltf;
    test.assert(rendered.instancedDrawCalls-before.instancedDrawCalls>=2 && rendered.instancesDrawn-before.instancesDrawn>=6,
        'rigid glTF instances use instanced draws in both XR eyes', {before,rendered});

    const readEyes = async () => [await pixels(0), await pixels(1)];
    const first = await readEyes();
    first.forEach((image, eye) => assertForeground(image, `glTF instances in ${eye ? 'right' : 'left'} XR eye`));
    frame();
    const warm = await readEyes();
    test.assert(warm.every((image, eye) => pixelDifference(first[eye], image, 0)===0),
        'identical frozen glTF instance frame reproduces exact stereo pixels');

    // Real eye attachments are the oracle; retaining handles alone cannot prove
    // that a different view has not overwritten their mutable GPU contents.
    frame();frame();
    const planBefore=engine.debugStats().gltf;
    frame();frame();
    const planAfter=engine.debugStats().gltf;
    test.assert(planAfter.preparedPlanCache.hits>=planBefore.preparedPlanCache.hits+4,
        'both frozen XR eye plans hit', {planBefore:planBefore.preparedPlanCache,planAfter:planAfter.preparedPlanCache});
    test.assert(planAfter.prepareOpaqueGroupRebuilds===planBefore.prepareOpaqueGroupRebuilds && planAfter.preparePayloadRebuilds===planBefore.preparePayloadRebuilds,
        'frozen XR eyes skip opaque groups and payload packing');
    engine.setPreparedPlanCacheEnabled(false);engine.set_frustum_culling(false);frame();
    const uncached=await readEyes();
    test.assert(uncached.every((image,eye)=>pixelDifference(warm[eye],image,0)===0),'cache-off culling-off independent oracle preserves exact stereo pixels');
    engine.set_frustum_culling(true);engine.setPreparedPlanCacheEnabled(true);frame();frame();

    const publicationBefore=engine.debugStats().gltf.publication;
    frame();frame();
    test.assert(engine.debugStats().gltf.publication.modelVisits===publicationBefore.modelVisits,
        'unchanged stereo models skip publication visits for both eyes');
    instances[2].setPosition([.9,1.6,-2]);
    frame();
    const moved = await readEyes();
    test.assert(moved.every((image, eye) => pixelDifference(warm[eye], image)>0),
        'moving one glTF instance updates both XR eye projections', moved.map((image, eye) => pixelDifference(warm[eye], image)));
    engine.setModelPublicationReuseEnabled(false);frame();
    const publishedReference=await readEyes();
    test.assert(publishedReference.every((image,eye)=>pixelDifference(moved[eye],image,0)===0),
        'changed model publication matches full-scan reference in both eyes');
    engine.setModelPublicationReuseEnabled(true);
    instances[2].setPosition([baseline[2],1.6,-2]);
    frame();
    const restored = await readEyes();
    test.assert(restored.every((image, eye) => pixelDifference(first[eye], image, 0)===0),
        'restoring the glTF instance reproduces exact stereo pixels');

    frame();frame();const headBefore=engine.debugStats().gltf;
    frame([.01,1.6,0]);const headAfter=engine.debugStats().gltf;
    test.assert(headAfter.finePreparedRowsReused>headBefore.finePreparedRowsReused && headAfter.finePayloadRecordsWritten===headBefore.finePayloadRecordsWritten && headAfter.fineWorldDataUpdates===headBefore.fineWorldDataUpdates,
        'head movement refreshes both views without rebuilding world payloads', {headBefore,headAfter});
    const headMoved=await readEyes();engine.setPreparedPlanCacheEnabled(false);engine.set_frustum_culling(false);frame([.01,1.6,0]);
    const headOracle=await readEyes();test.assert(headOracle.every((image,eye)=>pixelDifference(headMoved[eye],image,0)===0),'moving stereo head matches independent cache-off culling-off oracle');
    engine.set_frustum_culling(true);engine.setPreparedPlanCacheEnabled(true);
    frame();frame();

    if (shiftLeft) {
        const rightBefore = restored[1];
        frame();frame();const fineBefore=engine.debugStats().gltf;
        // Small change retains membership; the original large shift below
        // deliberately removes all geometry from the left view.
        shiftLeft(.015);
        frame();
        const fineAfter=engine.debugStats().gltf;
        test.assert(fineAfter.finePreparedRowsReused>fineBefore.finePreparedRowsReused && fineAfter.finePayloadRecordsWritten===fineBefore.finePayloadRecordsWritten,
            'changing only left projection reuses fine rows without repacking either eye', {fineBefore,fineAfter});
        const finePixels=await readEyes();
        test.assert(pixelDifference(restored[0],finePixels[0])>0 && pixelDifference(rightBefore,finePixels[1],0)===0,
            'fine left-only projection changes left pixels and preserves exact right pixels');
        engine.setPreparedPlanCacheEnabled(false);engine.set_frustum_culling(false);frame();
        const fineOracle=await readEyes();test.assert(fineOracle.every((image,eye)=>pixelDifference(finePixels[eye],image,0)===0),
            'fine left-only projection matches independent cache-off culling-off stereo oracle');
        engine.set_frustum_culling(true);engine.setPreparedPlanCacheEnabled(true);
        shiftLeft(0);frame();shiftLeft(3);frame();
        const shifted = await readEyes();
        test.assert(pixelDifference(restored[0], shifted[0])>0, 'left projection shift changes glTF pixels in the left eye');
        test.assert(pixelDifference(rightBefore, shifted[1], 0)===0, 'left projection shift preserves exact glTF pixels in the right eye');
        // Keep the right eye fixed while independently disabling main root or
        // primitive selection. The oracle reads both real eye attachments.
        try {
            for (const [roots, primitives] of [[false,true],[true,false],[false,false],[true,true]]) {
                engine.setMainViewCulling(roots,primitives);frame();
                const uncullEyes=await readEyes();
                test.assert(uncullEyes.every((image,eye)=>pixelDifference(shifted[eye],image,0)===0),
                    'main-only culling controls preserve asymmetric XR eye pixels '+[roots,primitives]);
            }
        } finally { engine.setMainViewCulling(true,true); }
        shiftLeft(0);
        frame();
    }

    // Repeated eye-owned pool oscillation; keep the asset alive while culling
    // changes membership, then compare actual independent eye attachments.
    if (engine.configureGltfInstancePool) {
        engine.configureGltfInstancePool({byteLimit:64*1024*1024});
        place(baseline);frame();frame();
        const poolStart=engine.debugStats().gltf.instancePool;
        for(let cycle=0;cycle<4;cycle++) {
            place([0,10000,10001]);frame();
            place([10000,10001,10002]);frame();
            place(baseline);frame();frame();
        }
        const poolEnd=engine.debugStats().gltf.instancePool;
        test.assert(poolEnd.buffersCreated===poolStart.buffersCreated && poolEnd.bindGroupsCreated===poolStart.bindGroupsCreated,
            'many/few/zero/many XR eye resources plateau after warmup',{poolStart,poolEnd});
        const pooled=await readEyes();
        engine.setPreparedPlanCacheEnabled(false);engine.set_frustum_culling(false);frame();
        const oracle=await readEyes();
        test.assert(oracle.every((image,eye)=>pixelDifference(pooled[eye],image,0)===0),
            'pooled stereo resources match independent cache-off culling-off pixels');
        engine.set_frustum_culling(true);engine.setPreparedPlanCacheEnabled(true);
        for(let cycle=0;cycle<3;cycle++) {
            await engine.xr.exit();await engine.xr.enter();frame();frame();
            const image=await readEyes();
            test.assert(image.every((image,eye)=>pixelDifference(pooled[eye],image,0)===0),
                'XR reentry preserves exact pooled eye pixels '+cycle);
        }
    }

    // Independent original vertex-layout control, through real per-eye targets.
    const optimizedLayout={...engine.debugStats().gltf.vertexColors,rigidLayouts:engine.gltfVertexLayouts()};
    test.assert(optimizedLayout.rigidLayouts.some(l=>l.omitColor), 'XR exercised omitted rigid color attributes', optimizedLayout);
    const reloadEyes = async enabled => {
        engine.scene.clear();frame();
        test.assert(engine.setGltfWhiteColorOmissionEnabled(enabled), 'XR clear permits explicit layout control '+enabled);
        engine.scene.createLight({position:[-2,4,0],color:[12,12,12]});
        const original=await engine.scene.loadModel(new URL('../../assets/models/damaged-helmet/DamagedHelmet.glb', import.meta.url));
        [original,original.createInstance(),original.createInstance()].forEach((m,i)=>m.setPosition([baseline[i],1.6,-2]).setScale([.18,.18,.18]));
        frame();frame();return await readEyes();
    };
    const originalEyes=await reloadEyes(false), originalRepeat=await reloadEyes(false);
    const colorlessEyes=await reloadEyes(true), colorlessRepeat=await reloadEyes(true);
    const changes=(a,b)=>b.map((image,eye)=>({changed:pixelDifference(a[eye],image,0),max:image.data.reduce((m,v,i)=>Math.max(m,Math.abs(v-a[eye].data[i])),0)}));
    const layoutEvidence={firstVsOriginal:changes(first,originalEyes),originalRepeat:changes(originalEyes,originalRepeat),colorlessRepeat:changes(colorlessEyes,colorlessRepeat),referenceVsColorless:changes(originalEyes,colorlessEyes)};
    window.xrWhiteLayoutEvidence=layoutEvidence;
    test.assert(originalEyes.every((image,eye)=>pixelDifference(first[eye],image,0)===0), 'original layout reproduces the initial optimized XR image', layoutEvidence);
    test.assert(originalRepeat.every((image,eye)=>pixelDifference(originalEyes[eye],image,0)===0) && colorlessRepeat.every((image,eye)=>pixelDifference(colorlessEyes[eye],image,0)===0), 'both XR layout reload controls are exactly repeatable', layoutEvidence);
    test.assert(originalEyes.every((image,eye)=>pixelDifference(colorlessEyes[eye],image,0)===0), 'omitted and original color layout reproduce exact pixels in both XR eyes', layoutEvidence);

    const compactIndexLayouts=engine.gltfIndexLayouts();
    engine.scene.clear();frame();
    test.assert(engine.setGltfCompactIndicesEnabled(false), 'XR empty scene selects original Uint32 indices');
    const uint32Eyes=await reloadEyes(true);
    test.assert(engine.gltfIndexLayouts().every(l=>l.format==='Uint32'), 'XR original path binds Uint32 geometry');
    test.assert(uint32Eyes.every((image,eye)=>pixelDifference(colorlessEyes[eye],image,0)===0), 'both XR eyes match independent original Uint32 geometry exactly');
    test.assert(compactIndexLayouts.some(l=>l.format==='Uint16'), 'XR optimized path exercised real Uint16 indices');

    // Overlay selection is exercised through actual eye targets, not a shared
    // desktop visibility list. Readback remains a correctness-only operation.
    const selectionReference=engine.debugStats().overlaySelectionReference;
    engine.postprocessing.enable('fxaa',{strength:.6});
    const blendUrl=URL.createObjectURL(new Blob([JSON.stringify(createOverlayFixture())],{type:'model/gltf+json'}));
    let blendModel;
    try{blendModel=await engine.scene.loadModel(blendUrl);}finally{URL.revokeObjectURL(blendUrl);}
    blendModel.setPosition([0,1.6,-1.65]).setScale([.6,.6,.6]);
    engine.setOverlaySelectionReference(true);
    blendModel.selectScene(1);frame();const noBlend=await readEyes();
    blendModel.selectScene(0);frame();const visibleBlend=await readEyes();
    test.assert(visibleBlend.every((image,eye)=>pixelDifference(noBlend[eye],image,0)>0),'BLEND primitive genuinely contributes pixels in both XR eyes');
    const overlayEyes=async(label)=>{
        engine.setOverlaySelectionReference(true);frame();const reference=await readEyes();
        const before=engine.debugStats().overlaySelection.calls;
        engine.setOverlaySelectionReference(false);frame();const optimized=await readEyes();
        test.assert(engine.debugStats().overlaySelection.calls-before===2,label+' selects separately for both eyes');
        engine.setOverlaySelectionReference(true);frame();const restored=await readEyes();
        for(let eye=0;eye<2;eye++){
            assertForeground(reference[eye],label+' eye '+eye);
            test.assert(pixelDifference(reference[eye],optimized[eye],0)===0&&pixelDifference(reference[eye],restored[eye],0)===0,label+' exact original/optimized/restored eye '+eye);
        }
    };
    try{
        await overlayEyes('XR mixed alpha overlay presence');
        blendModel.selectScene(1);await overlayEyes('XR inactive BLEND presence');blendModel.selectScene(0);
        if(shiftLeft){shiftLeft(.07);await overlayEyes('asymmetric left-eye overlay presence');shiftLeft(0);}
    }finally{
        engine.setOverlaySelectionReference(selectionReference);
        engine.postprocessing.disable('fxaa');
        if(shiftLeft)shiftLeft(0);
    }
    engine.scene.clear();
    frame();
    test.assert(engine.setGltfCompactIndicesEnabled(true), 'XR restores default index format policy');
    test.assert(engine.debugStats().gltf.instancePool?.liveBytes===0 && engine.debugStats().gltf.instanceBufferBytes===0, 'scene.clear releases glTF XR instance buffers', engine.debugStats().gltf);
    test.assert(engine.setGltfWhiteColorOmissionEnabled(true), 'XR restores default rigid layout policy');
}
