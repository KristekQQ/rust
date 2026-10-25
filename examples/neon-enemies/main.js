import {initEngine} from '../shared/runtime-info.js';
import {createScene,loadEnemy} from './scene.js';

const canvas=document.getElementById('gpu-canvas');
if(!(canvas instanceof HTMLCanvasElement))throw new Error('Chybí plátno.');
const parameters=new URLSearchParams(location.search);
export const engine=await initEngine(canvas,{backend:parameters.get('backend')==='webgl'?'webgl':'auto'});
engine.set_render_effects(false,false);
engine.set_grid_visible(false);
engine.set_light_helpers_visible(false);
engine.setBackgroundColor([.001,.002,.004]);
engine.postprocessing.enable('bloom',{strength:.8});
const fit=()=>engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
fit();window.addEventListener('resize',fit);

const descriptions={
    'drone-orbiter':['DRONE ORBITER','Lehký / rychlý · pohyblivá energetická křídla'],
    'blade-wraith':['BLADE WRAITH','Střední / agresivní · dvojitý sek energetickými čepelemi'],
    'core-tank':['CORE TANK','Těžký / odolný · nabití, palba a zpětný ráz']
};
const actions={
    'drone-orbiter':{
        Idle:'Hlídka · reaktor a stabilizátory v pohybu.',Run:'Let vpřed · křídla se naklánějí do letu.',
        Attack:'Rotační výpad a střemhlavý náraz.',Attack2:'Jedna plazmová střela.',Attack3:'Dávka tří párů střel.',Attack4:'Nabitá salva tří párů střel.',
        Explode:'Zásah → rozlet dílů → zničený dron.',Assemble:'Díly se vracejí a dron znovu ožívá.'
    },
    'blade-wraith':{
        Idle:'Hlídka · čepele pulzují energií.',Run:'Běžecká smyčka na místě · pohyb po scéně řídí hra.',
        Attack:'Dvojitý sek energetickými čepelemi.',Attack2:'Nadzvednutí čepelí → prudký svislý sek.',Attack3:'Výpad vpřed a bodnutí.',Attack4:'Rotační sek oběma čepelemi.',
        Explode:'Zásah → rozlet dílů → zničený Wraith.',Assemble:'Díly se vracejí a Wraith znovu ožívá.'
    },
    'core-tank':{
        Idle:'Hlídka · reaktor a stabilizátory v pohybu.',Run:'Vznášení vpřed · těžký tank se naklání do letu.',
        Attack:'Nabití děla → výstřel → zpětný ráz.',Attack2:'Dvojice plazmových střel.',Attack3:'Dávka tří párů střel.',Attack4:'Nabitá salva tří párů střel.',
        Explode:'Zásah → rozlet dílů → zničený tank.',Assemble:'Díly se vracejí a tank znovu ožívá.'
    }
};
let slug='drone-orbiter',color='pink',clip='Idle',speed=1,paused=false,repeat=false,checking=false;
/** @type {import('../../js/engine.js').ModelObject | undefined} */
export let model;
const status=document.getElementById('status');
const pause=document.getElementById('pause');
const timeline=document.getElementById('timeline');
const wide=()=>clip!=='Idle'&&clip!=='Run';
/** @returns {import('../../js/engine.js').Vector3} */
const target=()=>wide()?[0,1.4,1.3]:[0,1.4,0];
export const hero=()=>engine.setOrbitView(wide()?[5,3.4,10]:[2.6,2.05,6.25],target());
export const front=()=>engine.setOrbitView(wide()?[0,2.8,10]:[0,1.8,6.6],target());
export const rear=()=>engine.setOrbitView(wide()?[-5,3.4,-10]:[-2.6,2.05,-6.25],[0,1.4,0]);
hero();

/** @param {string} selector @param {string} value @param {string} key */
function mark(selector,value,key){
    document.querySelectorAll(selector).forEach(el=>el.setAttribute('aria-pressed',String(el instanceof HTMLElement&&el.dataset[key]===value)));
}
/** Refresh DOM controls from native animation state. Does not drive the animation. */
function reflectPlayback(){
    if(checking||!model)return;
    const info=model.info,playback=info.playback;
    const duration=info.animations.find(a=>a.name===clip)?.duration??1;
    if(timeline instanceof HTMLInputElement){timeline.max=String(duration);timeline.value=String(playback?.time??0);}
    const timeValue=document.getElementById('time-value');
    if(timeValue)timeValue.textContent=`${(playback?.time??0).toFixed(1)} / ${duration.toFixed(1)} s`;
    if(pause)pause.textContent=playback?.playing?'Pozastavit':paused?'Pokračovat':'Přehrát znovu';
}
/** @param {number} [time] */
function play(time=0){
    model?.playAnimation(clip,{loop:clip==='Idle'||clip==='Run'||repeat,speed}).seekAnimation(time).pauseAnimation(paused);
    mark('[data-clip]',clip,'clip');
    const description=document.getElementById('action-description');
    if(description)description.textContent=actions[/** @type {keyof typeof actions} */(slug)][/** @type {keyof typeof actions['drone-orbiter']} */(clip)];
    reflectPlayback();
}
/** @param {boolean} busy */
function lock(busy){
    document.querySelectorAll('button,input').forEach(el=>{if(el instanceof HTMLButtonElement||el instanceof HTMLInputElement)el.disabled=busy;});
}
/** @param {string} nextSlug @param {string} nextColor */
export async function select(nextSlug,nextColor){
    lock(true);if(status)status.textContent='Načítám model…';
    try{
        const next=await loadEnemy(engine.scene,nextSlug,nextColor);
        const same=slug===nextSlug,time=same?(model?.info.playback?.time??0):0;
        model?.remove();model=next;slug=nextSlug;color=nextColor;
        if(!same){clip='Idle';paused=false;hero();}
        play(time);
        mark('[data-model]',slug,'model');mark('[data-color]',color,'color');
        const info=descriptions[/** @type {keyof typeof descriptions} */(slug)];
        const title=document.getElementById('title'),subtitle=document.getElementById('subtitle');
        if(title)title.textContent=info[0];if(subtitle)subtitle.textContent=info[1];
        document.querySelectorAll('[data-clip]').forEach(el=>{
            if(el instanceof HTMLButtonElement){const name=el.dataset.clip??'Idle';el.textContent=name==='Idle'?'Hlídka':name==='Run'?(slug==='blade-wraith'?'Běh':slug==='drone-orbiter'?'Let':'Přesun'):name==='Attack'?(slug==='blade-wraith'?'Dvojitý sek':slug==='drone-orbiter'?'Výpad':'Výstřel'):name==='Attack2'?(slug==='blade-wraith'?'Svislý sek':slug==='drone-orbiter'?'1 střela':'2 střely'):name==='Attack3'?(slug==='blade-wraith'?'Výpad':'3 páry'):name==='Attack4'?(slug==='blade-wraith'?'Rotační sek':'Nabitá salva'):name==='Explode'?'Zničení':'Sestavení';}
        });
        const download=document.getElementById('download');
        if(download instanceof HTMLAnchorElement)download.href=new URL(`../../assets/models/neon-enemies/${slug}-${color}.glb`,import.meta.url).href;
        if(status)status.textContent=`${model.info.animations.length} pohybové sekvence · ${engine.renderer_backend()==='Gl'?'WebGL':'WebGPU'}`;
    }finally{lock(false);}
}
function report(/** @type {unknown} */error){if(status)status.textContent=`Chyba: ${error}`;console.error(error);}
lock(true);
try{
    await createScene(engine.scene);await select(slug,color);
    document.documentElement.dataset.rendererBackend=engine.renderer_backend();
}catch(error){report(error);throw error;}

document.querySelectorAll('[data-model]').forEach(el=>{
    if(el instanceof HTMLButtonElement)el.onclick=()=>select(el.dataset.model??slug,el.dataset.model==='core-tank'?'amber':'pink').catch(report);
});
document.querySelectorAll('[data-color]').forEach(el=>{
    if(el instanceof HTMLButtonElement)el.onclick=()=>select(slug,el.dataset.color??color).catch(report);
});
document.querySelectorAll('[data-clip]').forEach(el=>{
    if(el instanceof HTMLButtonElement)el.onclick=()=>{clip=el.dataset.clip??'Idle';paused=false;play();hero();};
});
if(pause instanceof HTMLButtonElement)pause.onclick=()=>{
    if(!model)return;
    const playback=model.info.playback;
    if(!playback?.playing&&!paused){play();return;}
    paused=!paused;model.pauseAnimation(paused);reflectPlayback();
};
for(const [id,view]of [['front',front],['back',rear],['hero',hero]]){
    const button=document.getElementById(/** @type {string} */(id));
    if(button instanceof HTMLButtonElement)button.onclick=/** @type {()=>void} */(view);
}
const speedControl=document.getElementById('speed');
if(speedControl instanceof HTMLInputElement)speedControl.oninput=()=>{
    speed=Number(speedControl.value);play(model?.info.playback?.time??0);
    const output=document.getElementById('speed-value');if(output)output.textContent=`${speed}×`;
};
const repeatControl=document.getElementById('repeat');
if(repeatControl instanceof HTMLInputElement)repeatControl.onchange=()=>{repeat=repeatControl.checked;play(model?.info.playback?.time??0);};
if(timeline instanceof HTMLInputElement)timeline.oninput=()=>{
    paused=true;model?.pauseAnimation().seekAnimation(Number(timeline.value));reflectPlayback();
};
// DOM polling only: Rust continues sampling and rendering without this timer.
const playbackDisplay=setInterval(reflectPlayback,200);
window.addEventListener('pagehide',()=>{
    clearInterval(playbackDisplay);window.removeEventListener('resize',fit);engine.dispose();
},{once:true});
const verify=document.getElementById('verify');
if(verify instanceof HTMLButtonElement)verify.onclick=async()=>{
    lock(true);checking=true;
    const result=document.getElementById('result');if(result)result.textContent='Ověřuji…';
    model?.setPosition([100,0,0]);
    const savedClip=clip;clip='Idle';
    try{
        const {verifyEnemies}=await import('./verify.js');
        await verifyEnemies(engine,canvas,front,rear);
        if(result)result.textContent='PASS: tři GLB, čtyři sekvence, fáze útoku, rozpad, sestavení, barvy a zadní geometrie.';
    }catch(error){if(result)result.textContent=`FAIL: ${error}`;console.error(error);}
    finally{clip=savedClip;model?.setPosition([0,0,0]);hero();checking=false;reflectPlayback();lock(false);}
};
if(parameters.has('verify'))document.querySelector('details')?.setAttribute('open','');
