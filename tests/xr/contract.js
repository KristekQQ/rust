import {initEngine} from '../../js/engine.js';
import {installXRSimulator} from './simulator.js';
import {matrixFromPose} from './poses.js';
import {XRTestRun,publishXRReport,enableReportDownload,pixelDifference} from './testkit.js';
import {assertForeground} from '../pixels.js';

const profiles=[
    {id:'standing-72',hz:72,ipd:0.064,eyeWidth:192,eyeHeight:192,height:1.6},
    {id:'seated-90',hz:90,ipd:0.058,eyeWidth:192,eyeHeight:192,height:1.1},
    {id:'room-120',hz:120,ipd:0.072,eyeWidth:192,eyeHeight:192,height:1.8}
];
const query=new URLSearchParams(location.search), selected=query.get('profile')||'all';
const picker=document.querySelector('#profile');picker.value=selected;
document.querySelector('#run').onclick=()=>{const url=new URL(location.href);url.searchParams.set('profile',picker.value);location.href=url;};
enableReportDownload(document.querySelector('#export'));
const aggregate={version:1,suite:'webxr-contract',environment:'simulated-webxr-real-rust-renderer',status:'RUNNING',checks:[],profiles:[],trace:null};
const update=()=>publishXRReport(aggregate);
const geometry={positions:[-1,-1,-1,1,-1,-1,1,1,-1,-1,1,-1,-1,-1,1,1,-1,1,1,1,1,-1,1,1],indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5]};
const quaternion=(axis,angle)=>axis.map(value=>value*Math.sin(angle/2)).concat(Math.cos(angle/2));
const near=(actual,expected,tolerance=1e-4)=>actual.length===expected.length&&actual.every((value,i)=>Math.abs(value-expected[i])<tolerance);

for(const profile of profiles.filter(profile=>selected==='all'||profile.id===selected)) {
    // Each profile starts from restored browser globals and a fresh engine lifetime.
    const sim=installXRSimulator({profile,floorAvailable:profile.id!=='seated-90'});
    const run=new XRTestRun(profile.id,sim,report=>{
        aggregate.checks=aggregate.profiles.flatMap(profile=>profile.checks).concat(report.checks.map(check=>({...check,name:`${profile.id}/${check.name}`})));
        update();
    });
    let engine;
    const report=await run.execute(async test=>{
        const canvas=document.querySelector('#gpu-canvas');
        engine=await initEngine(canvas,{backend:'webgl'});
        document.documentElement.dataset.rendererBackend=engine.renderer_backend();
        engine.resize(420,280);engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
        engine.setCameraView([0,profile.height,0],[0,profile.height,-2]);
        engine.scene.createMesh({...geometry,color:[0.8,0.08,0.04]}).setPosition([-0.35,profile.height,-1.8]).setScale([0.2,0.3,0.2]);
        engine.scene.createSphere([0.4,profile.height-0.15,-2.3]).setScale([0.25,0.25,0.25]);
        engine.scene.createLight({position:[-2,4,0],color:[1,1,1]});
        const left=engine.scene.createMesh({...geometry,color:[0.1,0.9,0.2]}).setScale([0.06,0.08,0.12]);
        const right=engine.scene.createMesh({...geometry,color:[0.1,0.3,0.9]}).setScale([0.06,0.08,0.12]);
        engine.xr.bindController('left',left);engine.xr.bindController('right',right);
        test.assert(await engine.xr.isSupported(),'support probe');
        sim.denyNextRequest();let denied=false;
        try{await engine.xr.enter();}catch{denied=true;}
        test.assert(denied&&!engine.xr.isPresenting,'permission denial cleans pending session');
        await engine.xr.enter();
        test.assert(engine.xr.isPresenting,'immersive session starts');
        // A local fallback has an emulated 1.6m standing offset; raw native Y is adjusted here.
        const rawHeight=profile.id==='seated-90'?profile.height-1.6:profile.height;
        const head={position:[0,rawHeight,0],orientation:[0,0,0,1],tracked:true};
        const frame=async input=>{
            sim.frame(input);
            test.assert(!engine.xr.lastError,'frame has no production XR error',String(engine.xr.lastError));
            if(query.has('slow'))await sim.wait(30);
        };
        await frame({head,controllers:{left:null,right:null}});
        const base=sim.pixels('left'), other=sim.pixels('right');
        assertForeground(base,'left eye');assertForeground(other,'right eye');
        test.assert(pixelDifference(base,other)>20,'distinct stereo eyes / IPD');
        const stereo=sim.pixels();const preview=document.querySelector('#stereo');preview.width=stereo.width;preview.height=stereo.height;preview.getContext('2d').putImageData(stereo,0,0);
        const timer=engine.scene.createCube([100,100,100]).removeAfter(2.5/profile.hz);
        await frame({});await frame({});test.assert(timer.exists,'simulation advances once per frame, not per eye');
        await frame({});test.assert(!timer.exists,'simulation uses profile frame interval');
        const replayStart=sim.recording().frames.length, golden=[];
        for(const [name,change] of [
            ['X translation',{position:[0.2,rawHeight,0]}],['Y translation',{position:[0,rawHeight+0.15,0]}],['Z translation',{position:[0,rawHeight,0.2]}],
            ['yaw',{orientation:quaternion([0,1,0],0.2)}],['pitch',{orientation:quaternion([1,0,0],0.15)}],['roll',{orientation:quaternion([0,0,1],0.2)}]
        ]) {
            await frame({head:{...head,...change}});
            const image=sim.pixels('left');
            test.assert(pixelDifference(base,image)>20,`head 6DoF: ${name}`);
            golden.push(image);
        }
        const movement=sim.recording();movement.frames=movement.frames.slice(replayStart);
        await sim.replay(movement,{afterFrame:index=>test.assert(pixelDifference(golden[index],sim.pixels('left'),0)===0,'replayed head trace reproduces exact rendered pixels')});
        const pose=(x,y,z,orientation=[0,0,0,1])=>({position:[x,y-(profile.id==='seated-90'?1.6:0),z],orientation,pressed:false,tracked:true});
        await frame({head,controllers:{left:pose(-0.3,profile.height-0.3,-0.7,quaternion([0,1,0],0.3)),right:pose(0.3,profile.height-0.3,-0.7,quaternion([1,0,0],0.2))}});
        test.assert(near(left.meshInfo.position,[-0.3,profile.height-0.3,-0.7]),'left grip world position and floor');
        test.assert(near(right.meshInfo.position,[0.3,profile.height-0.3,-0.7]),'right grip world position and floor');
        test.assert(Math.abs(left.meshInfo.rotation[1]-0.3)<1e-4&&Math.abs(right.meshInfo.rotation[0]-0.2)<1e-4,'controller orientation in Rust');
        await frame({controllers:{right:{targetRayMatrix:matrixFromPose({position:[-8,rawHeight,8]})}}});
        test.assert(near(right.meshInfo.position,[0.3,profile.height-0.3,-0.7]),'grip is independent of aiming ray');
        await frame({controllers:{right:{grip:false}}});
        test.assert(right.meshInfo.position[1]<-100,'source without grip is not treated as a held controller');
        await frame({controllers:{right:{grip:true}}});
        let leftSelect=0,rightSelect=0;
        const select=event=>event.detail.handedness==='left'?leftSelect++:rightSelect++;
        engine.xr.addEventListener('select',select);
        await frame({controllers:{left:{pressed:true}}});await frame({controllers:{left:{pressed:true}}});
        test.assert(leftSelect===1&&rightSelect===0,'trigger edge once per hand, held input does not repeat');
        await frame({controllers:{left:{tracked:false}}});
        test.assert(left.meshInfo.position[1]<-100&&right.meshInfo.position[1]>0,'one lost grip hides only its own object');
        await frame({controllers:{left:{tracked:true,pressed:true}}});
        test.assert(leftSelect===1,'held trigger does not synthesize press after tracking recovery');
        await frame({controllers:{left:{pressed:false}}});await frame({controllers:{left:{pressed:true}}});
        test.assert(leftSelect===2,'release rearms trigger after recovery');
        await frame({controllers:{left:null}});await frame({controllers:{left:{...pose(-0.3,profile.height-0.3,-0.7),pressed:true}}});
        test.assert(leftSelect===2,'controller reconnect does not synthesize press');
        sim.setVisibility('visible-blurred');
        await frame({controllers:{right:{pressed:true}}});
        test.assert(rightSelect===0,'system overlay suppresses controller input');
        const pausedTimer=engine.scene.createCube([100,100,100]).removeAfter(0.08);
        sim.setVisibility('hidden');const pending=sim.session.pendingFrameCount;
        sim.frame({dt:500,controllers:{right:{pressed:true}}});
        test.assert(sim.session.pendingFrameCount===pending,'hidden session suppresses RAF delivery');
        sim.setVisibility('visible');await frame({controllers:{right:{pressed:true}}});
        test.assert(rightSelect===0,'held trigger does not fire on visibility resume');
        test.assert(pausedTimer.exists,'resume does not replay hidden elapsed time into Rust simulation');
        await frame({controllers:{right:{pressed:false}}});await frame({controllers:{right:{pressed:true}}});
        test.assert(rightSelect===1,'new trigger edge after visibility resume');
        const tool=engine.scene.createMesh(geometry).setScale([1,1,1]).setCutter({base:[0,0,-0.1],tip:[0,0,-0.95]});
        const target=engine.scene.createMesh(geometry).setPosition([0.25,profile.height-0.3,-0.75]).setScale([0.12,0.12,0.12]).setCuttable();
        engine.xr.bindController('right',tool,{cutting:true});
        await frame({controllers:{right:{...pose(0.25,profile.height+0.1,-0.1),pressed:true}}});
        await frame({controllers:{right:{...pose(0.25,profile.height-0.05,-0.1),pressed:true}}});
        sim.resetReferenceSpace();
        await frame({controllers:{right:{...pose(0.25,profile.height-0.6,-0.1),pressed:true}}});
        test.assert(target.exists,'reference-space reset never connects stale blade samples');
        const ending=sim.session;
        test.assert(ending.pendingFrameCount===1,'exactly one XR frame is scheduled');
        await engine.xr.exit();
        test.assert(!engine.xr.isPresenting&&ending.pendingFrameCount===0,'end cancels pending XR frame');
        test.assert(canvas.width===420&&canvas.height===280,'end restores canvas dimensions');
        await engine.xr.enter();await frame({head,controllers:{left:null,right:null}});
        test.assert(sim.session!==ending&&sim.session.pendingFrameCount===1,'fresh session can reenter without duplicate RAF');
        engine.dispose();
        test.assert(!engine.xr.isPresenting&&sim.session.pendingFrameCount===0,'dispose stops active XR');
        let stale=false;try{void left.exists;}catch{stale=true;}
        test.assert(stale,'disposed scene handles cannot mutate next game');
        // Replay the same platform inputs against a new renderer lifetime.
        const trace=sim.recording();
        test.assert(trace.version===1&&trace.frames.length>10,'versioned input trace is available');
    });
    try{engine?.dispose();sim.dispose();}
    catch(error){report.status='FAIL';report.error=`Cleanup: ${error.stack||error}`;}
    aggregate.profiles.push(report);
    aggregate.checks=aggregate.profiles.flatMap(profile=>profile.checks.map(check=>({...check,name:`${profile.suite}/${check.name}`})));
    aggregate.trace=aggregate.profiles.map(profile=>({profile:profile.profile,trace:profile.trace}));
    if(report.status==='FAIL'){aggregate.error=report.error;break;}
}
aggregate.status=aggregate.profiles.length&&aggregate.profiles.every(report=>report.status==='PASS')?'PASS':'FAIL';
if(!aggregate.profiles.length)aggregate.error='Unknown test profile';
update();
