// Optional DOM controls, deliberately outside the engine SDK.
import {AudioMixer} from '../../js/audio.js';
export function installAudioControls({container=document.body,floating=true}={}){
    const box=document.createElement('div');box.style.cssText=(floating?'position:fixed;right:12px;bottom:12px;z-index:30;':'')+'display:flex;gap:8px;align-items:center;padding:8px;background:#101522dd;color:white;border-radius:8px;font:13px sans-serif';
    const button=document.createElement('button');button.textContent='Zapnout zvuk';
    const slider=document.createElement('input');slider.type='range';slider.min='0';slider.max='100';slider.value='100';slider.style.cssText='min-width:0;max-width:130px;flex:1';slider.setAttribute('aria-label','Hlasitost zvuku');
    box.append(button,slider);container.append(box);
    let mixer=null,loading=null,disposed=false,enabled=false;
    async function enable(){
        if(disposed)return Promise.resolve();
        if(!mixer){try{mixer=new AudioMixer();}catch(error){button.textContent='Zvuk není dostupný';throw error;}}
        if(!loading)loading=mixer.load(new URL('../../assets/audio/neon/manifest.json',import.meta.url));
        const resumed=mixer.resume();
        return Promise.all([resumed,loading]).then(()=>{if(disposed)return;enabled=true;mixer.setOutputEnabled(!document.hidden);button.textContent='Ztlumit zvuk';mixer.setVolume(Number(slider.value)/100);}).catch(error=>{loading=null;button.textContent='Zvuk nelze spustit';console.warn('Audio:',error);throw error;});
    }
    button.onclick=()=>{if(enabled){enabled=false;mixer.setOutputEnabled(false);mixer.setVolume(0);button.textContent='Zapnout zvuk';}else void enable().catch(()=>{});};
    slider.oninput=()=>{if(enabled)mixer?.setVolume(Number(slider.value)/100);};
    const hide=()=>mixer?.setOutputEnabled(enabled&&!document.hidden);document.addEventListener('visibilitychange',hide);
    return {get mixer(){return mixer;},enable,reset(){mixer?.reset();},dispose(){disposed=true;document.removeEventListener('visibilitychange',hide);box.remove();void mixer?.dispose();}};
}
