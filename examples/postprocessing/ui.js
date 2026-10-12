// Optional application UI. No GPU resources, camera math or animation loop here.
export function createUI(container, engine) {
    if (!container) throw new Error('Chybí UI');
    container.innerHTML = `
        <a href="../">← Rozcestník / ukončit render</a>
        <h1>Katedrála Šibenik · geometrie a světlo</h1>
        <label><input type="checkbox" name="ssr" checked> Screen-space reflections</label>
        <label>Síla SSR <input type="range" name="ssrStrength" min="0" max="1" step="0.05" value="0.85"></label>
        <label><input type="checkbox" name="godRays" checked> Volumetrické paprsky</label>
        <label>Síla paprsků <input type="range" name="godRaysStrength" min="0" max="1" step="0.05" value="0.65"></label>
        <label><input type="checkbox" name="pause"> Pozastavit pohyb</label>
        <label><input type="checkbox" name="texture"> Výstup přes texturu</label>
        <label><input type="checkbox" name="grid"> Pomocná mřížka</label>
        <label><input type="checkbox" name="helpers"> Značky světel</label>
        <button data-camera="free">Free kamera</button><button data-camera="orbit">Orbit kamera</button>
        <button id="verify">Ověřit řetězec</button>
        <pre role="status"></pre>
        <p>Táhni myší pro změnu pohledu. WASD: pohyb · Q/E: výška · Shift: rychleji. Odrazy obsahují pouze objekty viditelné kamerou. Paprsky se počítají v prostoru ze stínových map prvního světla.</p>
        <p>Model: Marko Dabrovic, opravy Kenzie Lamar. <a href="https://casual-effects.com/data/">McGuire Computer Graphics Archive</a> · CC BY-NC (nekomerční demo). <a href="../../docs/GLTF.md">Import glTF</a>.</p>
        <p><a href="main.js">main.js</a> · <a href="scene.js">scene.js</a> · <a href="ui.js">ui.js</a></p>`;
    const field = name => container.querySelector(`[name="${name}"]`);
    const apply = () => {
        for (const name of ['ssr', 'godRays']) {
            if (field(name).checked) engine.postprocessing.enable(name, { strength: Number(field(`${name}Strength`).value) });
            else engine.postprocessing.disable(name);
        }
        engine.set_grid_visible(field('grid').checked);
        engine.set_light_helpers_visible(field('helpers').checked);
        engine.set_simulation_paused(field('pause').checked);
        engine.set_output_mode(field('texture').checked ? 'texture' : 'canvas');
    };
    container.addEventListener('input', apply);
    const cameraClick = event => {
        const mode = event.target instanceof HTMLElement ? event.target.dataset.camera : undefined;
        if (mode === 'free' || mode === 'orbit') engine.set_camera_mode(mode);
    };
    container.addEventListener('click', cameraClick);
    let message = '';
    const frames = async () => { for (let i = 0; i < 6; i++) await new Promise(requestAnimationFrame); };
    container.querySelector('#verify').onclick = async () => {
        try {
            let rejected = false;
            try { engine.postprocessing.enable('unknown'); } catch { rejected = true; }
            if (!rejected) throw new Error('Neznámý efekt nebyl odmítnut');
            rejected = false;
            try { engine.postprocessing.enable('ssr', { strength: NaN }); } catch { rejected = true; }
            if (!rejected) throw new Error('Neplatná síla nebyla odmítnuta');
            rejected = false;
            try { engine.postprocessing.setReflectionMask({ width: 2, height: 2, pixels: new Uint8Array(1) }); } catch { rejected = true; }
            if (!rejected) throw new Error('Neplatná maska nebyla odmítnuta');
            engine.postprocessing.disable('ssr'); engine.postprocessing.disable('godRays');
            await frames();
            if (engine.postprocessing.stats().targets !== 0) throw new Error('Vypnutí neuvolnilo textury');
            for (const name of ['ssr', 'godRays']) {
                const before = engine.postprocessing.stats().frames;
                engine.postprocessing.enable(name, { strength: 0.5 }); await frames();
                if (engine.postprocessing.stats().passes !== 1 || engine.postprocessing.stats().frames <= before) throw new Error('Samostatný efekt neběží');
                engine.postprocessing.disable(name);
            }
            engine.postprocessing.enable('ssr'); engine.postprocessing.enable('godRays');
            engine.resize(640, 360); await frames();
            const stats = engine.postprocessing.stats();
            if (stats.passes !== 2 || stats.width !== 640 || stats.height !== 360) throw new Error('Řetězec / resize');
            engine.set_output_mode('texture'); await frames(); engine.set_output_mode('canvas');
            const canvas = document.getElementById('gpu-canvas');
            engine.resize(canvas.clientWidth, canvas.clientHeight);
            apply();
            message = 'PASS: efekty samostatně i spolu, resize, textura, odmítnutí parametrů, uvolnění cílů';
        } catch (error) { message = `FAIL: ${error.message}`; console.error(error); }
    };
    const timer = setInterval(() => {
        const p = engine.postprocessing.stats();
        const r = engine.renderer_stats();
        container.querySelector('[role="status"]').textContent = `${message}\n${engine.renderer_backend()} · ${r.fps.toFixed(0)} FPS\nPost průchody ${p.passes}\nCíle ${p.targets} · ${p.width} × ${p.height}`;
    }, 250);
    return { dispose() { clearInterval(timer); container.removeEventListener('input', apply); container.removeEventListener('click', cameraClick); } };
}
