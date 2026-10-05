// Volitelné DOM nástroje pro vývoj. Tento modul nic nekreslí do WebGPU scény.
// Engine na něm nezávisí; ve vlastní aplikaci jej nemusíš importovat.
/**
 * @param {HTMLElement|null} container
 * @param {import('../../js/engine.js').Engine} engine
 * @param {{title:string, defaults:{shadows:boolean,mirror:boolean,grid:boolean,helpers:boolean},onReset:()=>void}} options
 */
export function createTools(container, engine, options) {
    if (!container) throw new Error('Kontejner nástrojů nebyl nalezen');
    container.innerHTML = `
        <a href="../">← Rozcestník / ukončit render</a>
        <h1></h1>
        <button data-action="reset">Obnovit scénu</button>
        <button data-action="stop">Ukončit render</button>
        <label><input type="checkbox" name="shadows"> Stíny světel</label>
        <label><input type="checkbox" name="mirror"> Zrcadlo</label>
        <label><input type="checkbox" name="texture"> Výstup přes texturu</label>
        <label><input type="checkbox" name="helpers"> Značky světel</label>
        <label><input type="checkbox" name="grid"> Pomocná mřížka</label>
        <label><input type="checkbox" name="pause"> Pozastavit simulaci</label>
        <label><input type="checkbox" name="culling" checked> Frustum culling</label>
        <button data-camera="orbit">Orbit</button><button data-camera="free">Free</button>
        <pre role="status"></pre>
        <p>Myš: pohled · WASD: pohyb · +/−: vzdálenost<br>Free: Q/E nahoru a dolů · Shift: rychleji</p>
        <p>Kód aplikace: <a href="main.js">main.js</a><br>Definice scény: <a href="scene.js">scene.js</a><br>Volitelné UI: <a href="../shared/tools.js">tools.js</a></p>`;
    container.querySelector('h1').textContent = options.title;
    const input = name => container.querySelector(`input[name="${name}"]`);
    for (const [name, value] of Object.entries(options.defaults)) {
        input(name).checked = value;
    }

    // UI posílá stejné veřejné příkazy jako libovolná jiná aplikace.
    const apply = () => {
        engine.set_render_effects(input('shadows').checked, input('mirror').checked);
        engine.set_output_mode(input('texture').checked ? 'texture' : 'canvas');
        engine.set_grid_visible(input('grid').checked);
        engine.set_light_helpers_visible(input('helpers').checked);
        engine.set_simulation_paused(input('pause').checked);
        engine.set_frustum_culling(input('culling').checked);
    };
    const change = event => {
        if (event.target instanceof HTMLInputElement) apply();
    };
    const click = event => {
        if (!(event.target instanceof HTMLElement)) return;
        if (event.target.dataset.action === 'reset') {
            options.onReset();
            input('pause').checked = false;
            apply();
        }
        // Navigace zruší stránku, WASM frame loop a její GPU prostředky.
        if (event.target.dataset.action === 'stop') location.assign('../');
        const camera = event.target.dataset.camera;
        if (camera === 'orbit' || camera === 'free') engine.set_camera_mode(camera);
    };
    container.addEventListener('change', change);
    container.addEventListener('click', click);
    const status = container.querySelector('[role="status"]');
    const timer = setInterval(() => {
        const r = engine.renderer_stats();
        const e = engine.effects_stats();
        status.textContent = `${engine.renderer_backend()} · ${r.fps.toFixed(0)} FPS\nViditelné ${r.visible}/${r.total}\nVyřazené ${r.culled} · dávky ${r.drawCalls}\nUpload ${r.uploadBytes} B\nStínující světla ${e.shadowLightCount}\nStatické / dynamické mapy ${e.staticUpdates} / ${e.dynamicUpdates}\nPrůchody ${e.passes}`;
    }, 250);
    return {
        dispose() {
            clearInterval(timer);
            container.removeEventListener('change', change);
            container.removeEventListener('click', click);
            container.replaceChildren();
        },
    };
}
