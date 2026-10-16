/** @typedef {import('../../js/engine.js').Scene} Scene */
export const BOARD_Y = 0.2;

/** @param {import('../../js/engine.d.ts').Vector3} color @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function box(color) {
    return {color, positions:[-1,-1,-1,1,-1,-1,1,1,-1,-1,1,-1,-1,-1,1,1,-1,1,1,1,1,-1,1,1],
        indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5]};
}

/** Lobed shoulders and a sunken crown, rather than a scaled sphere.
 * @param {number} [segments] @param {number} [rings]
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
export function appleGeometry(segments=32, rings=18) {
    const positions = [0, BOARD_Y, 0, 0, BOARD_Y + 1.92, 0];
    const indices = [];
    for (let j = 1; j < rings; j++) {
        const theta = j / rings * Math.PI;
        for (let i = 0; i < segments; i++) {
            const phi = i / segments * Math.PI * 2;
            const radius = Math.sin(theta) * 1.1 * (1 - .1 * Math.cos(theta)) * (1 + .045 * Math.cos(phi * 5));
            const y = BOARD_Y + 1.1 * (1 - Math.cos(theta)) - .28 * Math.exp(-(((Math.PI-theta)/.38) ** 2));
            positions.push(Math.cos(phi)*radius, y, Math.sin(phi)*radius);
        }
    }
    for (let i = 0; i < segments; i++) {
        const next = (i + 1) % segments;
        indices.push(0, 2+i, 2+next);
        const top = 2 + (rings-2)*segments;
        indices.push(1, top+next, top+i);
        for (let j = 0; j < rings-2; j++) {
            const b = 2+j*segments+i, bn = 2+j*segments+next, t = b+segments, tn = bn+segments;
            indices.push(b,t,tn,b,tn,bn);
        }
    }
    return {positions, indices, color: [.72, .022, .014], smooth: true};
}

/** @param {boolean} held @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function swordGeometry(held) {
    /** @type {number[]} */ const positions=[];
    /** @type {number[]} */ const indices=[];
    /** @type {number[]} */ const triangleColors=[];
    /** @param {number[]} size @param {number[]} position @param {number[]} color */
    const part=(size,position,color)=> {
        const source=box([1,1,1]),base=positions.length/3;
        for(let i=0;i<source.positions.length;i+=3)positions.push(source.positions[i]*size[0]/2+position[0],source.positions[i+1]*size[1]/2+position[1],source.positions[i+2]*size[2]/2+position[2]);
        indices.push(...Array.from(source.indices,i=>i+base));
        for(let i=0;i<source.indices.length;i+=3)triangleColors.push(...color);
    };
    part([.1,.2,5.3],[0,0,0],[.72,.8,.88]);
    part([.75,.16,.16],[0,0,-2.8],[.3,.2,.07]);
    part([.15,.15,.95],[0,0,-3.3],[.09,.045,.02]);
    if(held) {
        part([.6,.58,.95],[.42,-.01,-3.3],[.65,.34,.17]);
        for(let i=0;i<4;i++)part([.68,.17,.18],[.12,-.16,-2.96-i*.23],[.75,.41,.22]);
        part([.5,.5,.65],[.45,-.015,-4.0],[.62,.3,.13]);
        part([.65,.6,1.0],[.45,-.015,-4.8],[.045,.075,.09]);
    }
    return {positions,indices,triangleColors};
}
/** @param {Scene} scene @param {boolean} held */
export function createSword(scene,held) {
    return scene.createMesh(swordGeometry(held)).setPosition([0,3.6,0]).setCutter({base:[0,-.1,-2.65],tip:[0,-.1,2.65]});
}

/** @param {Scene} scene */
export function createScene(scene,held=true) {
    scene.clear();
    const table = scene.createMesh(box([.16,.07,.035])).setScale([45,.2,45]).setPosition([0,-.35,0]).setRenderOptions({isStatic:true});
    const board = scene.createMesh(box([.52,.31,.14])).setScale([5.3,.14,3.8]).setPosition([0,.06,-.6]).setRenderOptions({isStatic:true});
    // Quiet wood-grain seams on the chopping board, all non-cuttable.
    for (let i = -6; i <= 6; i++) scene.createMesh(box([.39+i*.005,.22+i*.004,.10])).setScale([.006,.001,3.7]).setPosition([i*.76,BOARD_Y+.001,-.6]).setRenderOptions({isStatic:true});
    const apples=[[0,0],[-2.65,.15],[2.65,.15],[-1.35,-2.4],[1.35,-2.4]].map(([x,z])=> {
        const apple=scene.createMesh(appleGeometry()).setPosition([x,0,z]).setCuttable(true,{capColor:[1,.83,.46]});
        const stem=scene.createMesh(box([.17,.065,.018])).setScale([.045,.23,.045]).setPosition([x,2.3,z]).setRotation([0,0,-.2]);
        const leaf=scene.createMesh({positions:[1,0,0,-1,0,0,0,1,0,0,-1,0,0,0,1,0,0,-1],indices:[0,2,4,2,1,4,1,3,4,3,0,4,2,0,5,1,2,5,3,1,5,0,3,5],color:[.12,.32,.035]})
            .setScale([.5,.045,.22]).setPosition([x+.35,2.47,z]).setRotation([0,.3,.3]);
        return {apple,stem,leaf,center:/** @type {import('../../js/engine.d.ts').Vector3} */([x,1.15,z])};
    });
    const sword=createSword(scene,held);
    const guides=Array.from({length:24},()=>scene.createMesh(box([.16,1,.12])).setRenderOptions({castsShadow:false,receivesShadow:false}));
    scene.createLight({position:[-4,9,5],color:[1,.94,.82]});
    scene.createLight({position:[5,7,-4],color:[.65,.76,1]});
    return {apples,sword,guides,table,board};
}
