import {capturePixels, assertForeground, assertChanged} from '../../tests/pixels.js';

/**
 * @param {import('../../js/engine.js').Engine} engine
 * @param {import('../../js/engine.js').ModelObject} monkey
 * @param {HTMLCanvasElement} canvas
 * @param {() => void} front
 */
export async function verify(engine, monkey, canvas, front) {
    const result = document.getElementById('result');
    if (!result) return;
    try {
        /** @param {unknown} value @param {string} message */
        const assert = (value, message) => {if (!value) throw new Error(message);};
        assert(monkey.exists && monkey.info.animations.some(a => a.name === 'Hello'), 'GLB + Hello clip');
        assert(monkey.info.nodes.some(n => n.name === 'Tail'), 'Missing tail geometry');
        front();
        monkey.seekAnimation(0).pauseAnimation(false);
        for (let i = 0; i < 12; i++) await new Promise(requestAnimationFrame);
        assert(monkey.info.playback?.playing && monkey.info.playback.time > 0,
            'Rust playback must advance automatically');
        monkey.pauseAnimation().seekAnimation(0);
        const a = await capturePixels(canvas);
        assertForeground(a, 'Monkey', 2000);
        monkey.seekAnimation(0.5);
        assertChanged(a, await capturePixels(canvas), 'Waving arm/head', 100);
        // Remove the only scene object: its pixels must disappear (no floor can mask failure).
        monkey.setPosition([100, 0, 0]);
        assertChanged(a, await capturePixels(canvas), 'Actual imported monkey pixels', 2000);
        monkey.setPosition([0, 0, 0]);
        engine.setOrbitView([-3.2, 2.4, -6], [0.1, 1.35, 0]);
        const rear = await capturePixels(canvas);
        assertForeground(rear, 'Monkey rear', 1500);
        assertChanged(a, rear, 'Front/rear geometry', 500);
        front();
        monkey.seekAnimation(0).pauseAnimation(false);
        assert(!monkey.info.lastError, 'Rust animation error');
        result.textContent = `PASS: ${engine.renderer_backend()} · GLB geometry, front/rear pixels, animated arm/head, Rust playback`;
    } catch (error) {
        monkey.setPosition([0, 0, 0]);
        front();
        monkey.pauseAnimation(false);
        result.textContent = `FAIL: ${error instanceof Error ? error.message : error}`;
        console.error(error);
    }
}
