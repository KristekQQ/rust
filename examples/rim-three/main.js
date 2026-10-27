import './app.js';
import {STRESS_LEVELS} from '../rim/stress.js';
import {createThreeStressLoad} from './stress.js';

const $ = id => document.getElementById(id);
const params = new URLSearchParams(location.search);
const fromQuery = Number(params.get('stress'));
const initialCount = params.has('stress') && STRESS_LEVELS.includes(fromQuery) ? fromQuery : 0;
const initialMode = ['instanced', 'objects'].includes(params.get('representation')) ? params.get('representation') : 'instanced';
const countSelect = $('stress-count');
const modeSelect = $('stress-representation');
const status = $('stress-status');
for (const count of STRESS_LEVELS) {
    const option = document.createElement('option');
    option.value = String(count);
    option.textContent = String(count);
    countSelect.append(option);
}
countSelect.value = String(initialCount);
modeSelect.value = initialMode;

let load = null;
let disposed = false;
function writeQuery(count, mode) {
    const next = new URL(location.href);
    next.searchParams.set('stress', String(count));
    next.searchParams.set('representation', mode);
    history.replaceState(null, '', next);
}
function setStress(count, mode = modeSelect.value) {
    if (!load) throw new Error('Modely pro zátěž se ještě připravují.');
    load.setCount(count, mode);
    countSelect.value = String(count);
    modeSelect.value = mode;
    window.cityDebug.debugPanel.setStressInfo(load.describe());
    status.textContent = `${load.count} statických modelů · ${load.representation === 'instanced' ? 'instanced' : 'samostatné objekty'}`;
    writeQuery(count, mode);
    return load.describe();
}
countSelect.addEventListener('change', () => {
    try { setStress(Number(countSelect.value)); }
    catch (error) { status.textContent = `Zátěž se nepodařilo změnit: ${error.message}`; countSelect.value = String(load?.count ?? 0); }
});
modeSelect.addEventListener('change', () => {
    try { setStress(load?.count ?? initialCount, modeSelect.value); }
    catch (error) { status.textContent = `Reprezentaci se nepodařilo změnit: ${error.message}`; modeSelect.value = load?.representation ?? 'instanced'; }
});

async function initializeStress() {
    try {
        const debug = window.cityDebug;
        await debug.characters.ready;
        await debug.combat.ready;
        const source = debug.characters.state.entries.find(entry => entry.loaded && entry.slug === 'drone-orbiter');
        if (!source) throw new Error('Zdrojový model drone-orbiter se nenačetl.');
        source.mixer.setTime(0);
        source.root.updateMatrixWorld(true);
        load = createThreeStressLoad(debug.scene, source.root);
        debug.stress = load;
        debug.debugPanel.setStressInfo(load.describe());
        debug.setStress = setStress;
        if (initialCount || params.has('representation')) setStress(initialCount, initialMode);
        else status.textContent = 'Volitelná zátěž je vypnutá.';

        const renderText = window.render_game_to_text;
        window.render_game_to_text = () => JSON.stringify({...JSON.parse(renderText()), stress: load.describe()});
        if (params.get('verify') === '1') {
            const result = $('three-stress-result');
            result.textContent = 'Ověřuji…';
            try {
                const {verifyThreeStress} = await import('./verify.js');
                debug.setManualTime(true);
                await verifyThreeStress(debug);
                document.documentElement.dataset.rendererBackend = debug.renderer.getContext().getParameter(debug.renderer.getContext().VERSION).includes('WebGL 2') ? 'ThreeWebGL2' : 'unknown';
                result.textContent = 'PASS: Three clone/instance image and command checks';
                result.dataset.result = 'pass';
            } catch (error) {
                result.textContent = `FAIL: ${error.message}`;
                result.dataset.result = 'fail';
            }
        }
    } catch (error) {
        status.textContent = `Stress load selhal: ${error.message}`;
        status.dataset.result = 'fail';
        $('three-stress-result').textContent = `FAIL: ${error.message}`;
    }
}

addEventListener('pagehide', () => {
    if (disposed) return;
    disposed = true;
    load?.dispose();
}, {once: true});
void initializeStress();
