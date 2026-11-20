import {BIRD_FLIGHTS,FEATHER_SPECIES} from '../../examples/neon-arena/bird-flight.js';
import {createArenaCabinet} from '../../examples/neon-arena/cabinet.js';
import {installAudioControls} from '../../examples/shared/audio-controls.js';
import {initEngine} from '../../js/engine.js';
import {WireCache,decodePacket,decodeRecording} from './protocol.mjs';
import {createHelmet,createNativeSwords} from './scene.js';
import {decodeByteDelta,isByteDelta} from './byte-delta.mjs';
import {InterpolationCadence} from './interpolation-policy.mjs';
import {SceneInbox} from './scene-inbox.mjs';
const params=new URLSearchParams(location.search),canvas=document.querySelector('canvas'),veil=document.querySelector('#veil');
let engine,helmet,socket,session=null,pending=null,initializing=false,closed=false,replay=null;
let requestedSession=null,initRequest=0,transportBase=null,fatalError=null,appliedWireSequence=0n,appliedRevisions=null;
let inbox=new SceneInbox(),firstSnapshot=false,reconnectTimer=null,lastMessageAt=0;
let cadence=new InterpolationCadence();
const PLAYBACK_DELAY_MS=50,PLAYBACK_EXTRAPOLATION_MS=80;
let playbackMetadata=[],playbackState=null;
const bufferedPlayback=()=>requestedScene&&!replay;
const sourceTime=meta=>meta.sourceSampledAt??meta.sampledAt;
function presentPlaybackMetadata(state){
    const presented=BigInt(state.sequence);
    while(playbackMetadata.length&&playbackMetadata[0].sequence<=presented){
        const frame=playbackMetadata.shift();
        appliedWireSequence=frame.sequence;appliedRevisions=frame.revisions;
        lastMeta=frame.meta;lastApplied=performance.now();
        audio.mixer?.ingest(lastMeta.state?.audio,{suppressInitial:true,transportAge:Math.max(0,(state.timeMs-sourceTime(lastMeta))/1000)});
    }
    // Rust can advance after the previous JS RAF. Rebase from the actual commit.
    appliedWireSequence=presented;
}
function resetPlayback(enabled){
    if(engine){const state=engine.scenePlaybackState();if(state.enabled)presentPlaybackMetadata(state);}
    playbackMetadata=[];playbackState=null;
    engine?.setScenePlayback({delayMs:0});
    if(enabled){
        engine.setScenePrediction({horizonMs:0});
        engine.setSceneInterpolation({durationMs:0,maxDistance:2});
        engine.setScenePlayback({delayMs:PLAYBACK_DELAY_MS,maxExtrapolationMs:PLAYBACK_EXTRAPOLATION_MS,maxDistance:2});
    }
}
let glowEnabled=true,debugBounds=null,debugBoundsEnabled=false;
let presentation='waiting',staleTransitions=0,renderFrames=0;
let requestedNative=false,requestedScene=false,nativeMode=false,nativeSwords=[];
let camera=params.get('camera')??'side',lastMeta=null,lastApplied=0,offset=0,uncertainty=Infinity,receivedBytes=0,rateAt=performance.now(),rate=0;
const samples=[],frameGaps=[],receiptCosts=[],renderSpikes=[];
let lastRenderAt=null;
const audio=installAudioControls();
const notify=value=>{if(parent!==window)parent.postMessage(value,location.origin);};
// CSS size, camera projection and GPU attachments must change together.
// Checking before presentation also catches DPR changes when moving displays.
function resizeView(){
    if(!engine||initializing)return;
    const width=canvas.clientWidth,height=canvas.clientHeight;
    if(width<=0||height<=0)return;
    const ratio=window.devicePixelRatio||1;
    const pixelsWide=Math.max(1,Math.round(width*ratio));
    const pixelsHigh=Math.max(1,Math.round(height*ratio));
    if(canvas.width!==pixelsWide||canvas.height!==pixelsHigh)engine.resize(pixelsWide,pixelsHigh);
}
function setCamera(name){
    camera=name;
    const views={side:[[3.4,2,1.8],[0,1.25,-1.1],'Z boku'],player:[[0,1.7,1.3],[0,1.4,-1.6],'Od hráče'],wide:[[4.2,3.5,5.5],[0,1.1,-1],'Celá aréna'],top:[[.05,6,.7],[0,.5,-1.2],'Shora']};
    const [position,target,label]=views[name]??views.side;engine?.setOrbitView(position,target);
    document.querySelector('#caption').textContent=label+' · tažením myši otoč kameru';
}
async function initScene(newSession,isNative=false,isScene=false){
    resetPlayback(false);engine?.setScenePrediction({horizonMs:0});
    if(isScene&&engine&&!initializing&&requestedScene===isScene){
        audio.reset();transportBase=null;fatalError=null;pending=null;
        session=requestedSession=newSession;cadence=new InterpolationCadence();firstSnapshot=true;
        inbox=new SceneInbox({sequence:appliedWireSequence,revisions:appliedRevisions,buffered:!replay});
        resetPlayback(!replay);
        socket?.readyState===WebSocket.OPEN&&socket.send(JSON.stringify({type:'ready'}));return;
    }
    audio.reset();fatalError=null;appliedWireSequence=0n;appliedRevisions=null;
    inbox=new SceneInbox({buffered:isScene&&!replay});firstSnapshot=true;
    requestedSession=newSession;requestedNative=isNative;requestedScene=isScene;transportBase=null;initRequest++;
    if(initializing)return;
    initializing=true;
    try{
      while(!closed){
        const request=initRequest;
        nativeMode=requestedNative;
        veil.hidden=false;veil.textContent='Připravuji scénu…';session=requestedSession;pending=null;lastMeta=null;
        debugBounds?.dispose();debugBounds=null;
        engine?.dispose();
        engine=await initEngine(canvas,{backend:params.get('backend')==='webgl'?'webgl':'auto'});
        if(typeof engine.debugSceneBounds==='function'){
            const {installDebugBounds}=await import('./debug-bounds.mjs');
            debugBounds=installDebugBounds(engine);debugBounds?.setEnabled(debugBoundsEnabled);
        }
        notify({type:'observer-debug-capabilities',bounds:!!debugBounds});
        engine.setCameraHandedness('right');
        document.documentElement.dataset.rendererBackend=engine.renderer_backend();
        engine.setDebugOverlay(false);engine.set_grid_visible(!requestedScene);engine.set_light_helpers_visible(false);engine.set_render_effects(false,false);
        engine.postprocessing.disable('ssr');engine.postprocessing.disable('bloom');engine.postprocessing.disable('fxaa');
        engine.setBackgroundColor([0,0,0]);engine.setNeonGlow({strength:glowEnabled ? .4 : 0,radius:.045});
        // Match the native Neon spotlight profile for authoritative scene playback.
        // RenderPublication carries scene objects, not renderer/postprocess settings.
        // Keep this explicit until the publisher transmits its render profile.
        if(requestedScene&&params.get('source')==='native'){
            const assetRoot=new URL('./assets/neon-spotlight/',import.meta.url);
            const [profileResponse,maskResponse]=await Promise.all([
                fetch(new URL('neon-spotlight-profile.json',assetRoot)),
                fetch(new URL('neon-spotlight-mask.bin',assetRoot))]);
            if(!profileResponse.ok||!maskResponse.ok)throw Error('Chybí společný profil osvětlení');
            const profile=await profileResponse.json(),maskPixels=new Uint8Array(await maskResponse.arrayBuffer());
            const floor=await engine.scene.loadModel(new URL('neon-spotlight.gltf',assetRoot));
            floor.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,reflectivity:.94});
            await createArenaCabinet(engine.scene,new URL('./assets/apollo-stellar/apollo-stellar.glb',import.meta.url));
            const cat=await engine.scene.loadModel(new URL('./assets/stealth-cat/stealth-cat.gltf',import.meta.url));
            cat.bindSprite(1);
            for(const [i,name] of FEATHER_SPECIES.entries()) {
                const feathers=await engine.scene.loadModel(new URL('./assets/slot-birds/slot-bird-feathers-'+name+'.gltf',import.meta.url));
                for(const {config} of BIRD_FLIGHTS.filter(f=>f.config.cutEffectKey===900+i)) feathers.createInstance().bindSprite(config.key+3000);
                feathers.bindSprite(900+i);
            }
            const birdTemplates=new Map();
            for(const flight of BIRD_FLIGHTS){
                const name=flight.asset.split('/').pop();
                let template=birdTemplates.get(name);
                if(!template){
                    template=await engine.scene.loadModel(new URL('./assets/slot-birds/'+name,import.meta.url));
                    template.setScale([0.000001,0.000001,0.000001]);
                    birdTemplates.set(name,template);
                }
                // bindSprite consumes the instance root; retain a hidden source for later copies.
                for(const offset of [0,1000,2000]) {
                    const bird=template.createInstance();
                    bird.bindSprite(flight.config.key+offset);
                }
            }
            for(const template of birdTemplates.values())template.remove();
            engine.setNeonGlowResolution(.5);
            engine.setFontGlowAtlas(true);
            engine.prewarmFontGlowAtlas('0123456789+-');
            engine.postprocessing.configureSSR(profile.ssr);
            engine.postprocessing.setReflectionMask({...profile.mask,pixels:maskPixels});
            engine.postprocessing.enable('ssr',{strength:profile.ssr.strength});
            document.documentElement.dataset.lightingProfile='neon-spotlight-0.13.31';
        }
        cadence=new InterpolationCadence();engine.setSceneInterpolation({durationMs:60,maxDistance:2});
        helmet=requestedScene?null:createHelmet(engine.scene);
        nativeSwords=nativeMode?createNativeSwords(engine.scene):[];
        engine.setSceneAuthoringPaused(false);setCamera(camera);resetPlayback(bufferedPlayback());
        if(request!==initRequest)continue;
        if(replay)replay.ready=true;else socket?.readyState===WebSocket.OPEN&&socket.send(JSON.stringify({type:'ready'}));
        break;
      }
    }finally{initializing=false;}
}
function fail(error){fatalError=String(error);veil.hidden=false;veil.textContent=String(error);notify({type:'error',error:String(error)});console.error(error);}
function reconnect(){
    const old=socket;socket=null;
    if(old){old.onclose=null;old.onmessage=null;old.close();}
    if(!closed&&!replay&&reconnectTimer===null)reconnectTimer=setTimeout(()=>{reconnectTimer=null;connect();},250);
}
function connect(){
    if(closed||replay)return;
    const endpoint=new URL('/observer?role=viewer',location.href);endpoint.protocol=location.protocol==='https:'?'wss:':'ws:';
    if(params.get('source')==='native'){endpoint.searchParams.set('source','native');endpoint.searchParams.set('encoding','ndp1');}
    transportBase=null;const ws=socket=new WebSocket(endpoint);ws.binaryType='arraybuffer';lastMessageAt=Date.now();
    ws.onopen=()=>{if(socket===ws)ws.send(JSON.stringify({type:'clock',sentAt:Date.now()}));};
    ws.onmessage=({data})=>{
        if(replay||socket!==ws)return;
        lastMessageAt=Date.now();
        try{
            if(typeof data==='string'){
                const value=JSON.parse(data);
                if(value.type==='clock'){const now=Date.now();offset=value.serverAt-(value.sentAt+now)/2;uncertainty=(now-value.sentAt)/2;}
                else if(value.type==='session')void initScene(value.session,value.native===true&&!value.scene,value.scene===true).catch(error=>{fail(error);reconnect();});
                else if(value.type==='pose-frame'){
                    receivedBytes+=data.length;pending={meta:value.meta,native:true};
                    ws.send(JSON.stringify({type:'ack'}));
                }
                else if(value.type==='error')throw Error(value.error??value.message);
            }else{
                const receiptStart=performance.now();
                receivedBytes+=data.byteLength;
                const packet=new Uint8Array(data);let frame;
                if(isByteDelta(packet)){transportBase=decodeByteDelta(transportBase,packet);frame=decodePacket(transportBase.bytes);}
                else frame=decodePacket(packet);
                if(frame.meta.session!==session)throw Error('Stav z jiné relace');
                inbox.receive(frame);
                receiptCosts.push({at:Date.now(),ms:performance.now()-receiptStart,bytes:data.byteLength});
                if(receiptCosts.length>120)receiptCosts.shift();
                ws.send(JSON.stringify({type:'ack'}));
            }
        }catch(error){fail(error);reconnect();}
    };
    ws.onclose=()=>{if(socket===ws)reconnect();};
}
// Data may legitimately stop with tracking. A clock reply distinguishes that
// from a silent dead connection; do not wait for a TCP close handshake to retry.
const heartbeat=setInterval(()=>{
    if(closed||replay||!socket)return;
    if(Date.now()-lastMessageAt>6000){reconnect();return;}
    if(socket.readyState===WebSocket.OPEN)socket.send(JSON.stringify({type:'clock',sentAt:Date.now()}));
},2000);
function age(){return lastMeta?Math.max(0,Date.now()+offset-lastMeta.sampledAt):Infinity;}
function renderUpdate(now){
    if(closed)return;
    if(lastRenderAt!==null){const gap=now-lastRenderAt;frameGaps.push(gap);if(frameGaps.length>120)frameGaps.shift();if(gap>40&&lastMeta){renderSpikes.push({at:Date.now(),gap,age:age(),shots:lastMeta.state?.round?.shots,receiptMs:receiptCosts.at(-1)?.ms,applyMs:samples.at(-1)?.applyMs});if(renderSpikes.length>30)renderSpikes.shift();}}
    lastRenderAt=now;
    try{
        resizeView();
        debugBounds?.update(now);
        if(audio.mixer&&engine){
            const view=engine.cameraViewState(),position=view.position,target=view.orbitTarget;
            if(target)audio.mixer.listener.setPose(position,target.map((v,i)=>v-position[i]));
        }
        if(replay?.ready){
            const position=replay.startTime===null?0:now-replay.startTime;
            if(replay.startTime===null)replay.startTime=now;
            let latest=null;
            while(replay.index<replay.packets.length&&replay.packets[replay.index].meta.sampledAt-replay.firstAt<=position){
                latest=replay.packets[replay.index++];replay.cache.ingest(latest.wire);
            }
            if(latest)pending={meta:latest.meta,wire:replay.cache.packetFor(replay.peer)};
        }
        if(!replay&&!initializing){
            if(bufferedPlayback()){
                let frame;
                while((frame=inbox.take())){
                    const started=performance.now();
                    engine.enqueueScenePublication(frame.wire,sourceTime(frame.meta));
                    const header=new DataView(frame.wire.buffer,frame.wire.byteOffset,frame.wire.byteLength);
                    playbackMetadata.push({sequence:header.getBigUint64(8,true),revisions:[16,24,32,40].map(at=>header.getBigUint64(at,true)),meta:frame.meta});
                    if(playbackMetadata.length>256)throw Error('Observer metadata backlog exceeded');
                    samples.push({ageMs:Math.max(0,Date.now()+offset-frame.meta.sampledAt),uncertaintyMs:uncertainty+(frame.meta.clockUncertaintyMs??0),applyMs:performance.now()-started,at:now});
                    if(samples.length>1800)samples.shift();
                }
                playbackState=engine.scenePlaybackState();
                if(playbackState.error)throw Error(playbackState.error);
                // Scene metadata and sound commit on the same delayed timeline as geometry.
                presentPlaybackMetadata(playbackState);
                document.documentElement.dataset.interpolationMs=String(PLAYBACK_DELAY_MS);
            }else{const latest=inbox.take();if(latest)pending=latest;}
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
                if(firstSnapshot)engine.setSceneInterpolation({durationMs:0,maxDistance:2});
                const duration=cadence.sample(frame.meta.sampledAt,replay?undefined:now);
                if(duration!==null&&!firstSnapshot)engine.setSceneInterpolation({durationMs:duration,maxDistance:2});
                engine.applySimulationPublication(wire,helmet?[helmet]:[]);
                if(firstSnapshot){firstSnapshot=false;engine.setSceneInterpolation({durationMs:cadence.duration,maxDistance:2});}
                document.documentElement.dataset.interpolationMs=String(cadence.duration);
                const header=new DataView(wire.buffer,wire.byteOffset,wire.byteLength);
                appliedWireSequence=header.getBigUint64(8,true);
                appliedRevisions=[16,24,32,40].map(at=>header.getBigUint64(at,true));
            }
            const head=frame.meta.state?.head;
            if(head&&helmet){
                if(!Array.isArray(head.position)||!Array.isArray(head.rotation)||head.position.length!==3||head.rotation.length!==3||![...head.position,...head.rotation].every(Number.isFinite))throw Error('Neplatná póza hlavy');
                helmet.setPosition(head.position).setRotation(head.rotation);
            }else helmet?.setPosition([0,-10000,0]);
            lastMeta=frame.meta;lastApplied=performance.now();
            audio.mixer?.ingest(frame.meta.state?.audio,{suppressInitial:!replay,delay:cadence.duration/1000,transportAge:replay?0:Math.max(0,Date.now()+offset-frame.meta.sampledAt)/1000});
            const ageMs=age(),bound=uncertainty+(frame.meta.clockUncertaintyMs??0);
            if(!replay){samples.push({ageMs,uncertaintyMs:bound,applyMs:lastApplied-started,at:now});if(samples.length>1800)samples.shift();}
        }
        const elapsed=now-rateAt;if(elapsed>=1000){rate=receivedBytes/1024/(elapsed/1000);receivedBytes=0;rateAt=now;}
        const sceneAge=age(),bound=uncertainty+(lastMeta?.clockUncertaintyMs??0);
        if(document.hidden||(!replay&&sceneAge>1500))audio.mixer?.stopAll();
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
        badge.dataset.state=presentation;
        const deliveryLabel=presentation==='paused'?'Přenos pozastaven · poslední zobrazený stav':bufferedPlayback()&&playbackState?.extrapolating?'Zpoždění přenosu · dopočítávám pohyb':'Zpoždění přenosu';
        badge.textContent=presentation==='paused'?'Ⅱ':'●';
        badge.title=deliveryLabel;badge.setAttribute('aria-label',deliveryLabel);
        badge.setAttribute('role','img');
        if(now-(renderUpdate.lastDiagnostics??0)>1000){
            renderUpdate.lastDiagnostics=now;
            const recent=samples.slice(-120),sorted=key=>recent.map(v=>v[key]).sort((a,b)=>a-b),ages=sorted('ageMs'),applies=sorted('applyMs');
            const gaps=recent.slice(1).map((v,i)=>v.at-recent[i].at).sort((a,b)=>a-b);
            document.documentElement.dataset.observerDiagnostics=JSON.stringify({sprite:debugBounds?engine.scene.spriteState(1):undefined,presentation,glowEnabled,renderSpikes,renderGapMax:Math.max(0,...frameGaps),receiptMaxMs:Math.max(0,...receiptCosts.map(v=>v.ms)),applyMaxMs:Math.max(0,...applies),interpolationMs:bufferedPlayback()?PLAYBACK_DELAY_MS:cadence.duration,predictionHorizonMs:bufferedPlayback()?PLAYBACK_EXTRAPOLATION_MS:0,playback:playbackState,staleTransitions,renderFrames,samples:recent.length,ageP50:ages[Math.floor(ages.length*.5)],ageP95:ages[Math.floor(ages.length*.95)],applyP95:applies[Math.floor(applies.length*.95)],gapP50:gaps[Math.floor(gaps.length*.5)],gapP95:gaps[Math.floor(gaps.length*.95)],kibPerSec:rate});
        }
        if(lastMeta&&now-(renderUpdate.lastReport??0)>100){
            renderUpdate.lastReport=now;
            notify({type:'observer-metrics',presentation,ageMs:replay?null:sceneAge,uncertaintyMs:Number.isFinite(bound)?bound:null,
                stale,replay:!!replay,mode:lastMeta.mode,state:lastMeta.state,kibPerSec:rate});
        }
    }catch(error){fail(error);pending=null;if(!replay)reconnect();}
    requestAnimationFrame(renderUpdate);
}
window.addEventListener('message',event=>{
    if(event.origin!==location.origin)return;
    if(event.data.type==='camera')setCamera(event.data.camera);
    else if(event.data.type==='glow'){glowEnabled=event.data.enabled===true;engine?.setNeonGlow({strength:glowEnabled ? .4 : 0,radius:.045});}
    else if(event.data.type==='debug-bounds'&&debugBounds){debugBoundsEnabled=event.data.enabled===true;debugBounds.setEnabled(debugBoundsEnabled);}
    else if(event.data.type==='replay')void loadReplay(event.data.bytes).catch(fail);
});
async function loadReplay(bytes){
    const raw=decodeRecording(new Uint8Array(bytes));
    const packets=raw.map(packet=>decodePacket(packet));if(!packets.length)throw Error('Prázdný záznam');
    replay={packets,index:0,firstAt:packets[0].meta.sampledAt,startTime:null,cache:new WireCache(),peer:{sequence:0,known:[new Map(),new Map(),new Map()]},ready:false};
    socket?.close();await initScene(packets[0].meta.session);
}
globalThis.sceneObserver={get engine(){return engine;},get helmet(){return helmet;},get swords(){return nativeSwords;},get meta(){return lastMeta;},get stale(){return ['waiting','paused'].includes(presentation);},get camera(){return camera;},get samples(){return samples;},get replay(){return replay;},get playback(){return playbackState;},setCamera,loadReplay};
window.addEventListener('pagehide',()=>{closed=true;clearInterval(heartbeat);clearTimeout(reconnectTimer);audio.dispose();socket?.close();engine?.dispose();},{once:true});
connect();requestAnimationFrame(renderUpdate);
