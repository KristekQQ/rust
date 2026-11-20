import {uv,faces} from './xp-print.js';
/** @param {import('../../js/engine.js').Scene} scene */
export function createXpDrink(scene) {
    /** @type {number[]} */ const positions=[];
    /** @type {number[]} */ const indices=[];
    /** @type {number[]} */ const triangleColors=[];
    /** @typedef {[number,number,number[]]} Ring */
    const pearl=[.86,.90,.94], silver=[.53,.62,.69], bright=[.90,.96,1];
    const opening=[.012,.019,.026];
    const inks=[pearl,[.013,.019,.026],[.055,.50,.73],[.62,.74,.27],[.82,.065,.31],[.23,.31,.36],[.63,.85,.92]];
    const segments=40;
    /** @param {number} angle */
    function metalLight(angle) {
        const highlight=Math.max(0,Math.cos(angle+.65));
        return Math.min(1,.58+.34*Math.cos(angle+.4)+.18*Math.pow(highlight,18));
    }
    /** @param {number} a @param {number} b @param {number} c @param {number[]} color */
    function tri(a,b,c,color) { indices.push(a,b,c);triangleColors.push(...color); }
    // Closed lathed solids share one mesh. Per-face inks wrap around both sides.
    /** @param {Ring[]} rows @param {number} n */
    function lathe(rows,n,offsetZ=0,depth=1,label=false) {
        const start=positions.length/3;
        for(const [y,r] of rows)for(let i=0;i<n;i++){
            const angle=2*Math.PI*i/n;
            positions.push(r*Math.sin(angle),y,offsetZ+depth*r*Math.cos(angle));
        }
        for(let j=0;j<rows.length-1;j++)for(let i=0;i<n;i++){
            if(label && j===3)continue; // Wrapped print below joins these two rim rings.
            const angle=2*Math.PI*(i+.5)/n;
            const tint=rows[j][2].map(channel=>channel*metalLight(angle));
            const a=start+j*n+i,b=start+j*n+(i+1)%n;
            tri(a,b,b+n,tint);tri(a,b+n,a+n,tint);
        }
        if(label){
            /** @type {number[]} */ const ids=[];
            const seam=new Map();
            for(let k=0;k<uv.length;k+=2){
                const u=uv[k],v=uv[k+1], col=(Math.round(u)+n/2)%n;
                if(v===0 || v===40){ids.push(start+(v===0?3:4)*n+col);continue;}
                const edge=u===0 || u===40;
                if(edge && seam.has(v)){ids.push(seam.get(v));continue;}
                const id=positions.length/3,angle=(u/n-.5)*2*Math.PI;
                ids.push(id);if(edge)seam.set(v,id);
                positions.push(.044*Math.sin(angle),.018+v/40*(.193-.018),.044*Math.cos(angle));
            }
            for(let k=0;k<faces.length;k+=4){
                const a=faces[k],b=faces[k+1],c=faces[k+2];
                const angle=((uv[a*2]+uv[b*2]+uv[c*2])/(3*n)-.5)*2*Math.PI;
                tri(ids[a],ids[b],ids[c],inks[faces[k+3]].map(channel=>channel*metalLight(angle)));
            }
        }
        const end=start+(rows.length-1)*n;
        for(let i=1;i<n-1;i++){
            tri(start,start+i+1,start+i,rows[0][2]);
            tri(end,end+i,end+i+1,rows[rows.length-1][2]);
        }
    }
    /** @type {Ring[]} */
    const rows=[[0,.036,silver],[.003,.041,bright],[.009,.040,pearl],[.018,.044,pearl],
        [.193,.044,pearl],[.201,.042,pearl],[.208,.037,silver],[.211,.036,silver],
        [.213,.0385,bright],[.217,.039,bright],[.219,.038,silver],[.219,.0355,silver],
        [.216,.034,silver],[.214,.033,silver]];
    lathe(rows,segments,0,1,true);
    lathe([[.2151,.013,opening],[.216,.013,opening]],16,.015,.72);
    // Raised oval pull tab with a real hole visible when the can tips.
    const start=positions.length/3,major=16,minor=6;
    for(let i=0;i<major;i++)for(let j=0;j<minor;j++){
        const a=2*Math.PI*i/major,b=2*Math.PI*j/minor;
        const tube=.0026*Math.cos(b);
        positions.push((.0105+tube)*Math.sin(a),.220+.0026*Math.sin(b),-.008+(.018+tube)*Math.cos(a));
    }
    for(let i=0;i<major;i++)for(let j=0;j<minor;j++){
        const a=start+i*minor+j,b=start+((i+1)%major)*minor+j;
        const c=start+((i+1)%major)*minor+(j+1)%minor,d=start+i*minor+(j+1)%minor;
        tri(a,b,c,bright);tri(a,c,d,bright);
    }
    return scene.createMesh({positions,indices,triangleColors,smooth:true})
        .setPosition([0,-10000,0]).setEmission(.63)
        .setRenderOptions({castsShadow:false,receivesShadow:false,reflectivity:.08});
}
