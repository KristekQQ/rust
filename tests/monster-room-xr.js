import {initEngine} from '../js/engine.js';
import {createScene,PERFORMANCE_PANEL_OFFSET} from '../examples/monster-room/scene.js';
import {installXRSimulator} from './xr/simulator.js';
import {XRTestRun,publishXRReport,pixelDifference} from './xr/testkit.js';
import {assertForeground} from './pixels.js';
const sim=installXRSimulator({profile:{id:'monster-room-72',hz:72,eyeWidth:320,eyeHeight:320},clockTimers:false});
const run=new XRTestRun('monster-room-xr',sim,publishXRReport);
const near=(a,b,t=.002)=>a.length===b.length&&a.every((v,i)=>Math.abs(v-b[i])<t);
let engine;
await run.execute(async test=>{
    engine=await initEngine(document.querySelector('#gpu-canvas'),{backend:'webgl'});
    document.documentElement.dataset.rendererBackend=engine.renderer_backend();
    engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
    const world=await createScene(engine.scene);
    engine.xr.player.configureLocomotion({...world.locomotion,marker:world.marker});
    const hand=engine.scene.createCube([0,-10000,0]).setScale([.04,.08,.12]);engine.xr.player.equip('right',hand);
    await engine.xr.enter({resolutionScale:1});
    test.assert(engine.effects_stats().shadowLightCount===0,'expensive shadow passes disabled');
    const frame=(input={})=>{
        sim.frame(input);test.assert(!engine.xr.lastError,'XR frame succeeds',String(engine.xr.lastError));
        const head=engine.xr.player.state.head,panel=world.performancePanel.meshInfo;
        if(head.object?.id===world.performancePanel.id&&head.tracked){
            const m=head.matrix,[x,y,z]=PERFORMANCE_PANEL_OFFSET;
            const origin=engine.xr.player.state.locomotion.origin;
            const expected=[m[12]+origin[0]*x+origin[8]*z,origin[13]+y,m[14]+origin[2]*x+origin[10]*z];
            test.assert(near(panel.position,expected),'debug panel stays beside right foot through motion and teleport');
            test.assert(near(panel.scale,[1,1,1]),'spatial panel keeps authored physical size');
        }
    };
    const buttons=(a=0,b=0)=>[0,0,0,0,a,b];
    frame({head:{position:[0,1.6,0],orientation:[0,0,0,1]},controllers:{left:{position:[-.3,1.1,-.2],axes:[0,0,0,0]},right:{position:[.3,1.1,-.2],buttons:buttons(),axes:[0,0,0,0]}}});
    frame();
    assertForeground(sim.pixels('left'),'room left');assertForeground(sim.pixels('right'),'room right');
    test.assert(pixelDifference(sim.pixels('left'),sim.pixels('right'))>50,'distinct rendered stereo eyes');
    // Both eyes see one finite-depth scene panel; no screen-corner overlay.
    const panel=world.performancePanel;
    for(const enemy of world.enemies)enemy.pauseAnimation();
    frame({dt:0,head:{orientation:[-Math.sin(.4),0,0,Math.cos(.4)]}});const empty={left:sim.pixels('left'),right:sim.pixels('right')};
    engine.xr.player.bindHead(panel,{offset:PERFORMANCE_PANEL_OFFSET,anchor:'feet'});frame({dt:0});
    const panelPixels={left:sim.pixels('left'),right:sim.pixels('right')};
    const bounds=(a,b)=>{
        let minX=a.width,maxX=-1,minY=a.height,maxY=-1,count=0;
        for(let y=0;y<a.height;y++)for(let x=0;x<a.width;x++){
            const i=(y*a.width+x)*4;
            if(Math.abs(a.data[i]-b.data[i])+Math.abs(a.data[i+1]-b.data[i+1])+Math.abs(a.data[i+2]-b.data[i+2])>40){minX=Math.min(minX,x);maxX=Math.max(maxX,x);minY=Math.min(minY,y);maxY=Math.max(maxY,y);count++;}
        }
        return {cx:(minX+maxX)/2,cy:(minY+maxY)/2,count,minX,maxX,minY,maxY};
    };
    const l=bounds(empty.left,panelPixels.left),r=bounds(empty.right,panelPixels.right);
    for(const eye of [l,r])test.assert(eye.count>100&&eye.cx>160&&eye.maxX<320&&eye.minY>10&&eye.maxY<310,'debug panel is visible at the right foot when looking down',eye);
    test.assert(Math.abs(l.cx-r.cx)>.5,'panel has stereo disparity at finite distance',{left:l.cx,right:r.cx});
    test.assert(!engine.debugStats().enabled&&engine.debugStats().hudFrames===0,'screen-space HUD remains disabled');
    const anchored=panel.meshInfo.position;
    for(const yaw of [-1.1,.8]){
        frame({head:{orientation:[0,Math.sin(yaw/2),0,Math.cos(yaw/2)]}});
        test.assert(near(panel.meshInfo.position,anchored),'looking sideways leaves debug panel fixed in the player reference frame');
    }
    // A real down/right glance must bring the stationary panel into the centre of both eyes.
    const yaw=-Math.atan2(.9,1),pitch=-Math.atan2(1.25,Math.hypot(.9,1));
    const sy=Math.sin(yaw/2),cy=Math.cos(yaw/2),sp=Math.sin(pitch/2),cp=Math.cos(pitch/2);
    frame({dt:0,head:{orientation:[cy*sp,sy*cp,-sy*sp,cy*cp]}});
    const glanced={left:sim.pixels('left'),right:sim.pixels('right')};
    engine.xr.player.bindHead(null);frame({dt:0});
    for(const eye of ['left','right']){
        const seen=bounds(sim.pixels(eye),glanced[eye]);
        test.assert(seen.count>100&&Math.abs(seen.cx-160)<25&&Math.abs(seen.cy-160)<25,'turning gaze toward debug panel reveals it centrally instead of moving it away',{eye,...seen});
    }
    engine.xr.player.bindHead(panel,{offset:PERFORMANCE_PANEL_OFFSET,anchor:'feet'});frame({dt:0});
    frame({head:{position:[0,1.2,0],orientation:[-Math.sin(.55),0,0,Math.cos(.55)]}});
    test.assert(near(panel.meshInfo.position,anchored),'looking down and crouching do not make floor debug panel chase the face');
    frame({head:{position:[0,1.6,0],orientation:[-Math.sin(.4),0,0,Math.cos(.4)]}});
    const panelPreview=document.querySelector('#spatial');const stereoPanel=sim.pixels();panelPreview.width=stereoPanel.width;panelPreview.height=stereoPanel.height;panelPreview.getContext('2d').putImageData(stereoPanel,0,0);
    frame({head:{position:[.25,1.7,.3],orientation:[0,Math.sin(.4),0,Math.cos(.4)]}});
    frame({head:{tracked:false}});test.assert(panel.meshInfo.position[1]<-9000,'tracking loss hides spatial panel');
    frame({head:{tracked:true,position:[0,1.6,0],orientation:[0,0,0,1]}});
    engine.xr.player.bindHead(null);test.assert(panel.meshInfo.position[1]<-9000,'disabling panel hides scene node');
    engine.xr.player.bindHead(panel,{offset:PERFORMANCE_PANEL_OFFSET,anchor:'feet'});frame();
    for(const enemy of world.enemies)enemy.playAnimation('Idle',{loop:true});
    const start=engine.xr.player.state.head.position;
    for(let i=0;i<30;i++)frame({controllers:{left:{axes:[0,0,0,-1]}}});
    frame({controllers:{left:{axes:[0,0,0,0]}}});
    const walked=engine.xr.player.state;
    test.assert(walked.head.position[2]<start[2]-.9,'left analog actually moves head through world');
    test.assert(near(walked.rightHand.position,[.3,1.1,walked.head.position[2]-.2]),'grip and stereo rig share locomotion');
    const initial=sim.pixels('left');
    // Movement is head-relative and physical room-scale offset is retained.
    frame({head:{position:[.7,1.6,.4]}});
    const before=engine.xr.player.state.head.position;
    frame({controllers:{right:{axes:[0,0,1,0]}}});
    const snapped=engine.xr.player.state;
    test.assert(near(snapped.head.position,before),'snap turns around the actual head');
    const origin=snapped.locomotion.origin;
    frame();test.assert(near(engine.xr.player.state.locomotion.origin,origin),'held right stick does not repeat snap');
    test.assert(pixelDifference(initial,sim.pixels('left'))>100,'rig motion changes real eye pixels');
    frame({controllers:{right:{axes:[0,0,0,0]}}});
    // Restore a known start via session reentry, preserving configuration.
    await engine.xr.exit();await engine.xr.enter({resolutionScale:1});
    frame({head:{position:[.7,1.6,.4]},controllers:{right:{position:[.3,1.1,-.2],buttons:buttons(),axes:[0,0,0,0]},left:{axes:[0,0,0,0]}}});frame();
    const model=world.enemies[0];model.playAnimation('Idle',{loop:true});
    const normalStart=model.info.playback.time;
    for(let i=0;i<8;i++)frame();
    const normalDelta=model.info.playback.time-normalStart;
    frame({controllers:{right:{buttons:buttons(1)}}});
    const target=engine.xr.player.state.locomotion.target;
    test.assert(target!==null&&engine.xr.player.state.locomotion.aiming,'A hold produces valid landing');
    test.assert(Math.abs(engine.xr.player.state.locomotion.timeScale-.15)<.001,'A hold slows simulation to 15 percent');
    const slowStart=model.info.playback.time;
    for(let i=0;i<8;i++)frame();
    const slowDelta=model.info.playback.time-slowStart;
    test.assert(Math.abs(slowDelta/normalDelta-.15)<.02,'actual GLB animation advances at slowed rate',{normalDelta,slowDelta});
    // Isolate marker pixels against the same paused world, looking down at the landing.
    for(const enemy of world.enemies)enemy.pauseAnimation();
    frame({head:{orientation:[Math.sin(-.24),0,0,Math.cos(-.24)]}});
    const marked=sim.pixels('left');
    const markerPreview=document.querySelector('#teleport');markerPreview.width=marked.width;markerPreview.height=marked.height;markerPreview.getContext('2d').putImageData(marked,0,0);
    frame({controllers:{right:{buttons:buttons(1,1)}}});
    test.assert(pixelDifference(marked,sim.pixels('left'),15)>10,'landing ellipse is visible in rendered eye');
    test.assert(!engine.xr.player.state.locomotion.aiming&&engine.xr.player.state.locomotion.timeScale===1,'B cancels aim and restores time');
    frame({controllers:{right:{buttons:buttons()}}});
    frame({controllers:{right:{buttons:buttons(1)}}});
    const landing=engine.xr.player.state.locomotion.target;
    frame({controllers:{right:{buttons:buttons()}}});
    const teleported=engine.xr.player.state;
    test.assert(near([teleported.head.position[0],teleported.head.position[2]],[landing[0],landing[2]]),'release lands physical head at ellipse');
    test.assert(Math.abs(teleported.head.position[1]-1.6)<.001,'teleport preserves real height');
    test.assert(teleported.rightHand.speed<.001,'teleport does not synthesize a weapon swing');
    const saved=teleported.locomotion.origin;
    frame({controllers:{right:{buttons:buttons(1)}}});
    frame({controllers:{right:{tracked:false}}});
    frame({controllers:{right:{tracked:true,buttons:buttons()}}});
    test.assert(near(engine.xr.player.state.locomotion.origin,saved)&&engine.xr.player.state.locomotion.timeScale===1,'tracking loss cancels teleport without jumping');
    frame({controllers:{right:{buttons:buttons(1)}}});sim.resetReferenceSpace();frame({controllers:{right:{buttons:buttons()}}});
    test.assert(near(engine.xr.player.state.locomotion.origin,saved),'reference reset cancels pending teleport');
    await engine.xr.exit();await engine.xr.enter({resolutionScale:1});
    frame({head:{position:[0,1.6,0],orientation:[0,0,0,1]},controllers:{right:{buttons:buttons()},left:{axes:[0,0,0,0]}}});frame();
    // A street barricade branches the route; navigation matches the authored GLB footprint.
    for(let i=0;i<280;i++)frame({dt:50,controllers:{left:{axes:[0,0,0,-1]}}});
    const wall=engine.xr.player.state.head.position;
    const barrier=world.layout.obstacles.find(o=>o.id==='Branch barricade');
    if(!barrier)throw new Error('Missing central-barricade layout obstacle');
    const stop=barrier.center[2]+barrier.halfExtents[2]+.4;
    test.assert(wall[2]>=stop-.001&&wall[2]<=stop+.15,'analog walking stops before street barricade',wall);
    frame({controllers:{left:{axes:[0,0,0,0]},right:{buttons:buttons(1)}}});
    test.assert(engine.xr.player.state.locomotion.target===null,'teleport arc cannot cross street barricade');
    // Traverse both flanking routes and the rear junction using actual A press/release frames.
    await engine.xr.exit();await engine.xr.enter({resolutionScale:1});
    frame({head:{position:[0,1.6,0],orientation:[0,0,0,1]},controllers:{right:{position:[.3,1.1,-.2],buttons:buttons(),axes:[0,0,0,0]},left:{axes:[0,0,0,0]}}});frame();
    const route=[];let previous=[0,0,0];
    for(const padIndex of [1,3,4,6,8,7,-1,5,3,2,0]){
        // Go outside the right-hand utility cover before approaching the next pad.
        const destination=padIndex===-1?[9,0,-23]:world.layout.landingPads[padIndex].position;
        // Split long connections: physical arc reach also depends on the controller height.
        const steps=Math.ceil(Math.hypot(destination[0]-previous[0],destination[2]-previous[2])/7.5);
        for(let i=1;i<=steps;i++)route.push({padIndex,target:previous.map((v,k)=>v+(destination[k]-v)*i/steps)});
        previous=destination;
    }
    for(const {padIndex,target} of route){
        const head=engine.xr.player.state.head.position;
        const dx=target[0]-head[0]-.3,dz=target[2]-head[2]+.2,d=Math.hypot(dx,dz),v=world.locomotion.teleportSpeed,g=9.8,h=1.1;
        const discriminant=v**4-g*(g*d*d-2*h*v*v);
        test.assert(discriminant>=0,'pad is reachable by native teleport arc',padIndex);
        const pitch=Math.atan((v*v-Math.sqrt(discriminant))/(g*d)),yaw=Math.atan2(-dx,-dz);
        const sp=Math.sin(pitch/2),cp=Math.cos(pitch/2),sy=Math.sin(yaw/2),cy=Math.cos(yaw/2);
        frame({controllers:{right:{orientation:[cy*sp,sy*cp,-sy*sp,cy*cp],buttons:buttons()}}});
        frame({controllers:{right:{buttons:buttons(1)}}});
        const aim=engine.xr.player.state.locomotion.target;
        test.assert(aim!==null&&near(aim,target,.04),'route landing is valid', {padIndex,aim,target});
        frame({controllers:{right:{buttons:buttons()}}});
        const landed=engine.xr.player.state.head.position;
        test.assert(near([landed[0],landed[2]],[target[0],target[2]],.04),'teleport traverses authored street route',padIndex);
    }
    // Elevated landing uses the actual static collider world; both eyes/hands share the new height.
    await engine.xr.exit();await engine.xr.enter({resolutionScale:1});
    frame({head:{position:[0,1.6,0],orientation:[0,0,0,1]},controllers:{left:{axes:[0,0,0,0]},right:{position:[.3,1.1,-.2],orientation:[0,0,0,1],buttons:buttons()}}});frame();
    const aimAt=(target)=>{
        const head=engine.xr.player.state.head.position;
        const dx=target[0]-head[0]-.3,dz=target[2]-head[2]+.2,d=Math.hypot(dx,dz),v=world.locomotion.teleportSpeed,g=9.8;
        const dy=target[1]-(head[1]-1.6+1.1);
        const pitch=Math.atan((v*v+Math.sqrt(v**4-g*(g*d*d+2*dy*v*v)))/(g*d)),yaw=Math.atan2(-dx,-dz);
        const sp=Math.sin(pitch/2),cp=Math.cos(pitch/2),sy=Math.sin(yaw/2),cy=Math.cos(yaw/2);
        frame({controllers:{right:{orientation:[cy*sp,sy*cp,-sy*sp,cy*cp],buttons:buttons()}}});
        frame({controllers:{right:{buttons:buttons(1)}}});
        const aim=engine.xr.player.state.locomotion.target;
        test.assert(aim!==null&&near(aim,target,.05),'elevated arc selects actual upper surface',{aim,target});
        frame({controllers:{right:{buttons:buttons()}}});
        test.assert(near(engine.xr.player.state.head.position,[target[0],target[1]+1.6,target[2]],.05),'head retains physical height above roof or bridge');
    };
    aimAt([0,9.45,-20]);
    for(let i=0;i<90;i++)frame();
    test.assert(Math.abs(engine.xr.player.state.head.position[1]-11.05)<.005,'bridge supports stationary player');
    for(let i=0;i<80;i++)frame({controllers:{left:{axes:[0,0,0,-1]}}});
    frame({controllers:{left:{axes:[0,0,0,0]}}});
    for(let i=0;i<120;i++)frame();
    test.assert(Math.abs(engine.xr.player.state.head.position[1]-1.6)<.005,'walking beyond deck edge falls and lands on street');
    // The right-side building cap is 17.7m high; land away from its penthouse.
    await engine.xr.exit();await engine.xr.enter({resolutionScale:1});
    frame({head:{position:[0,1.6,0],orientation:[0,0,0,1]},controllers:{left:{axes:[0,0,0,0]},right:{position:[.3,1.1,-.2],orientation:[0,0,0,1],buttons:buttons()}}});frame();
    aimAt([12.6,17.7,-7.5]);
    for(let i=0;i<90;i++)frame();
    test.assert(Math.abs(engine.xr.player.state.head.position[1]-19.3)<.005,'roof supports player instead of dropping to y zero');
    const effects=engine.effects_stats();
    test.assert(effects.shadowLightCount===0&&effects.dynamicUpdates===0&&effects.staticUpdates===0,'full animated route submits no shadow work',effects);
    const stereo=sim.pixels();const preview=document.querySelector('#stereo');preview.width=stereo.width;preview.height=stereo.height;preview.getContext('2d').putImageData(stereo,0,0);
    await engine.xr.exit();test.assert(engine.xr.player.state.locomotion.timeScale===1,'exit clears slowdown');
});
engine?.dispose();sim.dispose();
