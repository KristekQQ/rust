import {VR_DEBUG_PANEL_OFFSET,VR_DEBUG_PANEL_WIDTH,VR_DEBUG_PANEL_HEIGHT} from '../shared/vr-debug-panel.js';
export const PADDLE_FACE_OFFSET=.135; // along grip -Z, matches native contact center
export const DESKTOP_PADDLE_ROTATION=/** @type {[number,number,number]} */([Math.PI/2,0,Math.PI/2]);
/** Self-contained authoring geometry; simulation and tracking live in Rust. */
/** @param {number[]} color */
function box(color) {
    return {positions:[-.5,-.5,-.5,.5,-.5,-.5,.5,.5,-.5,-.5,.5,-.5,-.5,-.5,.5,.5,-.5,.5,.5,.5,.5,-.5,.5,.5],
        indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5],color:/** @type {[number,number,number]} */(color)};
}
/** WebXR grip mesh: handle along -Z toward thumb, face normal +X (back of right hand). */
/** @param {number[]} color */
function paddle(color) {
    const positions=/** @type {number[]} */([]),indices=[],triangleColors=[];
    const N=40,r=.085,thickness=.006;
    for(const z of [-thickness,thickness]) {
        positions.push(0,0,z);
        for(let i=0;i<N;i++){const a=i/N*Math.PI*2;positions.push(Math.cos(a)*r,Math.sin(a)*r,z);}
    }
    for(let i=0;i<N;i++) {
        const a=1+i,b=1+(i+1)%N,c=a+N+1,d=b+N+1;
        indices.push(0,b,a,N+1,c,d,a,b,d,a,d,c);
        triangleColors.push(...color,...color,...[.12,.09,.07],...[.12,.09,.07]);
    }
    const handle=box([.6,.36,.16]),base=positions.length/3;
    for(let i=0;i<handle.positions.length;i+=3)positions.push(handle.positions[i]*.032,handle.positions[i+1]*.11-PADDLE_FACE_OFFSET,handle.positions[i+2]*.022);
    indices.push(...handle.indices.map(i=>i+base));
    for(let i=0;i<handle.indices.length/3;i++)triangleColors.push(.6,.36,.16);
    for(let i=0;i<positions.length;i+=3){
        const x=positions[i],y=positions[i+1]+PADDLE_FACE_OFFSET,z=positions[i+2];
        positions[i]=z;positions[i+1]=-x;positions[i+2]=-y;
    }
    return {positions,indices,triangleColors};
}
/** Closed sphere with physical spin marks, authored once. */
function ballGeometry(){
    const positions=/** @type {number[]} */([0,1,0,0,-1,0]),indices=/** @type {number[]} */([]),triangleColors=/** @type {number[]} */([]);
    const segments=32,rings=16;
    for(let j=1;j<rings;j++)for(let i=0;i<segments;i++){
        const phi=j/rings*Math.PI,theta=i/segments*Math.PI*2;
        positions.push(Math.sin(phi)*Math.cos(theta),Math.cos(phi),Math.sin(phi)*Math.sin(theta));
    }
    for(let i=0;i<segments;i++){
        const next=(i+1)%segments,last=2+(rings-2)*segments;
        indices.push(0,2+next,2+i,1,last+i,last+next);
        for(let j=0;j<rings-2;j++){
            const a=2+j*segments+i,b=2+j*segments+next;
            indices.push(a,b,a+segments,b,b+segments,a+segments);
        }
    }
    for(let i=0;i<indices.length;i+=3){
        const v=indices.slice(i,i+3).map(k=>positions.slice(k*3,k*3+3));
        const x=v.reduce((a,p)=>a+p[0]/3,0),z=v.reduce((a,p)=>a+p[2]/3,0);
        triangleColors.push(...(Math.abs(x)<.11||Math.abs(z)<.11?[.06,.17,.25]:[1,.93,.75]));
    }
    return {positions,indices,triangleColors,smooth:true};
}
/** @param {import('../../js/engine.js').Scene} scene */
export function createScene(scene) {
    scene.clear();
    /** @param {number[]} p @param {number[]} scale @param {number[]} color */
    const block=(p,scale,color)=>scene.createMesh(box(color)).setPosition(/** @type {[number,number,number]} */(p)).setScale(/** @type {[number,number,number]} */(scale)).setRenderOptions({isStatic:true});
    block([0,-.05,-2.1],[10,.08,10],[.025,.035,.055]);
    block([0,.735,-2.2],[1.525,.05,2.74],[.025,.22,.32]);
    block([0,.68,-2.2],[1.56,.06,2.77],[.02,.06,.1]);
    for(const x of [-.62,.62])for(const z of [-1.1,-3.3])block([x,.33,z],[.065,.66,.065],[.08,.11,.15]);
    const white=[.8,.95,1];
    for(const x of [-.752,.752])block([x,.762,-2.2],[.02,.003,2.72],white);
    for(const z of [-.84,-3.56])block([0,.762,z],[1.52,.003,.02],white);
    block([0,.762,-2.2],[.008,.003,2.72],white);
    for(const x of [-.81,.81])block([x,.82,-2.2],[.024,.25,.024],[.25,.7,.8]);
    block([0,.9125,-2.2],[1.64,.008,.012],white);
    // Actual open mesh net, not an opaque wall.
    for(let x=-.79;x<=.8;x+=.04)block([x,.837,-2.2],[.002,.145,.002],[.35,.55,.6]);
    for(let y=.78;y<.91;y+=.026)block([0,y,-2.2],[1.6,.002,.002],[.35,.55,.6]);
    for(const x of [-2,2])block([x,.002,-2.2],[.012,.002,5],[.1,.4,.48]).setEmission(1.2);
    const player=scene.createMesh(paddle([.95,.14,.16])).setPosition([0,1-PADDLE_FACE_OFFSET,-.65]).setRotation(DESKTOP_PADDLE_ROTATION);
    const opponent=scene.createMesh(paddle([.12,.3,.95])).setPosition([0,1-PADDLE_FACE_OFFSET,-3.75]).setRotation(DESKTOP_PADDLE_ROTATION);
    const ball=scene.createMesh(ballGeometry()).setPosition([0,1,-1.3]).setScale([.02,.02,.02]).setEmission(.25);
    const scorePanel=scene.createTextPanel({title:'ODRAZY / SKORE',lines:['0 : 0'],position:[0,1.65,-4.05],width:1.3,height:.38});
    const titlePanel=scene.createTextPanel({title:'STOLNI TENIS',lines:['PRAVY TRIGGER: MICEK','LEVY: TRENINK / ZAPAS'],position:[0,2.15,-4.15],width:1.5,height:.36});
    scene.createLight({position:[0,4,-1],color:[1.4,1.4,1.4]});
    scene.createLight({position:[-3,2,-4],color:[.25,.55,.85]});
    const infoPanel=scene.createInfoPanel('diagnostics',{position:VR_DEBUG_PANEL_OFFSET,width:VR_DEBUG_PANEL_WIDTH,height:VR_DEBUG_PANEL_HEIGHT});
    return {ball,player,opponent,scorePanel,titlePanel,infoPanel};
}
