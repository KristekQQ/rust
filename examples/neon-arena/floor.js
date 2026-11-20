// Baked tile appearance is the default; preserve the original mesh for A/B comparison.
const requestedFloor=new URLSearchParams(location.search).get('floor');
export const FLOOR_MODE=requestedFloor==='baseline'||requestedFloor==='normal'||requestedFloor==='texture'
    ? requestedFloor : 'spotlight';

/** @param {import('../../js/engine.js').Scene} scene
 * @param {'baseline'|'texture'|'normal'|'spotlight'} [mode] */
export async function createArenaFloor(scene,mode=FLOOR_MODE) {
    if(mode==='texture'||mode==='normal'||mode==='spotlight') {
        const floorTiles=await scene.loadModel(new URL(mode==='spotlight'?'./assets/neon-spotlight.gltf':`./assets/neon-floor-${mode}.gltf`,import.meta.url));
        floorTiles.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,reflectivity:.94});
        // Keep both scene-scoped handles valid; experimental floor has no separate grout geometry.
        return {floorTiles,floorGrout:floorTiles,floorMode:mode,
            floorMetadata:{kind:'model',vertices:mode==='spotlight'?28:4,triangles:mode==='spotlight'?14:2,primitives:floorTiles.info.primitives,reflectivity:.94}};
    }
    // One static draw for 80 individually bevelled tiles, with unreflective grout below.
    const tilePositions=[],tileIndices=[],tileColors=[];
    for(let row=0;row<10;row++)for(let col=0;col<8;col++){
        const x=(col-3.5)*.75,z=1.875-row*.75,b=tilePositions.length/3;
        for(const [half,y] of [[.363,0],[.371,-.012]])
            tilePositions.push(x-half,y,z-half,x+half,y,z-half,x+half,y,z+half,x-half,y,z+half);
        tileIndices.push(b,b+2,b+1,b,b+3,b+2);
        const shade=(row+col)%2===0?[.052,.073,.10]:[.032,.047,.070];
        tileColors.push(...shade,...shade);
        for(let side=0;side<4;side++){
            const next=(side+1)%4;
            tileIndices.push(b+side,b+next,b+4+next,b+side,b+4+next,b+4+side);
            tileColors.push(.021,.033,.047,.021,.033,.047);
        }
        tileIndices.push(b+4,b+5,b+6,b+4,b+6,b+7);
        tileColors.push(.012,.018,.025,.012,.018,.025);
    }
    const floorTiles=scene.createMesh({positions:tilePositions,indices:tileIndices,triangleColors:tileColors})
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,reflectivity:.94});
    const floorGrout=scene.createMesh({positions:[0,-.025].flatMap(y=>[-3,y,-5.25,3,y,-5.25,3,y,2.25,-3,y,2.25]),
        indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,1,2,6,1,6,5,2,3,7,2,7,6,3,0,4,3,4,7],color:[.006,.012,.020]})
        .setPosition([0,-.025,0])
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});
    return {floorTiles,floorGrout,floorMode:'baseline',
        floorMetadata:{kind:'mesh',vertices:648,triangles:972,primitives:2,reflectivity:.94}};
}
