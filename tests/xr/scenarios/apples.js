import {assertForeground} from '../../pixels.js';
import {checkCutProgress} from './cut-progress.js';
import {checkSliceMotion} from './slice-motion.js';
// Game adapter: only apple-specific setup, motion and success criteria live here.
export async function runAppleScenario({simulator,test,slow=false,onSnapshot=()=>{}}) {
    const assert=(condition,message)=>test.assert(condition,message);
    let engine;
    try {
    const app=await import('../../../examples/webxr/main.js');engine=app.engine;
    document.documentElement.dataset.rendererBackend=engine.renderer_backend();
    assert(engine.renderer_backend()==='Gl','VR simulation requires actual WebGL');
    const reports=[];
    engine.xr.addEventListener('cut',event=>{
        const report=event.detail;
        const original=report.removed.reduce((sum,id)=>sum+(records.get(id)?.volume??0),0);
        const info=report.pieces.map(piece=>piece.meshInfo);
        assert(info.every(piece=>piece.closed),'flying apple cut fragments are closed');
        assert(Math.abs(info.reduce((sum,piece)=>sum+piece.volume,0)-original)<original*0.0001,'flying apple cut conserves volume');
        report.pieces.forEach((piece,i)=>records.set(piece.id,{volume:info[i].volume,shot:'fragment'}));
        report.simulationTime=simulator.time;
        reports.push(report);
    });
    const records=new Map();
    const rememberFlying=()=>{
        for(const apple of app.gameState().liveApples) if(!records.has(apple.id))
            records.set(apple.id,{volume:apple.meshInfo.volume,shot:'extra'});
    };
    await engine.xr.enter();
    assert(app.gameState().shots===1 && simulator.timerCountForDelay(2000)===1,'XR start automatically fires and owns one timer');
    const first=app.gameState().liveApples[0];
    const startY=first.meshInfo.position[1];
    records.set(first.id,{volume:first.meshInfo.volume,shot:0});
    let seenShots=1,savedBefore=false,savedAfter=false,missVerified=false,lossVerified=false;
    for(let frame=0;frame<560;frame++) {
        const time=frame/72,period=Math.floor(time/2),phase=time-period*2;
        const side=period%2===0?-1:1;
        // Prepare above the projectile, then swing edge-first. Leaving a flat
        // face in the flight path before the swing legitimately deflects it.
        const swing=Math.max(0,Math.min(1,(phase-0.4)/0.34));
        const angle=(35-70*swing)*Math.PI/180;
        const miss=period===0;
        const lower=Math.max(0,Math.min(1,(phase-.38)/.14));
        const wrist=[miss?0.6:side*0.42,1.9-.55*lower,-0.45];
        const tracking=!(period===2 && phase>0.42 && phase<0.8);
        simulator.frame({dt:1000/72,head:{position:[0,1.55,0],tracked:tracking},controllers:{
            right:{matrix:simulator.gripX(angle,wrist),tracked:tracking,pressed:false},
            left:{position:[-1.2,1.1,-0.2],tracked:tracking,pressed:false}
        }});
        assert(!engine.xr.lastError,`XR render/controller error: ${engine.xr.lastError}`);
        if(app.gameState().shots>seenShots) {
            seenShots=app.gameState().shots;
            for(const apple of app.gameState().liveApples) if(!records.has(apple.id)) records.set(apple.id,{volume:apple.meshInfo.volume,shot:seenShots-1});
        }
        if(frame===25) assert(first.meshInfo.position[1]>startY+0.6,'auto-launched apple is genuinely airborne');
        if(period===0 && phase>0.9 && !missVerified) {
            assert(reports.length===0 && first.exists,'nearby wrist swing must miss');missVerified=true;
        }
        if(period===1 && phase>=0.4 && !savedBefore) {
            const image=simulator.pixels();assertForeground(image,'airborne apple and katana before hit');
            onSnapshot('before',image);savedBefore=true;
        }
        if(reports.length && !savedAfter) {
            // Wait a few simulation frames for the actual fragments to separate and upload.
            const latest=reports[reports.length-1];
            if(!latest.capturedAt)latest.capturedAt=time;
            if(time-latest.capturedAt>0.07) {
                const image=simulator.pixels();assertForeground(image,'real cut fragments in XR framebuffer');
                onSnapshot('after',image);savedAfter=true;
            }
        }
        if(period===1 && phase>1.1) assert(reports.some(report=>report.removed.some(id=>records.get(id)?.shot===1)),'rotational katana swing must cut the second automatically fired moving apple');
        // Reacquisition must not connect across the missing stroke. Later
        // valid samples may legitimately cut falling fruit against a held blade.
        if(period===2 && phase>0.8 && !lossVerified) {
            const gapHits=reports.filter(report=>report.removed.some(id=>records.get(id)?.shot===2));
            assert(gapHits.length===0,`tracking gap must not fabricate a connecting slash: ${JSON.stringify(gapHits.map(r=>({hand:r.handedness,time:r.simulationTime})))}`);lossVerified=true;
        }
        if(slow)await simulator.wait(1000/72);
    }
    assert(seenShots>=4 && simulator.timerCountForDelay(2000)===1,'automatic firing continues without controller buttons');
    assert(reports.some(report=>report.removed.some(id=>records.get(id)?.shot===3)),'subsequent rotational swing also cuts a newly launched apple');
    assert(savedBefore && savedAfter && missVerified && lossVerified,'all visual simulation milestones');
    // Reports hold real production SDK handles. Their geometry was checked at the hit, before expiry.
    assert(reports.every(report=>report.pieces.length===report.removed.length*2),'each split source creates two real pieces');
    // Fruit motion must contribute to the sweep even with a perfectly still hand.
    const crossing=engine.scene.createMesh(app.world.geometry).setPosition([0,.75,-1.1])
        .setScale([.13,.13,.13]).setCuttable(true,{capColor:[1,.83,.46]})
        .projectile([0,1.5,0],{gravity:0,bounce:0});
    records.set(crossing.id,{volume:crossing.meshInfo.volume,shot:'stationary'});
    for(let i=0;i<65;i++) {
        rememberFlying();
        simulator.frame({dt:1000/72,head:{position:[0,1.55,0]},controllers:{
            right:{position:[0,1.35,-.45],orientation:[0,0,0,1],tracked:true,pressed:false}
        }});
        if(slow)await simulator.wait(1000/72);
    }
    const crossingHit=()=>reports.some(report=>report.removed.includes(crossing.id));
    assert(crossingHit() || crossing.meshInfo.jammedBy===app.world.sword.id,
        `fruit crossing a stationary sharp edge cuts or wedges at half its section: ${crossing.exists?JSON.stringify(crossing.meshInfo):'removed'}`);
    if(!crossingHit()) {
        const jamStart=crossing.meshInfo.position;
        for(let i=0;i<90;i++) {
            rememberFlying();
            const jitter=(i%2?1:-1)*.0005,angle=Math.sin(i*.35)*.008;
            simulator.frame({dt:1000/90,controllers:{right:{position:[i/89*.03+jitter,1.35+jitter,-.45+jitter],
                orientation:[Math.sin(angle/2),0,0,Math.cos(angle/2)],tracked:true,pressed:false}}});
            assert(crossing.exists && crossing.meshInfo.jammedBy===app.world.sword.id,
                `real apple stays wedged through tracking jitter and wrist motion at frame ${i}: ${crossing.exists?JSON.stringify(crossing.meshInfo):'unexpected cut'}`);
            if(slow)await simulator.wait(1000/90);
        }
        assert(crossing.meshInfo.position[0]>jamStart[0]+.02,'wedged production apple visibly follows tangential hand movement');
        assert(!crossingHit(),'reversible tracking jitter never ratchets a wedged apple into a false full cut');
        onSnapshot('jam',simulator.pixels());
        for(let i=1;i<=12;i++) {
            rememberFlying();
            simulator.frame({dt:1000/72,controllers:{right:{position:[.03,1.35-i*.015,-.45],orientation:[0,0,0,1],tracked:true,pressed:false}}});
        }
    }
    assert(crossingHit(),'continuing the sharp stroke finishes fruit wedged on a stationary katana');
    checkSliceMotion({engine,app,simulator,test,records,reports,rememberFlying,onSnapshot});
    const yawTarget=engine.scene.createMesh(app.world.geometry).setPosition([0,1.35,-1.1])
        .setScale([.13,.13,.13]).setCuttable(true,{capColor:[1,.83,.46]});
    records.set(yawTarget.id,{volume:yawTarget.meshInfo.volume,shot:'yaw'});
    for(let i=0;i<=60;i++) {
        rememberFlying();
        const yaw=(i/60)*2*Math.PI/180;
        simulator.frame({dt:1000/72,controllers:{right:{position:[0,1.8-i/60*.9,-.45],
            orientation:[0,Math.sin(yaw/2),0,Math.cos(yaw/2)],tracked:true,pressed:false}}});
        if(slow)await simulator.wait(1000/72);
    }
    assert(reports.some(report=>report.removed.includes(yawTarget.id)),'small wrist yaw must not discard a legitimate vertical slash');
    const pair=[-.45,.45].map(x=>engine.scene.createMesh(app.world.geometry)
        .setPosition([x,1.35,-1.1]).setScale([.13,.13,.13]).setCuttable(true,{capColor:[1,.83,.46]}));
    pair.forEach(apple=>records.set(apple.id,{volume:apple.meshInfo.volume,shot:'two-hands'}));
    for(let i=0;i<=60;i++) {
        rememberFlying();
        simulator.frame({dt:1000/72,controllers:{
            left:{position:[-.45,1.8-i/60*.9,-.45],orientation:[0,0,0,1],tracked:true,pressed:false},
            right:{position:[.45,1.8-i/60*.9,-.45],orientation:[0,0,0,1],tracked:true,pressed:false}
        }});
        if(slow)await simulator.wait(1000/72);
    }
    for(const [index,hand] of ['left','right'].entries())
        assert(reports.some(report=>report.handedness===hand && report.removed.includes(pair[index].id)),`${hand} katana cuts real fruit without holding trigger`);
    const still={dt:0,head:{position:[0,1.55,0]},controllers:{
        right:{position:[1.2,1.1,-.2],orientation:[0,0,0,1],pressed:false},
        left:{position:[-1.2,1.1,-.2],orientation:[0,0,0,1],pressed:false}
    }};
    simulator.frame(still);
    const panels=[app.world.infoPanel,app.world.scorePanel];
    const origins=panels.map(panel=>panel.meshInfo.position);
    assert(!engine.debugStats().enabled && panels.every(panel=>panel.meshInfo.triangles>20),
        'info and score are real scene meshes with screen overlay disabled');
    const showPanels=()=>panels.forEach((panel,i)=>panel.setPosition(origins[i]));
    const hidePanels=()=>panels.forEach(panel=>panel.setPosition([100,100,100]));
    const images=()=>['left','right'].map(eye=>simulator.pixels(eye));
    const mask=(on,off)=>{
        let count=0,xsum=0;
        for(let y=0;y<on.height;y++)for(let x=0;x<on.width;x++) {
            const p=(y*on.width+x)*4;
            if(Math.abs(on.data[p]-off.data[p])+Math.abs(on.data[p+1]-off.data[p+1])+Math.abs(on.data[p+2]-off.data[p+2])>60){count++;xsum+=x;}
        }
        return {count,x:count?xsum/count:0};
    };
    const on=images();hidePanels();simulator.frame(still);const off=images();
    const base=on.map((image,i)=>mask(image,off[i]));
    assert(base.every(m=>m.count>100),'both eyes render world-space information text');
    assert(base[0].x-base[1].x>1,'text panels have actual binocular depth disparity');
    showPanels();
    const moved={...still,head:{position:[.25,1.55,0]}};
    simulator.frame(moved);const movedOn=images();hidePanels();simulator.frame(moved);const movedOff=images();
    const shifted=movedOn.map((image,i)=>mask(image,movedOff[i]));
    assert(shifted.every((m,i)=>base[i].x-m.x>8),'head translation moves scene text across the view instead of following the face');
    showPanels();simulator.frame(still);
    assert(panels.every((panel,i)=>panel.meshInfo.position.every((v,j)=>Math.abs(v-origins[i][j])<1e-5)),
        'panel world positions remain fixed during head motion');
    // A foreground solid must cover the text as it covers any scene object.
    const blocker=engine.scene.createCube([0,1.9,-1.4]).setScale([2.4,1.1,.05])
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    simulator.frame(still);const covered=images();hidePanels();simulator.frame(still);const coveredOff=images();
    assert(covered.every((image,i)=>mask(image,coveredOff[i]).count<20),'foreground geometry occludes scene text with real depth testing');
    blocker.remove();showPanels();simulator.frame(still);
    // Sparse high-speed tracking samples, including a missed display frame.
    // Use the production mesh/physics; a distant arc must still miss.
    let fastHits=0;
    for(const hz of [72,90,120,36]) for(const miss of [false,true]) {
        simulator.frame({dt:1000/hz,controllers:{left:null,right:null}});
        const target=engine.scene.createMesh(app.world.geometry).setPosition([0,1.2,-1.1])
            .setScale([.13,.13,.13]).setCuttable(true,{capColor:[1,.83,.46]})
            .projectile([.2,.8,0],{gravity:0,bounce:0}).spin([.5,1.2,.3]);
        records.set(target.id,{volume:target.meshInfo.volume,shot:`fast-${hz}`});
        const steps=Math.ceil((Math.PI/3)/(40/hz));
        for(let i=0;i<=steps;i++) {
            rememberFlying();
            const angle=Math.PI/6-i/steps*Math.PI/3;
            simulator.frame({dt:1000/hz,controllers:{right:{
                matrix:simulator.gripX(angle,[miss?.65:0,1.35,-.45]),tracked:true,pressed:false
            }}});
            if(slow)await simulator.wait(1000/hz);
        }
        const hit=reports.some(report=>report.removed.includes(target.id));
        assert(hit===!miss,`fast wrist arc at ${hz} Hz ${miss?'respects finite miss':'cuts moving spinning fruit'}`);
        if(hit)fastHits++;
        else target.remove();
    }
    for(const hand of ['left','right']) for(const miss of [false,true]) {
        simulator.frame({dt:1000/36,controllers:{left:null,right:null}});
        const target=engine.scene.createMesh(app.world.geometry).setPosition([0,1.35,-1.2])
            .setScale([.13,.13,.13]).setCuttable(true,{capColor:[1,.83,.46]});
        records.set(target.id,{volume:target.meshInfo.volume,shot:'long-fast-arc'});
        for(const angle of [1.2,1.2-40/36,1.2-80/36]) {
            rememberFlying();
            simulator.frame({dt:1000/36,controllers:{[hand]:{
                matrix:simulator.gripX(angle,[miss?.65:0,1.35,-.45]),tracked:true,pressed:false
            }}});
            if(slow)await simulator.wait(1000/36);
        }
        const hit=reports.some(report=>report.handedness===hand && report.removed.includes(target.id));
        assert(hit===!miss,`${hand} long fast arc preserves completed cuts across stroke reset and ${miss?'misses distant fruit':'splits fruit'}`);
        if(hit)fastHits++;
        else target.remove();
    }
    checkCutProgress({engine,simulator,test,records,reports,rememberFlying});
    const fruit=(position)=>{
        const apple=engine.scene.createMesh(app.world.geometry).setPosition(position).setScale([.13,.13,.13])
            .setCuttable(true,{capColor:[1,.83,.46]}).projectile([0,0,0],{gravity:8,bounce:0});
        records.set(apple.id,{volume:apple.meshInfo.volume,shot:'blade-body'});return apple;
    };
    const flatQ=[0,0,Math.SQRT1_2,Math.SQRT1_2];
    for(const mode of ['flat','back']) {
        simulator.frame({...still,dt:1000/72,controllers:{left:null,right:null}});
        const target=fruit([0,1.35,-1.1]),control=fruit([2,1.35,-1.1]);
        const count=reports.length;
        for(let i=0;i<=6;i++) {
            rememberFlying();
            simulator.frame({dt:1000/72,controllers:{right:{position:[0,mode==='flat'?1.9-i*.15:.9+i*.15,-.45],
                orientation:mode==='flat'?flatQ:[0,0,0,1],pressed:false,tracked:true}}});
        }
        assert(target.exists && reports.length===count,`${mode} side makes physical contact without slicing fruit`);
        assert(Math.abs(target.meshInfo.position[1]-control.meshInfo.position[1])>.05,`${mode} side transfers a real impulse instead of passing through fruit`);
        target.remove();control.remove();
    }
    simulator.frame({...still,dt:1000/90,controllers:{left:null,right:null}});
    const held=fruit([0,1.35,-1.1]).removeAfter(3);
    for(let i=0;i<450;i++) {
        rememberFlying();
        const slide=Math.max(0,i-270)/180*.06;
        simulator.frame({dt:1000/90,controllers:{right:{position:[slide,1,-.45],orientation:flatQ,pressed:false,tracked:true}}});
        if(slow)await simulator.wait(1000/90);
    }
    assert(held.exists && held.meshInfo.bottom>0.98 && held.meshInfo.bottom<1.04,'flat blade supports real apple for five seconds without floating or expiry');
    assert(held.meshInfo.position[0]>.025,`friction carries supported fruit with the moving blade: ${JSON.stringify(held.meshInfo.position)}`);
    assert(!reports.some(report=>report.removed.includes(held.id)),'supporting fruit never creates a cut or score');
    onSnapshot('physics',simulator.pixels());
    const heldY=held.meshInfo.position[1];
    for(let i=0;i<30;i++)simulator.frame({dt:1000/90,controllers:{left:null,right:null}});
    assert(held.exists && held.meshInfo.position[1]<heldY-.2,'fruit falls freely when the supporting hand loses tracking');
    held.remove();
    assert(engine.debugStats().xrCuts===app.gameState().cuts && app.gameState().cuts===reports.reduce((sum,r)=>sum+r.removed.length,0),
        'VR score counts successful source splits, never the two resulting fragments');
    assert(engine.xr.roundState.duration===30 && engine.xr.roundState.remaining>0,'production game starts a 30 second round');
    let ended=0;
    engine.xr.addEventListener('roundend',()=>ended++);
    for(let i=0;i<310 && !engine.xr.roundState.finished;i++) {
        rememberFlying();
        simulator.frame({...still,dt:100});
        if(slow)await simulator.wait(10);
    }
    const finalScore=engine.xr.roundState.cuts;
    assert(engine.xr.roundState.finished && engine.xr.roundState.remaining===0 && ended===1,'round ends once at zero');
    onSnapshot('score',simulator.pixels());
    assert(simulator.timerCountForDelay(2000)===0 && !app.gameState().launching,'round end stops automatic firing');
    const endedShots=app.gameState().shots;
    for(let i=0;i<30;i++)simulator.frame({...still,dt:100});
    assert(engine.xr.roundState.cuts===finalScore && app.gameState().shots===endedShots && ended===1,'finished round keeps score and stays stopped');
    const buttonRay=simulator.gripX(0,[0,1.05,-.2]);
    const missRay=simulator.gripX(0,[-1,1.05,-.2]);
    const aim=(ray,pressed)=>({...still,dt:1000/72,controllers:{...still.controllers,
        left:{...still.controllers.left,targetRayMatrix:ray,pressed}}});
    simulator.frame(aim(missRay,false));simulator.frame(aim(missRay,true));
    assert(engine.xr.roundState.finished && simulator.timerCountForDelay(2000)===0,'trigger outside the 3D restart button does not start a round');
    simulator.frame(aim(buttonRay,false));
    assert(Math.abs(app.world.pointers[0].meshInfo.position[2]+1.65)<.02,'target ray shows a cursor on the real restart button');
    simulator.frame(aim(buttonRay,true));
    assert(engine.xr.roundState.remaining===30 && engine.xr.roundState.cuts===0 && app.gameState().cuts===0 && simulator.timerCountForDelay(2000)===1,
        '3D button plus trigger starts a fresh 30 second round with zero score and one launch timer');
    simulator.frame(aim(buttonRay,true));
    assert(app.gameState().shots===1,'holding the button trigger does not repeatedly restart');
    const beforeStall=app.gameState().liveApples[0].meshInfo.position[1];
    const beforeStallTime=engine.xr.roundState.remaining;
    simulator.frame({...still,dt:2000});
    assert(Math.abs(engine.xr.roundState.remaining-(beforeStallTime-2))<.001,'round clock counts a full two second stall while physics stays bounded');
    assert(app.gameState().liveApples[0].meshInfo.position[1]-beforeStall<1,'physics keeps a bounded step during the stall');
    for(let i=0;i<12;i++)simulator.frame({...still,dt:200});
    assert(Math.abs(engine.debugStats().fps-5)<.001 && Math.abs(engine.debugStats().frameMs-200)<.001,
        'scene FPS measures real XR intervals instead of the capped physics step');
    const rightAim=pressed=>({...still,dt:1000/72,controllers:{...still.controllers,
        right:{...still.controllers.right,targetRayMatrix:buttonRay,pressed}}});
    simulator.frame(rightAim(false));simulator.frame(rightAim(true));
    assert(engine.xr.roundState.remaining===30 && app.gameState().shots===1 && simulator.timerCountForDelay(2000)===1,
        'right controller can use the same 3D button to restart an active round');
    const bladeFrame=(hand,y,x=0)=>({dt:1000/30,controllers:{left:null,right:null,
        [hand]:{position:[x,y,-.95],orientation:[0,0,0,1],tracked:true,pressed:false}}});
    engine.xr.startRound(1);
    for(let i=0;i<35;i++)simulator.frame({...still,dt:1000/30,controllers:{left:null,right:null}});
    assert(engine.xr.roundState.finished && simulator.timerCountForDelay(2000)===0,'finished round remains ready for physical sword restart');
    for(const hand of ['left','right']) {
        simulator.frame({dt:1000/30,controllers:{left:null,right:null}});
        simulator.frame(bladeFrame(hand,1.25,.6));simulator.frame(bladeFrame(hand,.85,.6));
        assert(engine.xr.roundState.remaining<30,`${hand} sword misses outside restart box`);
        simulator.frame({dt:1000/30,controllers:{left:null,right:null}});
        simulator.frame(bladeFrame(hand,1.25));simulator.frame(bladeFrame(hand,.85));
        assert(engine.xr.roundState.remaining===30 && app.gameState().shots===1 && simulator.timerCountForDelay(2000)===1,
            `${hand} fast sword sweep through restart box starts one fresh round without trigger`);
        // Enter the box, then leave the blade resting inside it.
        simulator.frame(bladeFrame(hand,1.05));
        for(let i=0;i<8;i++)simulator.frame(bladeFrame(hand,1.05));
        assert(engine.xr.roundState.remaining<29.8 && app.gameState().shots===1 && simulator.timerCountForDelay(2000)===1,
            `${hand} blade resting in restart box does not repeatedly reset the round`);
        const beforeGap=engine.xr.roundState.remaining;
        simulator.frame({dt:1000/30,controllers:{left:null,right:null}});
        simulator.frame(bladeFrame(hand,1.05));
        assert(engine.xr.roundState.remaining<beforeGap,'tracking reacquisition inside restart box never synthesizes a strike');
    }
    await engine.xr.exit();
    assert(simulator.timerCountForDelay(2000)===0 && !app.gameState().launching,'VR end stops automatic firing');
    await engine.xr.enter();
    assert(simulator.timerCountForDelay(2000)===1,'VR reentry owns exactly one launch timer');
    await engine.xr.exit();
    return {shots:seenShots,hits:reports.length,fastHits,missVerified,lossVerified};
    } finally {
        if(engine?.xr.isPresenting)await engine.xr.exit();
        engine?.dispose();
    }
}
