import {checkRecuts} from './recut.js';
// Isolate a production apple so unrelated flying fruit cannot hide a fragment jump.
export function checkSliceMotion({engine,app,simulator,test,records,reports,rememberFlying,onSnapshot}) {
    const scale=.13,head={position:[1.3,1.55,.15]};
    simulator.frame({dt:1000/90,head,controllers:{left:null,right:null}});
    const target=engine.scene.createMesh(app.world.geometry).setPosition([1.5,1.25,-1.1]).setScale([scale,scale,scale])
        .setCuttable(true,{capColor:[1,.83,.46]}).projectile([0,.25,0],{gravity:0,bounce:0});
    records.set(target.id,{volume:target.meshInfo.volume,shot:'smooth-slice'});
    let report;
    for(let i=0;i<=16 && !report;i++) {
        rememberFlying();
        simulator.frame({dt:1000/90,head,controllers:{left:null,right:{position:[1.5,1.8-i*.045,-.45],orientation:[0,0,0,1],tracked:true,pressed:false}}});
        report=reports.find(report=>report.removed.includes(target.id));
    }
    test.assert(!!report && report.pieces.length===2,'controlled apple creates two real mesh halves');
    const pieces=report.pieces.slice().sort((a,b)=>a.meshInfo.position[0]-b.meshInfo.position[0]);
    const initial=pieces.map(piece=>piece.meshInfo);
    const extent=(piece,info)=>piece.meshSnapshot.positions.map(p=>info.position[0]+p[0]*scale);
    const gap=Math.min(...extent(pieces[1],initial[1]))-Math.max(...extent(pieces[0],initial[0]));
    test.assert(gap>=-.0001 && gap<.003,`new cut starts at the actual blade plane without a visible pop: gap ${gap}`);
    const firstPixels=simulator.pixels(),eyeWidth=firstPixels.width/2;
    for(let eye=0;eye<2;eye++) {
        let skin=0;
        for(let y=Math.floor(firstPixels.height*.35);y<firstPixels.height*.9;y++)
            for(let x=Math.floor(eyeWidth*.45);x<eyeWidth*.85;x++) {
                const offset=(y*firstPixels.width+eye*eyeWidth+x)*4;
                const [r,g,b]=firstPixels.data.subarray(offset,offset+3);
                if(r>100 && r>g*1.5 && r>b*1.3)skin++;
            }
        test.assert(skin>300,`new halves are already rendered in the very first cut frame in eye ${eye}: ${skin} skin pixels`);
    }
    onSnapshot('slice-start',firstPixels);
    const separation=()=>pieces[1].meshInfo.position[0]-pieces[0].meshInfo.position[0];
    const startDistance=separation();
    simulator.frame({dt:1000/90,head,controllers:{left:null,right:null}});
    const after=pieces.map(piece=>piece.meshInfo);
    test.assert(after.every((info,i)=>Math.hypot(...info.position.map((p,k)=>p-initial[i].position[k]))<.02),
        'first fragment frame stays continuous instead of resolving overlapping whole-fruit spheres');
    for(let i=0;i<15;i++) {
        rememberFlying();
        simulator.frame({dt:1000/90,head,controllers:{left:null,right:null}});
    }
    const opening=separation()-startDistance;
    test.assert(opening>.003 && opening<.08,`halves progressively open along the cut normal over 178ms: ${opening}`);
    test.assert(pieces.every((piece,i)=>piece.meshInfo.position[1]<initial[i].position[1]),'both actual halves fall naturally after the cut');
    onSnapshot('slice-open',simulator.pixels());
    checkRecuts({engine,simulator,test,reports,pieces,head,rememberFlying,onSnapshot});
}
