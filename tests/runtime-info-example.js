// Each runner entry loads the production HTML and real module of one example.
import {installXRSimulator} from './xr/simulator.js';
import {XRTestRun,publishXRReport} from './xr/testkit.js';
import {assertChanged} from './pixels.js';
import {installRuntimeInfo} from '../examples/shared/runtime-info.js';
import {VR_DEBUG_PANEL_OFFSET} from '../examples/shared/vr-debug-panel.js';
const scenes=['basic','culling','cutting','cynatar','fox-control','gltf','lights','mirror','monkey','neon-arena','neon-enemies','neon-font-preview','neon-robot','postprocessing','projectiles','rim','shadows','webxr','forest-arena','crystal-cave','monster-room','table-tennis'];
const name=new URLSearchParams(location.search).get('scene')??'basic';
if(!scenes.includes(name))throw Error('Unknown example '+name);
const sim=installXRSimulator({profile:{id:'example-info-'+name,hz:72,eyeWidth:320,eyeHeight:320},clockTimers:false});
const run=new XRTestRun('example-info-'+name,sim,publishXRReport);
let engine;
await run.execute(async test=>{
    const response=await fetch('../examples/'+name+'/');if(!response.ok)throw Error('Example HTML unavailable');
    const source=new DOMParser().parseFromString(await response.text(),'text/html');
    for(const script of source.querySelectorAll('script'))script.remove();
    const styles=[];
    for(const node of source.head.querySelectorAll('style,link[rel="stylesheet"],meta[name="build-profile"]')){
        if(node instanceof HTMLLinkElement){
            node.href=new URL(node.getAttribute('href'),response.url).href;
            styles.push(new Promise((resolve,reject)=>{node.onload=resolve;node.onerror=()=>reject(Error('Example stylesheet unavailable: '+node.href));}));
        }
        document.head.append(node);
    }
    await Promise.all(styles);
    document.body.append(...source.body.childNodes);
    if(!document.querySelector('#result')){const result=document.createElement('pre');result.id='result';document.body.append(result);}
    const main=await import('../examples/'+name+'/main.js');engine=main.engine;
    test.assert(Boolean(engine),'actual example exports its running engine');
    document.documentElement.dataset.rendererBackend=engine.renderer_backend();
    await main.sceneReady;
    test.assert(!engine.frameProfile().enabled,'actual example defaults to profiling off');
    test.assert(!document.querySelector('[data-runtime-info]'),'no desktop info panel covers entry UI');
    const ui=installRuntimeInfo(engine);
    await engine.xr.enter();
    const yaw=-Math.atan2(.9,1),pitch=-Math.atan2(1.25,Math.hypot(.9,1));
    const sp=Math.sin(pitch/2),cp=Math.cos(pitch/2),sy=Math.sin(yaw/2),cy=Math.cos(yaw/2);
    // Studio characters occupy the authoring origin: stand in front of the model,
    // rather than placing the simulated eyes inside its opaque torso.
    const headPosition=['neon-robot','neon-enemies'].includes(name)?[0,1.6,3]:[0,1.6,0];
    const frame=()=>{sim.frame({dt:0,head:{tracked:true,position:headPosition,orientation:[cy*sp,sy*cp,-sy*sp,cy*cp]}});test.assert(!engine.xr.lastError,'real example renders simulated XR');};
    frame();frame();
    const panel=engine.xr.player.head.object;
    test.assert(Boolean(panel),'actual example binds one native information panel');
    test.assert(panel.meshInfo.scale.every(v=>v===1),'actual native panel is not mirrored');
    const head=engine.xr.player.state.head.matrix,origin=engine.xr.player.state.locomotion.origin,[x,y,z]=VR_DEBUG_PANEL_OFFSET;
    const expected=[head[12]+origin[0]*x+origin[8]*z,origin[13]+y,head[14]+origin[2]*x+origin[10]*z];
    test.assert(panel.meshInfo.position.every((v,i)=>Math.abs(v-expected[i])<.004),'actual example uses shared player-floor offset');
    const left=sim.pixels('left'),right=sim.pixels('right');
    ui.setVisible(false);frame();
    test.assert(engine.xr.player.head.object===null&&panel.meshInfo.position[1]<-9000,'actual example hides and releases panel');
    assertChanged(left,sim.pixels('left'),'actual information appears in left eye',30);
    assertChanged(right,sim.pixels('right'),'actual information appears in right eye',30);
    ui.setVisible(true);frame();
    test.assert(engine.xr.player.head.object?.id===panel.id,'actual example reuses its native node');
    await engine.xr.exit();test.assert(engine.xr.player.head.object===null,'actual example exit releases panel');
    await engine.xr.enter();frame();test.assert(engine.xr.player.head.object?.id===panel.id,'actual example reentry restores the same panel');
    await engine.xr.exit();
});
// Exercise the real example cleanup before the idempotent final safeguard.
window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:false}));
engine?.dispose();
