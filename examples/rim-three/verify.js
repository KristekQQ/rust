import {PerspectiveCamera, WebGLRenderTarget} from 'three';

// Independent ordinary-root clones are the image oracle. Readback is permitted
// here, outside every timing run. Two offset cameras test stereo transforms;
// they do not assert an XR session or physical-headset acceptance.
export async function verifyThreeStress(d) {
    const saved = d.stress.describe(), oldTarget = d.renderer.getRenderTarget();
    const target = new WebGLRenderTarget(960, 640, {samples: 4});
    const eyes = [['gate', 0, 1.7, 42], ['left-eye', -.032, 1.7, 42], ['right-eye', .032, 1.7, 42], ['oblique', 2, 2.2, 40]];
    const evidence = [];
    const source = d.characters.state.entries.find(e=>e.slug==='drone-orbiter'&&e.loaded), sourceTime = source.mixer.time;
    d.setManualTime(true);
    // Advance the live source after factory creation: both representations must
    // still use the private frozen pose, including a later objects-mode switch.
    source.mixer.setTime(sourceTime + .75); source.root.updateMatrixWorld(true);
    try {
        for (const load of [32, 512]) {
            const captures = new Map();
            for (const representation of ['objects', 'instanced']) {
                d.stress.setCount(load, representation);
                if (d.stress.count !== load || d.stress.representation !== representation) throw Error('Count/representation mismatch');
                for (const [name, x, y, z] of eyes) {
                    const camera = new PerspectiveCamera(65, 960 / 640, .08, 350);
                    camera.position.set(x, y, z); camera.lookAt(0, 1.7, -20); camera.updateMatrixWorld(true);
                    d.renderer.setRenderTarget(target); d.renderer.render(d.scene, camera);
                    const pixels = new Uint8Array(960 * 640 * 4);
                    d.renderer.readRenderTargetPixels(target, 0, 0, 960, 640, pixels);
                    const stats = {...d.renderer.info.render};
                    let foreground = 0; for (let i=4;i<pixels.length;i+=4) if (Math.abs(pixels[i]-pixels[0])+Math.abs(pixels[i+1]-pixels[1])+Math.abs(pixels[i+2]-pixels[2])>20) foreground++;
                    if(foreground<1000 || stats.triangles<100000) throw Error('Missing rendered workload in image oracle');
                    if (representation === 'objects') {captures.set(name, {pixels, stats}); continue;}
                    const oracle = captures.get(name); let over = 0, changed = 0, max = 0, total = 0;
                    for (let i = 0; i < pixels.length; i += 4) {
                        let error = 0;
                        for (let c = 0; c < 3; c++) {const delta = Math.abs(pixels[i+c] - oracle.pixels[i+c]); error = Math.max(error, delta); total += delta;}
                        max = Math.max(max, error); if (error) changed++; if (error > 8) over++;
                    }
                    const row = {load, camera: name, objectsCalls: oracle.stats.calls, instancedCalls: stats.calls,
                        objectsTriangles: oracle.stats.triangles, instancedTriangles: stats.triangles,
                        maxChannelError: max, meanChannelError: total / (960 * 640 * 3), changedPixels: changed,
                        fractionAbove8: over / (960 * 640), actualInstancedMeshes: d.stress.group.children.filter(n => n.isInstancedMesh).length};
                    evidence.push(row);
                    if (row.fractionAbove8 > .001) throw Error('Instancing pixel mismatch: ' + JSON.stringify(row));
                    if (row.instancedCalls >= row.objectsCalls || row.actualInstancedMeshes !== 67) throw Error('Instancing did not reduce real commands: ' + JSON.stringify(row));
                }
            }
        }
        const before = d.stress.describe(); let rejected = false;
        try {d.stress.setCount(2049);} catch {rejected = true;}
        if (!rejected || JSON.stringify(before) !== JSON.stringify(d.stress.describe())) throw Error('Invalid count changed load');
        d.stress.setCount(0);
        if (d.stress.group.children.length || d.stress.count) throw Error('Load removal failed');
        const result = {pass: true, scope: 'Three clone/InstancedMesh oracle at static poses; eye-offset cameras, not an XR session', liveSourceAdvancedSeconds: .75, evidence};
        window.threeStressVerification = result;
        return result;
    } finally {
        source.mixer.setTime(sourceTime); source.root.updateMatrixWorld(true);
        d.renderer.setRenderTarget(oldTarget); target.dispose();
        d.stress.setCount(saved.count, saved.representation);
        d.setManualTime(false);
    }
}
