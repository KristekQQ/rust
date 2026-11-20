import {initEngine} from '../../js/engine.js';
import {createScene} from './scene.js';
const canvas=document.querySelector('canvas'),status=document.querySelector('#status');
if(!(canvas instanceof HTMLCanvasElement)||!(status instanceof HTMLElement))throw Error('Chybí canvas nebo stavový panel.');
try {
 const engine=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')==='webgl'?'webgl':'auto'});
 engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
 engine.setBackgroundColor([.018,.022,.036]);
 /** @type {Record<string,[import('../../js/engine.js').Vector3,import('../../js/engine.js').Vector3]>} */
 const views={front:[[2.8,2.2,4.2],[0,1.1,0]],back:[[-2.8,2.2,-4.2],[0,1.1,0]],detail:[[.8,1.6,2],[0,1.55,0]]};
 /** @param {string} key */
 const view=key=>{const v=views[key];if(v)engine.setOrbitView(v[0],v[1]);};view(new URLSearchParams(location.search).get('view')??'front');
 const fit=()=>engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
 const resize=new ResizeObserver(fit);resize.observe(canvas);fit();
 await createScene(engine.scene);
 view(new URLSearchParams(location.search).get('view')??'front');
 document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 for(const button of document.querySelectorAll('[data-view]')){if(button instanceof HTMLButtonElement){button.disabled=false;button.onclick=()=>view(button.dataset.view??'front');}}
 const fullscreen=document.querySelector('#fullscreen');
 if(fullscreen instanceof HTMLButtonElement)fullscreen.onclick=()=>{if(document.fullscreenElement)void document.exitFullscreen();else void document.querySelector('main')?.requestFullscreen();};
 status.textContent='Model připravený · táhni pro otočení';
 window.addEventListener('pagehide',()=>{resize.disconnect();engine.dispose();},{once:true});
}catch(error){status.textContent='Načtení selhalo: '+error;console.error(error);}
