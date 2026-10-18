// Reuse the real isolated halves from the smooth-slice scenario; do not spawn mock quarters.
export function checkRecuts({engine,simulator,test,reports,pieces,head,rememberFlying,onSnapshot}) {
    const live=new Map(pieces.map(piece=>[piece.id,piece]));
    const volume=()=>[...live.values()].reduce((sum,piece)=>sum+piece.meshInfo.volume,0);
    const initialVolume=volume();
    let cursor=reports.length;
    const collect=()=>{
        for(const report of reports.slice(cursor)) if(report.removed.some(id=>live.has(id))) {
            for(const id of report.removed)live.delete(id);
            for(const piece of report.pieces)live.set(piece.id,piece);
        }
        cursor=reports.length;
    };
    const center=()=>{
        const info=[...live.values()].map(piece=>piece.meshInfo),total=volume();
        return [0,1,2].map(axis=>info.reduce((sum,piece)=>sum+piece.position[axis]*piece.volume,0)/total);
    };
    test.assert([...live.values()].every(piece=>piece.meshInfo.cuttable),'both original halves remain eligible for another sword cut');
    for(const hand of ['left','right']) {
        simulator.frame({dt:1000/90,head,controllers:{left:null,right:null}});
        const parents=new Set(live.keys()),beforeCount=live.size;
        for(let i=0;i<=18 && [...parents].some(id=>live.has(id));i++) {
            const c=center();
            const pose=hand==='left'
                ? {position:[c[0]-.4+i*.05,c[1],-.45],orientation:[0,0,Math.SQRT1_2,Math.SQRT1_2]}
                : {position:[c[0]+.55,c[1]+.3-i*.04,c[2]],orientation:[0,Math.SQRT1_2,0,Math.SQRT1_2]};
            rememberFlying();
            simulator.frame({dt:1000/90,head,controllers:{left:null,right:null,[hand]:{...pose,tracked:true,pressed:false}}});
            collect();
        }
        const completed=[...parents].every(id=>!live.has(id));
        test.assert(completed,`${hand} sword genuinely recuts every preceding half or quarter in another plane${completed?'':': '+JSON.stringify([...live.values()].map(p=>({id:p.id,parent:parents.has(p.id),...p.meshInfo})))}`);
        test.assert(live.size===beforeCount*2,`${hand} cross-cut creates ${beforeCount*2} distinct closed descendants`);
        test.assert(Math.abs(volume()-initialVolume)<initialVolume*.0002,'repeated cross-cuts preserve the full original apple volume');
        test.assert([...live.values()].every(piece=>piece.meshInfo.closed),'all recut descendants stay closed 3D solids');
    }
    const c=center(),inspectionHead={position:[c[0],c[1]+.55,c[2]+1],orientation:[Math.sin(-.25),0,0,Math.cos(-.25)]};
    for(let i=0;i<6;i++)simulator.frame({dt:1000/90,head:inspectionHead,controllers:{left:null,right:null}});
    onSnapshot('recut',simulator.pixels());
    for(const piece of live.values())piece.remove();
}
