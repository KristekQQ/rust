// Known cube sections give exact area fractions without guessing apple bounds.
const cube={positions:[-1,-1,-1,1,-1,-1,1,1,-1,-1,1,-1,-1,-1,1,1,-1,1,1,1,1,-1,1,1],indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5]};
export function checkCutProgress({engine,simulator,test,records,reports,rememberFlying}) {
    const frame=(p)=>{
        rememberFlying();
        simulator.frame({dt:1000/30,controllers:{left:null,right:{position:[0,1.6225-.4*p,-.45],orientation:[0,0,0,1],tracked:true,pressed:false}}});
    };
    for(const fraction of [.25,.5,.6,.74,.75,.9]) {
        simulator.frame({dt:1000/30,controllers:{left:null,right:null}});
        const target=engine.scene.createMesh(cube).setPosition([0,1.4,-1.1]).setScale([.2,.2,.2]).setCuttable();
        records.set(target.id,{volume:target.meshInfo.volume,shot:'section-progress'});
        frame(-.2);frame(fraction);
        const hit=reports.some(report=>report.removed.includes(target.id));
        test.assert(hit===(fraction>=.75),`${Math.round(fraction*100)}% actual section ${fraction>=.75?'splits':'stays whole'}`);
        if(target.exists) {
            const info=target.meshInfo;
            test.assert(Math.abs(info.cutProgress-fraction)<.002,`section progress reports actual ${Math.round(fraction*100)}% area: ${info.cutProgress}`);
            test.assert((info.jammedBy!==null)===(fraction>=.5),`${Math.round(fraction*100)}% section has correct blade jam state`);
            if(fraction===.6) {
                simulator.frame({dt:1000/30,controllers:{left:null,right:null}});
                test.assert(target.meshInfo.jammedBy===null,'tracking loss releases wedged fruit');
            }
            target.remove();
        }
    }
}
