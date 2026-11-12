// Observer-owned avatar geometry. The sender provides its world-space transform.
import {swordGeometry} from '../../examples/neon-arena/geometry.js';
export function createNativeSwords(scene){
    return ['left','right'].map(hand=>scene.createMesh(swordGeometry(hand)).setEmission(1.4).setPosition([0,-10000,0]));
}
export function createHelmet(scene){
    const positions=[],indices=[],triangleColors=[];
    const tri=(a,b,c,color)=>{indices.push(a,b,c);triangleColors.push(...color);};
    const box=(center,size,color)=>{
        const base=positions.length/3;
        for(const [x,y,z] of [[-1,-1,-1],[1,-1,-1],[1,1,-1],[-1,1,-1],[-1,-1,1],[1,-1,1],[1,1,1],[-1,1,1]])
            positions.push(center[0]+x*size[0]/2,center[1]+y*size[1]/2,center[2]+z*size[2]/2);
        for(const [a,b,c,d] of [[0,3,2,1],[4,5,6,7],[0,4,7,3],[1,2,6,5],[0,1,5,4],[3,7,6,2]]){tri(base+a,base+b,base+c,color);tri(base+a,base+c,base+d,color);}
    };
    // Low-poly head gives a readable silhouette behind the headset and straps.
    const rings=8,sides=12;
    positions.push(0,.145,0);
    for(let ring=1;ring<rings;ring++)for(let side=0;side<sides;side++){
        const latitude=Math.PI*ring/rings,longitude=2*Math.PI*side/sides;
        positions.push(.115*Math.sin(latitude)*Math.cos(longitude),.145*Math.cos(latitude),.12*Math.sin(latitude)*Math.sin(longitude));
    }
    const bottom=positions.length/3;positions.push(0,-.145,0);
    const vertex=(ring,side)=>1+(ring-1)*sides+side%sides;
    for(let side=0;side<sides;side++){
        tri(0,vertex(1,side+1),vertex(1,side),[.23,.31,.4]);
        tri(vertex(rings-1,side),vertex(rings-1,side+1),bottom,[.23,.31,.4]);
        for(let ring=1;ring<rings-1;ring++){
            const a=vertex(ring,side),next=vertex(ring,side+1),b=vertex(ring+1,side),bn=vertex(ring+1,side+1);
            tri(a,next,b,[.23,.31,.4]);tri(next,bn,b,[.23,.31,.4]);
        }
    }
    box([0,.01,-.12],[.255,.12,.095],[.055,.075,.105]);
    box([0,.01,-.171],[.215,.077,.012],[.045,.65,.85]);
    box([-.119,.01,.005],[.02,.042,.2],[.07,.1,.14]);
    box([.119,.01,.005],[.02,.042,.2],[.07,.1,.14]);
    box([0,.13,.005],[.038,.022,.16],[.07,.1,.14]);
    return scene.createMesh({positions,indices,triangleColors}).setEmission(.6).setPosition([0,-10000,0]);
}
