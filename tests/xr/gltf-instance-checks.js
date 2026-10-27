import {pixelDifference} from './testkit.js';
import {assertForeground} from '../pixels.js';

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
    const frame = () => {
        sim.frame({dt:0, head:{position:[0,1.6,0], orientation:[0,0,0,1]}});
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
    engine.setPreparedPlanCacheEnabled(false);frame();
    const uncached=await readEyes();
    test.assert(uncached.every((image,eye)=>pixelDifference(warm[eye],image,0)===0),'cache-off oracle preserves exact stereo pixels');
    engine.setPreparedPlanCacheEnabled(true);frame();frame();

    instances[2].setPosition([.9,1.6,-2]);
    frame();
    const moved = await readEyes();
    test.assert(moved.every((image, eye) => pixelDifference(warm[eye], image)>0),
        'moving one glTF instance updates both XR eye projections', moved.map((image, eye) => pixelDifference(warm[eye], image)));
    instances[2].setPosition([baseline[2],1.6,-2]);
    frame();
    const restored = await readEyes();
    test.assert(restored.every((image, eye) => pixelDifference(first[eye], image, 0)===0),
        'restoring the glTF instance reproduces exact stereo pixels');

    if (shiftLeft) {
        const rightBefore = restored[1];
        shiftLeft(3);
        frame();
        const shifted = await readEyes();
        test.assert(pixelDifference(restored[0], shifted[0])>0, 'left projection shift changes glTF pixels in the left eye');
        test.assert(pixelDifference(rightBefore, shifted[1], 0)===0, 'left projection shift preserves exact glTF pixels in the right eye');
        shiftLeft(0);
        frame();
    }

    engine.scene.clear();
    frame();
    test.assert(engine.debugStats().gltf.instanceBufferBytes===0, 'scene.clear releases glTF XR instance buffers', engine.debugStats().gltf);
}
