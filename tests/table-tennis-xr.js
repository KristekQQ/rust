import {matrixFromPose} from './xr/poses.js';
import {installXRSimulator} from './xr/simulator.js';
import {XRTestRun,publishXRReport,pixelDifference} from './xr/testkit.js';
import {PADDLE_FACE_OFFSET} from '../examples/table-tennis/scene.js';
import {VR_DEBUG_PANEL_OFFSET} from '../examples/shared/vr-debug-panel.js';
import {assertForeground} from './pixels.js';
const sim=installXRSimulator({profile:{id:'table-tennis-72',hz:72,eyeWidth:320,eyeHeight:320},clockTimers:false});
const run=new XRTestRun('table-tennis-xr',sim,publishXRReport);
let engine;
await run.execute(async test=>{
    const game=await import('../examples/table-tennis/main.js');engine=game.engine;
    const {world}=game;
    const showStereo=(title,eyes)=>{
        const label=document.createElement('h3');label.textContent=title;
        const canvas=document.createElement('canvas');canvas.width=eyes.left.width*2;canvas.height=eyes.left.height;
        const context=canvas.getContext('2d');context.putImageData(eyes.left,0,0);context.putImageData(eyes.right,eyes.left.width,0);
        document.body.append(label,canvas);
    };
    await engine.xr.enter({resolutionScale:1});
    const frame=input=>{sim.frame(input);test.assert(!engine.xr.lastError,'XR frame succeeds');};
    frame({head:{position:[0,1.6,0]},controllers:{right:{position:[.3,1,-.5],orientation:[0,0,0,1]}}});
    frame({controllers:{right:{position:[.25,1.05,-.65]}}});
    const paddle=world.player.meshInfo;
    test.assert(Math.abs(paddle.position[0]-.25)<.005&&Math.abs(paddle.position[2]+.65)<.005,'right grip moves paddle in world metres');
    assertForeground(sim.pixels('left'),'table left eye',1000);assertForeground(sim.pixels('right'),'table right eye',1000);
    test.assert(pixelDifference(sim.pixels('left'),sim.pixels('right'))>50,'real stereo scene');
    const near=(a,b)=>a.every((v,i)=>Math.abs(v-b[i])<.003);
    const rotate=(v,r)=>{
        const [x,y,z]=r;
        const a=[Math.cos(z)*v[0]-Math.sin(z)*v[1],Math.sin(z)*v[0]+Math.cos(z)*v[1],v[2]];
        const b=[a[0],Math.cos(x)*a[1]-Math.sin(x)*a[2],Math.sin(x)*a[1]+Math.cos(x)*a[2]];
        return [Math.cos(y)*b[0]+Math.sin(y)*b[2],b[1],-Math.sin(y)*b[0]+Math.cos(y)*b[2]];
    };
    for(const raw of [[.2,.3,.1,.92],[.7,-.2,.4,.3],[-.4,.6,.3,.6]]){
        const n=Math.hypot(...raw),orientation=raw.map(v=>v/n),position=[.25,1.05,-.85];
        frame({dt:0,controllers:{right:{position,orientation}}});
        const m=matrixFromPose({position,orientation}),info=world.player.meshInfo;
        const offset=rotate([0,0,-PADDLE_FACE_OFFSET],info.rotation);
        test.assert(near(info.position.map((v,i)=>v+offset[i]),[m[12]-m[8]*PADDLE_FACE_OFFSET,m[13]-m[9]*PADDLE_FACE_OFFSET,m[14]-m[10]*PADDLE_FACE_OFFSET]),'combined wrist orientation keeps shaft on grip -Z');
        test.assert(near(rotate([1,0,0],info.rotation),[m[0],m[1],m[2]]),'combined wrist orientation keeps face normal on grip +X');
    }
    const mesh=world.player.meshSnapshot;
    test.assert(near(mesh.positions[0],[-.006,0,-PADDLE_FACE_OFFSET]),'paddle shaft extends along WebXR grip -Z');
    const handle=mesh.positions.slice(-8);
    const centre=handle.reduce((a,p)=>a.map((v,i)=>v+p[i]/8),[0,0,0]);
    test.assert(near(centre,[0,0,0]),'actual handle geometry is centred on tracked grip');
    const xs=mesh.positions.slice(0,82).map(p=>p[1]);
    test.assert(Math.abs(Math.max(...xs)-Math.min(...xs)-.17)<.001,'paddle face is 17cm wide');
    frame({dt:0,controllers:{right:{position:[.25,1.05,-.65],orientation:[0,0,Math.SQRT1_2,Math.SQRT1_2]}}});
    test.assert(Math.abs(world.player.meshInfo.rotation[2]-Math.PI/2)<.003,'grip roll rotates real paddle including offset face');
    const rolled={left:sim.pixels('left'),right:sim.pixels('right')};
    showStereo('Pálka při natočení úchopu · levé a pravé oko',rolled);
    frame({dt:0,controllers:{right:{tracked:false}}});
    for(const eye of ['left','right'])test.assert(pixelDifference(rolled[eye],sim.pixels(eye))>40,'rolled paddle has visible geometry in '+eye);
    frame({dt:0,controllers:{right:{tracked:true,orientation:[.5,-.5,.5,.5]}}});
    test.assert(near(world.infoPanel.meshInfo.position,VR_DEBUG_PANEL_OFFSET),'native FPS panel is at shared right-foot anchor');
    test.assert(world.infoPanel.meshSnapshot.triangles.length>20,'native scene panel contains rendered text');
    const anchored=world.infoPanel.meshInfo.position;
    frame({dt:0,head:{orientation:[0,Math.sin(.4),0,Math.cos(.4)]}});
    test.assert(near(world.infoPanel.meshInfo.position,anchored),'looking sideways does not move FPS panel');
    const yaw=-Math.atan2(.9,1),pitch=-Math.atan2(1.25,Math.hypot(.9,1));
    const sy=Math.sin(yaw/2),cy=Math.cos(yaw/2),sp=Math.sin(pitch/2),cp=Math.cos(pitch/2);
    frame({dt:0,head:{orientation:[cy*sp,sy*cp,-sy*sp,cy*cp]}});
    const panelPixels={left:sim.pixels('left'),right:sim.pixels('right')};
    showStereo('Pohled dolů na FPS a informace o scéně · obě oči',panelPixels);
    engine.xr.player.bindHead(null);frame({dt:0});
    for(const eye of ['left','right'])test.assert(pixelDifference(panelPixels[eye],sim.pixels(eye))>80,'FPS and scene text actually visible when glancing down in '+eye);
    engine.xr.player.bindHead(world.infoPanel,{offset:VR_DEBUG_PANEL_OFFSET,anchor:'feet'});frame({dt:0});
    const toggle=document.querySelector('#fps-toggle');
    toggle.checked=false;toggle.dispatchEvent(new Event('change'));frame({dt:0});
    test.assert(world.infoPanel.meshInfo.position[1]<-9000,'FPS switch actually hides spatial panel');
    toggle.checked=true;toggle.dispatchEvent(new Event('change'));frame({dt:0});
    test.assert(near(world.infoPanel.meshInfo.position,anchored),'FPS switch restores shared floor anchor');
    test.assert(!engine.debugStats().enabled,'screen-space corner HUD stays disabled');
    frame({dt:0,head:{tracked:false}});test.assert(world.infoPanel.meshInfo.position[1]<-9000,'head tracking loss hides FPS panel');
    frame({dt:0,head:{tracked:true,position:[0,1.6,0],orientation:[0,0,0,1]}});
    engine.scene.tableTennisCommand('practice');
    const horizontal=[0,0,Math.SQRT1_2,Math.SQRT1_2];
    frame({controllers:{right:{position:[.38,1,-.65],orientation:horizontal,buttons:[1]}}});
    const drop=engine.scene.tableTennisState;
    test.assert(drop.practice&&Math.abs(drop.ballPosition[0]-.38)<.001,'trigger drops ball above THIS frame tracked paddle');
    test.assert(near(drop.paddleNormal,[0,1,0]),'horizontal physical paddle normal points up');
    frame({controllers:{right:{buttons:[0]}}});
    let stationary=0;
    for(let i=0;i<70;i++){
        frame();const state=engine.scene.tableTennisState;
        if(state.rallyHits>=1&&state.ballVelocity[1]>0){stationary=state.ballVelocity[1];break;}
        if(i%16===15)await new Promise(resolve=>setTimeout(resolve,0));
    }
    test.assert(stationary>1&&stationary<3.5,'falling ball physically rebounds from still paddle',engine.scene.tableTennisState);
    const bounce=engine.scene.tableTennisState;
    test.assert(Math.abs(bounce.ballVelocity[0])<.02&&Math.abs(bounce.ballVelocity[2])<.02,'no automatic shot toward opponent');
    showStereo('Fyzický dopad a odraz od stojící pálky', {left:sim.pixels('left'),right:sim.pixels('right')});
    for(let i=0;i<90&&engine.scene.tableTennisState.rallyHits<2;i++){
        frame();if(i%16===15)await new Promise(resolve=>setTimeout(resolve,0));
    }
    test.assert(engine.scene.tableTennisState.rallyHits>=2&&engine.scene.tableTennisState.score.every(v=>v===0),'repeated same-paddle bounces remain legal in practice');
    frame({controllers:{right:{orientation:[0,0,Math.sin(Math.PI/6),Math.cos(Math.PI/6)]}}});
    engine.scene.tableTennisCommand('serve');let tilted=false;
    for(let i=0;i<50;i++){
        frame();const state=engine.scene.tableTennisState;
        if(state.rallyHits>0){tilted=Math.abs(state.ballVelocity[0])>.3;break;}
        if(i%16===15)await new Promise(resolve=>setTimeout(resolve,0));
    }
    test.assert(tilted,'tilted paddle changes outgoing direction physically',engine.scene.tableTennisState);
    test.assert(Math.hypot(...engine.scene.tableTennisState.ballSpin)>1,'tangential contact creates actual ball spin');
    frame();test.assert(world.ball.meshInfo.rotation.some(v=>Math.abs(v)>.001),'physical spin rotates the marked ball geometry');
    frame({controllers:{right:{position:[.38,1,-.65],orientation:horizontal}}});
    engine.scene.tableTennisCommand('serve');let moving=0;
    for(let i=0;i<50;i++){
        frame({controllers:{right:{position:[.38,1+Math.max(0,i-8)/72*.8,-.65]}}});
        const state=engine.scene.tableTennisState;
        if(state.rallyHits>0){moving=state.ballVelocity[1];break;}
        if(i%16===15)await new Promise(resolve=>setTimeout(resolve,0));
    }
    test.assert(moving>stationary+.25,'upward wrist movement produces stronger rebound',{stationary,moving});
    engine.set_simulation_paused(true);const frozen=engine.scene.tableTennisState.ballPosition;
    frame({controllers:{right:{position:[-.4,1,-.65]}}});
    test.assert(near(engine.scene.tableTennisState.ballPosition,frozen),'pause freezes physics while tracked hands continue');
    engine.set_simulation_paused(false);frame();
    test.assert(engine.scene.tableTennisState.paddleVelocity.every(v=>Math.abs(v)<.001),'resume does not convert paused hand motion into a strike');
    frame({controllers:{right:{tracked:false}}});
    test.assert(world.player.meshInfo.position[1]<-9000,'tracking loss hides paddle');
    frame({controllers:{right:{tracked:true,position:[.3,1,-.65]}}});
    const count=engine.scene.objectCount;
    engine.scene.tableTennisCommand('reset');frame();
    test.assert(engine.scene.tableTennisState.score.every(v=>v===0)&&engine.scene.objectCount===count,'reset reuses pooled scene');
    await engine.xr.exit();test.assert(!engine.xr.isPresenting&&world.infoPanel.meshInfo.position[1]<-9000,'XR exit hides panel');
    await engine.xr.enter({resolutionScale:1});frame();
    test.assert(world.infoPanel.meshInfo.position[1]>0,'XR reentry restores FPS panel');
    await engine.xr.exit();
});
sim.dispose();
