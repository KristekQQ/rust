export async function verifyFox(page, verifyHud) {
    // Match the character's 100ms frame-delta cap. Wall-clock sleeps can expire
    // before slow software rendering has delivered enough simulation frames.
    const delay = milliseconds => page.evaluate(async (duration, timeout) => {
        const { engine, fox } = await import('./main.js');
        return new Promise((resolve, reject) => {
            let previous = performance.now(), elapsed = 0, frame;
            const timer = setTimeout(() => {
                cancelAnimationFrame(frame);
                reject(new Error('Character frame wait timed out'));
            }, timeout);
            const step = () => {
                const now = performance.now();
                elapsed += Math.min(Math.max(now - previous, 0), 100);
                previous = now;
                if (elapsed >= duration) {
                    clearTimeout(timer);
                    const info = fox.info;
                    resolve({ ...engine.characterState(), clip: info.animations[info.playback?.clip]?.name });
                }
                else frame = requestAnimationFrame(step);
            };
            frame = requestAnimationFrame(step);
        });
    }, milliseconds, page.getDefaultTimeout());
    const observeCharacter = async expected => {
        // Install and acknowledge the sampler before injecting the input. CDP
        // polling setup can itself miss a short airborne phase on slow frames.
        const probe = await page.evaluateHandle(async ({minY, atY, maxX, grounded, upward, pause}) => {
            const {engine} = await import('./main.js');
            const probe = {state:null, frame:0};
            const sample = () => {
                const state = engine.characterState();
                if (state && state.grounded === grounded
                    && (maxX === undefined || state.position[0] <= maxX)
                    && (minY === undefined || state.position[1] > minY)
                    && (atY === undefined || Math.abs(state.position[1] - atY) < .01)
                    && (!upward || state.verticalVelocity > 0)) {
                    probe.state = state;
                    if (pause) engine.set_simulation_paused(true);
                }
                else probe.frame = requestAnimationFrame(sample);
            };
            probe.frame = requestAnimationFrame(sample);
            return probe;
        }, expected);
        return async () => {
            let handle;
            try {
                handle = await page.waitForFunction(probe => probe.state, {polling:'raf'}, probe);
                return await handle.jsonValue();
            } catch (error) {
                throw new Error(`Character wait ${JSON.stringify(expected)}: ${JSON.stringify(await read())}`, {cause:error});
            } finally {
                if (handle) await handle.dispose();
                await probe.evaluate(probe => cancelAnimationFrame(probe.frame));
                await probe.dispose();
            }
        };
    };
    await page.waitForFunction(() => document.documentElement.dataset.rendererBackend);
    await verifyHud(page);
    const read = () => page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        const info = fox.info;
        return { ...engine.characterState(), clip: info.animations[info.playback?.clip]?.name };
    });
    const assert = (condition, message) => { if (!condition) throw new Error(message); };
    const frameHash = () => page.evaluate(async () => {
        const { capturePixels, assertForeground } = await import('../../tests/pixels.js');
        const image = await capturePixels(document.querySelector('#gpu-canvas'));
        assertForeground(image, 'Fox example');
        let hash = 2166136261;
        for (const value of image.data) hash = Math.imul(hash ^ value, 16777619);
        return hash;
    });
    const distance = (a, b) => Math.hypot(b.position[0] - a.position[0], b.position[2] - a.position[2]);
    const canvas = await page.$('#gpu-canvas');
    const waitGameState = async (playing, label) => {
        try {
            await page.waitForFunction(active => active
                ? document.pointerLockElement === document.querySelector('#gpu-canvas') && document.fullscreenElement === document.querySelector('#gpu-canvas')
                : !document.pointerLockElement && !document.fullscreenElement,
            { timeout: 10000 }, playing);
        } catch {
            const state = await page.evaluate(() => ({
                lock: document.pointerLockElement?.id, fullscreen: document.fullscreenElement?.id,
                focused: document.hasFocus(), status: document.querySelector('#status')?.textContent,
            }));
            throw new Error(`${label}: ${JSON.stringify(state)}`);
        }
    };
    const enterGame = async () => {
        const bounds = await canvas.boundingBox();
        await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
        await waitGameState(true, 'Click enters fullscreen and pointer lock');
        await delay(120);
    };
    await enterGame();
    const box = await canvas.boundingBox();
    const spawn = await read();
    assert(spawn.clip === 'Survey' && !spawn.moving && spawn.grounded, 'Fox starts idle on solid ground');
    await page.keyboard.down('w');
    await delay(750);
    const blocked = await read();
    assert(blocked.position[2] >= 1.1 && blocked.position[2] <= 1.201, 'Solid platform blocks forward movement at its front face');
    await delay(250);
    assert(distance(blocked, await read()) < 0.01, 'Holding W cannot walk through a platform');
    const airborneProbe = await observeCharacter({minY:.5, grounded:false, upward:true});
    const landedProbe = await observeCharacter({atY:.6, grounded:true});
    await page.keyboard.down('Space');
    const airborne = await airborneProbe();
    assert(airborne.position[1] > 0.5 && !airborne.grounded && airborne.verticalVelocity > 0, 'Space produces an airborne upward jump');
    const landed = await landedProbe();
    assert(Math.abs(landed.position[1] - 0.6) < 0.01 && landed.grounded, 'Fox lands on first platform instead of passing through it');
    await delay(250);
    assert(Math.abs((await read()).position[1] - 0.6) < 0.01, 'Holding Space does not auto-jump on landing');
    await page.evaluate(async () => (await import('./main.js')).engine.set_simulation_paused(true));
    await delay(120);
    await page.evaluate(async () => (await import('./main.js')).engine.set_simulation_paused(false));
    await page.keyboard.down('Space'); // Trusted repeat=true: this key is still held.
    await delay(220);
    const resumed = await read();
    assert(Math.abs(resumed.position[1] - 0.6) < 0.01 && resumed.grounded, 'Space held across pause cannot become a fresh jump');
    await page.keyboard.up('Space');
    await page.keyboard.down('w');
    await page.keyboard.down('d');
    const sliding = await delay(250);
    assert(sliding.position[0] - landed.position[0] > 0.2 && sliding.position[2] <= 3.901, `Diagonal movement slides along the next blocked platform: ${JSON.stringify({landed:landed.position,sliding:sliding.position})}`);
    await page.keyboard.up('d');
    const higherProbe = await observeCharacter({atY:1.2, grounded:true});
    await page.keyboard.press('Space');
    const higher = await higherProbe();
    assert(Math.abs(higher.position[1] - 1.2) < 0.01 && higher.grounded, 'A fresh jump reaches the higher second platform');
    await page.keyboard.up('w');
    // Reset through the same keyboard command as the player, then use an open lane.
    await page.keyboard.press('r');
    await delay(120);
    const reset = await read();
    assert(distance(reset, spawn) < 0.01 && Math.abs(reset.position[1]) < 0.01, 'R resets player and physics to the spawn');
    // Stop lane positioning in the sampled frame: CDP key-up can arrive several
    // slow frames later and otherwise overshoot into the left wall's path.
    const laneProbe = await observeCharacter({maxX:-2.5, grounded:true, pause:true});
    await page.keyboard.down('a');
    await laneProbe();
    await page.keyboard.up('a');
    await page.evaluate(async () => (await import('./main.js')).engine.set_simulation_paused(false));
    await delay(120);
    const initial = await read();
    const initialFrame = await frameHash();
    await page.keyboard.down('w');
    const walking = await delay(500);
    await page.keyboard.up('w');
    await delay(120);
    assert(distance(initial, walking) > 0.3 && walking.clip === 'Walk' && walking.moving, `W moves real model and selects Walk: ${JSON.stringify({initial,walking})}`);
    const stopped = await read();
    await delay(180);
    const idle = await read();
    assert(distance(stopped, idle) < 0.01 && idle.clip === 'Survey', 'Key release stops motion and selects idle');
    await page.keyboard.down('Shift');
    await page.keyboard.down('w');
    const running = await delay(500);
    await page.keyboard.up('w');
    await page.keyboard.up('Shift');
    await delay(120);
    assert(running.running && running.clip === 'Run' && distance(idle, running) > distance(initial, walking) * 1.3, `Shift runs faster and selects Run: ${JSON.stringify({idle,running,walkDistance:distance(initial,walking)})}`);
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.move(box.x + box.width / 2 + 160, box.y + box.height / 2, { steps: 8 });
    const turned = await read();
    await page.keyboard.down('w');
    await delay(450);
    const afterTurn = await read();
    assert(afterTurn.position[0] - turned.position[0] > 0.1, 'Mouse moving right turns right without holding any button');
    assert(await frameHash() !== initialFrame, 'Real canvas pixels change after movement and mouse steering');
    await page.evaluate(() => window.dispatchEvent(new Event('blur')));
    await waitGameState(false, 'Blur releases fullscreen and pointer lock');
    await delay(120);
    const blurred = await read();
    await delay(180);
    assert(distance(blurred, await read()) < 0.01, 'Blur releases held movement keys');
    await page.keyboard.up('w');
    await enterGame();
    await page.keyboard.down('w');
    await delay(160);
    await page.evaluate(async () => {
        const { engine } = await import('./main.js');
        engine.set_camera_mode('free');
    });
    await delay(120);
    const released = await page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        const info = fox.info;
        return { controller: engine.characterState(), clip: info.animations[info.playback?.clip]?.name };
    });
    assert(released.controller === null && released.clip === 'Survey', 'Changing camera releases control and the Walk animation');
    await page.keyboard.up('w');
    // Leave the example in its intended chase-camera mode for capture.
    await page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        engine.controlCharacter(fox, { animations: { idle: 'Survey', walk: 'Walk', run: 'Run' } });
    });
    await page.keyboard.down('w');
    await delay(160);
    await page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        engine.set_camera_mode('free');
        fox.playAnimation('Walk', { speed: 0.5 });
    });
    await delay(120);
    const manual = await page.evaluate(async () => {
        const { fox } = await import('./main.js');
        const info = fox.info;
        return { clip: info.animations[info.playback?.clip]?.name, speed: info.playback?.speed };
    });
    assert(manual.clip === 'Walk' && manual.speed === 0.5, 'Detached controller preserves an explicit same-clip animation command');
    await page.keyboard.up('w');
    await page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        engine.controlCharacter(fox, { animations: { idle: 'Survey', walk: 'Walk', run: 'Run' } });
    });
    await delay(120);
    await page.keyboard.press('Escape');
    await waitGameState(false, 'Escape releases fullscreen and pointer lock');
    const escaped = await read();
    await page.keyboard.down('w');
    await delay(180);
    assert(distance(escaped, await read()) < 0.01, 'Escape pauses movement while cursor is released');
    await page.keyboard.up('w');
    await delay(600);
    await enterGame();
    await page.keyboard.press('r');
    await delay(120);
    return 'PASS: solid obstacles, wall sliding, gravity, jump/landing on two platforms, R reset, fullscreen/mouse/Escape, clips and lifecycle';
}
