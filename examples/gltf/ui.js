/** @param {import('../../js/engine.js').Engine} engine @param {(kind:'fox'|'helmet')=>Promise<{model:import('../../js/engine.js').ModelObject}>} load */
export function createUI(engine, load) {
    const selector = /** @type {HTMLSelectElement} */(document.querySelector('#model'));
    const clip = /** @type {HTMLSelectElement} */(document.querySelector('#clip'));
    const playing = /** @type {HTMLInputElement} */(document.querySelector('#playing'));
    const speed = /** @type {HTMLInputElement} */(document.querySelector('#speed'));
    const status = /** @type {HTMLElement} */(document.querySelector('#status'));
    /** @type {import('../../js/engine.js').ModelObject|null} */
    let model = null;
    let loadSerial = 0;
    const refresh = async () => {
        const serial = ++loadSerial;
        status.textContent = 'Načítám…';
        model = null;
        selector.disabled = true;
        try {
            const loaded = await load(selector.value === 'helmet' ? 'helmet' : 'fox');
            if (serial !== loadSerial) return;
            model = loaded.model;
            clip.replaceChildren(...model.info.animations.map((a,i)=>new Option(a.name,String(i))));
            clip.value = String(Math.max(0,model.info.animations.findIndex(a=>a.name==='Walk')));
            playing.checked = true;
            speed.value = '1';
            clip.disabled = playing.disabled = speed.disabled = !model.info.animations.length;
            status.textContent = `${engine.renderer_backend()} · ${model.info.primitives} primitiv · ${model.info.textures} textur · ${model.info.skins} koster · ${model.info.animations.length} animací`;
        } catch(error) { if (serial === loadSerial) { status.textContent=String(error); console.error(error); } }
        finally { if (serial === loadSerial) selector.disabled=false; }
    };
    const play = () => { if(model?.info.animations.length) model.playAnimation(Number(clip.value),{speed:Number(speed.value)}).pauseAnimation(!playing.checked); };
    /** @param {number} fraction */
    const pose = (fraction) => {
        if (!model?.info.animations.length) return;
        const index = Number(clip.value);
        model.playAnimation(index).pauseAnimation().seekAnimation(model.info.animations[index].duration*fraction);
        playing.checked = false;
    };
    document.querySelector('#pose-start')?.addEventListener('click',()=>pose(0));
    document.querySelector('#pose-mid')?.addEventListener('click',()=>pose(0.5));
    selector.addEventListener('change',refresh);
    document.querySelector('#reset')?.addEventListener('click',refresh);
    clip.addEventListener('change',play); speed.addEventListener('input',play);
    playing.addEventListener('change',()=>model?.pauseAnimation(!playing.checked));
    refresh();
}
