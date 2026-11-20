/** Relaxed hand in XR grip space. Fingers point forward (-Z), thumb points up (+Y).
 * @param {import('../../js/engine.js').Scene} scene
 * @param {boolean} left */
export function createPetHand(scene, left) {
    /** @type {number[]} */
    const positions=[];
    /** @type {number[]} */
    const indices=[];
    /** @type {number[]} */
    const triangleColors=[];
    // Bake the luminous sides into the existing eight-sided cross-sections.
    const core=left?[.036,.21,.26]:[.20,.065,.24];
    const edge=left?[.11,.84,1.0]:[.84,.35,1.0];
    const cuff=left?[.48,.93,1.0]:[.98,.65,1.0];
    const sides=8, mirror=left?-1:1;
    /** @param {number} a @param {number} b @param {number} c @param {number[]} color */
    function triangle(a,b,c,color) {
        indices.push(a,left?c:b,left?b:c);
        triangleColors.push(...color);
    }
    // Each section describes center x/y/z and elliptical radii in metres.
    /** @param {Array<[number, number, number, number, number]>} sections @param {number[]} color */
    function shape(sections,color) {
        const start=positions.length/3;
        for(const [x,y,z,rx,ry] of sections) {
            for(let i=0;i<sides;i++){
                const angle=2*Math.PI*i/sides;
                // Roll each mirrored palm a quarter turn about the finger axis.
                // Grip-space palm normals point inward: +X on the left, -X on the right.
                const across=x+rx*Math.cos(angle), height=y+ry*Math.sin(angle);
                positions.push(mirror*height,-across,z);
            }
        }
        for(let ring=1;ring<sections.length;ring++)for(let i=0;i<sides;i++){
            const a=start+(ring-1)*sides+i,b=start+(ring-1)*sides+(i+1)%sides;
            const onEdge=i===0 || i===3 || i===4 || i===7;
            const tint=color===core && onEdge ? edge : color;
            triangle(a,a+sides,b+sides,tint);
            triangle(a,b+sides,b,tint);
        }
        const end=start+(sections.length-1)*sides;
        for(let i=1;i<sides-1;i++){
            triangle(start,start+i,start+i+1,color);
            triangle(end,end+i+1,end+i,color===core ? edge : color);
        }
    }
    shape([[0,0,.055,.026,.016],[0,0,.02,.031,.018],[-.002,0,-.025,.043,.017],[0,-.002,-.05,.040,.013]],core);
    const fingers=[[-.029,-.113,.010],[-.009,-.124,.011],[.012,-.117,.010],[.031,-.100,.008]];
    for(const [x,tip,r] of fingers){
        shape([[x,0,-.041,r,.012],[x*1.07,-.003,-.066,r,.010],
            [x*1.13,-.009,tip+.013,r*.87,.008],[x*1.14,-.013,tip,r*.50,.005]],core);
    }
    shape([[-.023,-.002,.014,.015,.013],[-.042,-.004,-.010,.015,.012],
        [-.054,-.009,-.038,.012,.010],[-.061,-.013,-.055,.007,.006]],core);
    shape([[0,0,.066,.028,.019],[0,0,.043,.031,.020]],cuff);
    return scene.createMesh({positions,indices,triangleColors,smooth:true})
        .setPosition([0,-10000,0]).setEmission(1.15).setOpacity(.55,{opaqueAbove:.48})
        .setRenderOptions({castsShadow:false,receivesShadow:false,reflectivity:0});
}
