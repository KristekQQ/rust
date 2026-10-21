// Renderer regression, independent of arena rules and gameplay fixtures.
export function verifyMainViewCaches(engine,sim,test,{cullingWasEnabled=true,observeLeft,readImportedWrites,singleImported=false}={}) {
    const head=[0,1.6,0];
    const frame=(position=head)=>{
        sim.frame({dt:0,head:{position,orientation:[0,0,0,1]}});
        test.assert(!engine.xr.lastError,'cache regression renders both eyes',String(engine.xr.lastError));
    };
    const edgeX=1.6*(sim.profile.eyeWidth/sim.profile.eyeHeight)/1.2;
    const edge=engine.scene.createMesh({positions:[-1,-1,-1,1,-1,-1,0,1,-1,0,0,1],indices:[0,2,1,0,1,3,1,2,3,2,0,3]})
        .setScale([.002,.002,.002]).setPosition([edgeX,1.6,-1.6]);
    try {
        engine.set_frustum_culling(true);
        frame();
        if(observeLeft)test.assert(engine.debugStats().visible===observeLeft()+1,'stereo cache fixture has different eye visibility');
        const warm=engine.debugStats().mainViewCache;
        frame();
        const stable=engine.debugStats().mainViewCache;
        test.assert(stable.slot===2&&stable.slots===3,'desktop and two XR main views own distinct bounded slots',stable);
        test.assert(stable.hits===warm.hits+2&&stable.misses===warm.misses,'identical stereo poses hit BOTH eye caches', {warm,stable});
        test.assert(stable.instanceUploadBytes===warm.instanceUploadBytes,'different visible eye lists reuse their own instance buffers',{warm,stable});
        frame([.08,1.6,0]);
        const moved=engine.debugStats().mainViewCache;
        test.assert(moved.misses===stable.misses+2,'head motion invalidates both eye views',{stable,moved});
        edge.setPosition([edgeX+.3,1.6,-1.6]);frame([.08,1.6,0]);
        const changed=engine.debugStats().mainViewCache;
        test.assert(changed.misses===moved.misses+2,'scene changes invalidate both eye views',{moved,changed});
        engine.set_frustum_culling(false);frame([.08,1.6,0]);
        const unculled=engine.debugStats().mainViewCache;
        test.assert(unculled.misses===changed.misses+2,'culling option changes invalidate both eye views',{changed,unculled});
        edge.setPosition([0,1.6,-1.6]);frame();
        const animationStart=engine.debugStats().mainViewCache;
        const writesStart=readImportedWrites?.();
        edge.setPosition([.04,1.6,-1.6]);frame();
        const animationEnd=engine.debugStats().mainViewCache;
        if(readImportedWrites)test.assert(readImportedWrites()-writesStart===1,'animated identical eye payload uploads one imported page, not two');
        if(singleImported)test.assert(animationEnd.instanceUploadBytes-animationStart.instanceUploadBytes===176,'animated identical eye payload uploads one instance, not two',{animationStart,animationEnd});
        test.report.evidence={...test.report.evidence,mainViewCache:{warm,stable,
            animatedUploadBytes:animationEnd.instanceUploadBytes-animationStart.instanceUploadBytes,
            animatedPageWrites:readImportedWrites?readImportedWrites()-writesStart:null}};
    } finally {
        edge.remove();engine.set_frustum_culling(cullingWasEnabled);frame();
    }
}
