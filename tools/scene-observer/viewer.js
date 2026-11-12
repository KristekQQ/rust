import {initEngine} from '../../js/engine.js';
import {WireCache,decodePacket,decodeRecording} from './protocol.mjs';
import {createHelmet,createNativeSwords} from './scene.js';
import {decodeByteDelta,isByteDelta} from './byte-delta.mjs';
import {InterpolationCadence} from './interpolation-policy.mjs';
const params=new URLSearchParams(location.search),canvas=document.querySelector('canvas'),veil=document.querySelector('#veil');
let engine,helmet,socket,session=null,pending=null,initializing=false,closed=false,replay=null;
let requestedSession=null,initRequest=0,transportBase=null,fatalError=null,wireRebase=0n,appliedWireSequence=0n;
let cadence=new InterpolationCadence();
let presentation='waiting',staleTransitions=0,renderFrames=0;
let requestedNative=false,requestedScene=false,nativeMode=false,nativeSwords=[];
let camera=params.get('camera')??'side',lastMeta=null,lastApplied=0,offset=0,uncertainty=Infinity,receivedBytes=0,rateAt=performance.now(),rate=0;
const samples=[];
const notify=value=>{if(parent!==window)parent.postMessage(value,location.origin);};
function setCamera(name){
    camera=name;
    const views={side:[[3.4,2,1.8],[0,1.25,-1.1],'Z boku'],player:[[0,1.7,1.3],[0,1.4,-1.6],'Od hráče'],wide:[[4.2,3.5,5.5],[0,1.1,-1],'Celá aréna'],top:[[.05,6,.7],[0,.5,-1.2],'Shora']};
    const [position,target,label]=views[name]??views.side;engine?.setOrbitView(position,target);
    document.querySelector('#caption').textContent=label+' · tažením myši otoč kameru';
}
async function initScene(newSession,isNative=false,isScene=false){
    if(isScene&&engine&&!initializing&&session===newSession&&requestedScene===isScene){
        transportBase=null;fatalError=null;wireRebase=isScene?appliedWireSequence:0n;
        socket?.readyState===WebSocket.OPEN&&socket.send(JSON.stringify({type:'ready'}));return;
    }
    fatalError=null;wireRebase=0n;appliedWireSequence=0n;
    requestedSession=newSession;requestedNative=isNative;requestedScene=isScene;transportBase=null;initRequest++;
    if(initializing)return;
    initializing=true;
    try{
      while(!closed){
        const request=initRequest;
        nativeMode=requestedNative;
        veil.hidden=false;veil.textContent='Připravuji scénu…';session=requestedSession;pending=null;lastMeta=null;
        engine?.dispose();
        engine=await initEngine(canvas,{backend:params.get('backend')==='webgl'?'webgl':'auto'});
        engine.setCameraHandedness('right');
        document.documentElement.dataset.rendererBackend=engine.renderer_backend();
        engine.setDebugOverlay(false);engine.set_grid_visible(true);engine.set_light_helpers_visible(false);engine.set_render_effects(false,false);
        engine.postprocessing.disable('ssr');engine.postprocessing.disable('bloom');engine.postprocessing.disable('fxaa');
        engine.setBackgroundColor([.018,.025,.04]);engine.setNeonGlow({strength:.4,radius:.045});
        cadence=new InterpolationCadence();engine.setSceneInterpolation({durationMs:60,maxDistance:2});
        helmet=requestedScene?null:createHelmet(engine.scene);
        nativeSwords=nativeMode?createNativeSwords(engine.scene):[];
        engine.setSceneAuthoringPaused(false);setCamera(camera);
        if(request!==initRequest)continue;
        if(replay)replay.ready=true;else socket?.readyState===WebSocket.OPEN&&socket.send(JSON.stringify({type:'ready'}));
        break;
      }
    }finally{initializing=false;}
}
function fail(error){fatalError=String(error);veil.hidden=false;veil.textContent=String(error);notify({type:'error',error:String(error)});console.error(error);}
function connect(){
    const endpoint=new URL('/observer?role=viewer',location.href);endpoint.protocol=location.protocol==='https:'?'wss:':'ws:';
    if(params.get('source')==='native'){endpoint.searchParams.set('source','native');endpoint.searchParams.set('encoding','ndp1');}
    transportBase=null;socket=new WebSocket(endpoint);socket.binaryType='arraybuffer';
    socket.onopen=()=>socket.send(JSON.stringify({type:'clock',sentAt:Date.now()}));
    socket.onmessage=({data})=>{
        if(replay)return;
        try{
            if(typeof data==='string'){
                const value=JSON.parse(data);
                if(value.type==='clock'){const now=Date.now();offset=value.serverAt-(value.sentAt+now)/2;uncertainty=(now-value.sentAt)/2;}
                else if(value.type==='session')void initScene(value.session,value.native===true&&!value.scene,value.scene===true).catch(fail);
                else if(value.type==='pose-frame'){receivedBytes+=data.length;pending={meta:value.meta,native:true};}
                else if(value.type==='error')fail(value.error);
            }else{
                receivedBytes+=data.byteLength;
                const packet=new Uint8Array(data);
                if(isByteDelta(packet)){transportBase=decodeByteDelta(transportBase,packet);pending=decodePacket(transportBase.bytes);}
                else pending=decodePacket(packet);
                if(pending.meta.session!==session)throw Error('Stav z jiné relace');
            }
        }catch(error){fail(error);socket.close();}
    };
    socket.onclose=()=>{if(!closed&&!replay)setTimeout(connect,1000);};
}
function age(){return lastMeta?Math.max(0,Date.now()+offset-lastMeta.sampledAt):Infinity;}
function renderUpdate(now){
    if(closed)return;
    try{
        if(replay?.ready){
            const position=replay.startTime===null?0:now-replay.startTime;
            if(replay.startTime===null)replay.startTime=now;
            let latest=null;
            while(replay.index<replay.packets.length&&replay.packets[replay.index].meta.sampledAt-replay.firstAt<=position){
                latest=replay.packets[replay.index++];replay.cache.ingest(latest.wire);
            }
            if(latest)pending={meta:latest.meta,wire:replay.cache.packetFor(replay.peer)};
        }
        if(pending&&!initializing){
            const frame=pending;pending=null;
            const started=performance.now();
            if(frame.native){
                for(const [i,name] of ['left','right'].entries()){
                    const pose=frame.meta.state[name];
                    if(pose)nativeSwords[i].setPosition(pose.position).setRotation(pose.rotation);
                    else nativeSwords[i].setPosition([0,-10000,0]);
                }
            }else {
                let wire=frame.wire;
                if(wireRebase){wire=wire.slice();const header=new DataView(wire.buffer,wire.byteOffset,wire.byteLength);header.setBigUint64(8,header.getBigUint64(8,true)+wireRebase,true);}
                const duration=cadence.sample(frame.meta.sampledAt);
                if(duration!==null)engine.setSceneInterpolation({durationMs:duration,maxDistance:2});
                engine.applySimulationPublication(wire,helmet?[helmet]:[]);
                document.documentElement.dataset.interpolationMs=String(cadence.duration);
                appliedWireSequence=new DataView(wire.buffer,wire.byteOffset,wire.byteLength).getBigUint64(8,true);
            }
            const head=frame.meta.state?.head;
            if(head&&helmet){
                if(!Array.isArray(head.position)||!Array.isArray(head.rotation)||head.position.length!==3||head.rotation.length!==3||![...head.position,...head.rotation].every(Number.isFinite))throw Error('Neplatná póza hlavy');
                helmet.setPosition(head.position).setRotation(head.rotation);
            }else helmet?.setPosition([0,-10000,0]);
            lastMeta=frame.meta;lastApplied=performance.now();
            const ageMs=age(),bound=uncertainty+(frame.meta.clockUncertaintyMs??0);
            if(!replay){samples.push({ageMs,uncertaintyMs:bound,applyMs:lastApplied-started,at:now});if(samples.length>1800)samples.shift();socket.send(JSON.stringify({type:'ack'}));}
        }
        const elapsed=now-rateAt;if(elapsed>=1000){rate=receivedBytes/1024/(elapsed/1000);receivedBytes=0;rateAt=now;}
        const sceneAge=age(),bound=uncertainty+(lastMeta?.clockUncertaintyMs??0);
        const nextPresentation=fatalError?'error':!lastMeta?'waiting':replay?'replay':sceneAge+bound>1500?'paused':sceneAge+bound>250?'delayed':'live';
        if(nextPresentation!==presentation&&nextPresentation==='paused')staleTransitions++;
        presentation=nextPresentation;renderFrames++;
        const stale=presentation==='paused'||presentation==='waiting';
        // A brief transport delay must not cover or destroy the last rendered scene.
        veil.hidden=!!lastMeta&&!fatalError;
        if(fatalError)veil.textContent=fatalError;else if(!lastMeta&&!initializing)veil.textContent='Čeká na zdroj scény…';
        let badge=document.querySelector('#delivery-state');
        if(!badge){badge=document.createElement('div');badge.id='delivery-state';badge.setAttribute('role','status');document.body.append(badge);}
        badge.hidden=!lastMeta||!['paused','delayed'].includes(presentation);
        badge.textContent=presentation==='paused'?'Pozastaveno · poslední přijatý stav':'Zpoždění přenosu · poslední přijatý stav';
        if(now-(renderUpdate.lastDiagnostics??0)>1000){
            renderUpdate.lastDiagnostics=now;
            const recent=samples.slice(-120),sorted=key=>recent.map(v=>v[key]).sort((a,b)=>a-b),ages=sorted('ageMs'),applies=sorted('applyMs');
            const gaps=recent.slice(1).map((v,i)=>v.at-recent[i].at).sort((a,b)=>a-b);
            document.documentElement.dataset.observerDiagnostics=JSON.stringify({presentation,interpolationMs:60,staleTransitions,renderFrames,samples:recent.length,ageP50:ages[Math.floor(ages.length*.5)],ageP95:ages[Math.floor(ages.length*.95)],applyP95:applies[Math.floor(applies.length*.95)],gapP50:gaps[Math.floor(gaps.length*.5)],gapP95:gaps[Math.floor(gaps.length*.95)],kibPerSec:rate});
        }
        if(lastMeta&&now-(renderUpdate.lastReport??0)>100){
            renderUpdate.lastReport=now;
            notify({type:'observer-metrics',presentation,ageMs:replay?null:sceneAge,uncertaintyMs:Number.isFinite(bound)?bound:null,
                stale,replay:!!replay,mode:lastMeta.mode,state:lastMeta.state,kibPerSec:rate});
        }
    }catch(error){fail(error);pending=null;}
    requestAnimationFrame(renderUpdate);
}
window.addEventListener('message',event=>{
    if(event.origin!==location.origin)return;
    if(event.data.type==='camera')setCamera(event.data.camera);
    else if(event.data.type==='replay')void loadReplay(event.data.bytes).catch(fail);
});
async function loadReplay(bytes){
    const raw=decodeRecording(new Uint8Array(bytes));
    const packets=raw.map(packet=>decodePacket(packet));if(!packets.length)throw Error('Prázdný záznam');
    replay={packets,index:0,firstAt:packets[0].meta.sampledAt,startTime:null,cache:new WireCache(),peer:{sequence:0,known:[new Map(),new Map(),new Map()]},ready:false};
    socket?.close();await initScene(packets[0].meta.session);
}
globalThis.sceneObserver={get engine(){return engine;},get helmet(){return helmet;},get swords(){return nativeSwords;},get meta(){return lastMeta;},get stale(){return ['waiting','paused'].includes(presentation);},get camera(){return camera;},get samples(){return samples;},get replay(){return replay;},setCamera,loadReplay};
window.addEventListener('pagehide',()=>{closed=true;socket?.close();engine?.dispose();},{once:true});
connect();requestAnimationFrame(renderUpdate);
