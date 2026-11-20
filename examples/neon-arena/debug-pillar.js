/** A subdued fragment of fluted stone; one static mesh, baked directional tint.
 * @param {import('../../js/engine.js').Scene} scene */
export function createDebugPillar(scene){
 /** @type {number[]} */ const positions=[];
 /** @type {number[]} */ const indices=[];
 /** @type {number[]} */ const triangleColors=[];
 /** @param {number} segments @param {number[][]} rings @param {boolean} fluted */
 const stone=(segments,rings,fluted)=>{
  const first=positions.length/3;
  for(const [y,radius] of rings)for(let i=0;i<segments;i++){
   const a=i*Math.PI*2/segments,r=radius*(fluted&&i%2?.90:1);
   positions.push(Math.cos(a)*r,y,Math.sin(a)*r);
  }
  /** @param {number[]} face @param {number} shade */
  const triangle=(face,shade)=>{indices.push(...face);triangleColors.push(shade*.95,shade*.98,shade);};
  for(let ring=0;ring<rings.length-1;ring++)for(let i=0;i<segments;i++){
   const j=(i+1)%segments,a=first+ring*segments+i,b=first+ring*segments+j,c=b+segments,d=a+segments;
   const side=.23+.77*Math.max(0,Math.cos((i+.5)*Math.PI*2/segments-2.3));
   const height=(rings[ring][0]+rings[ring+1][0])/2;
   const shade=(.065*side+.006)*(height>4?.12:height>2.2?.35:1);
   triangle([a,d,c],shade);triangle([a,c,b],shade);
  }
  const bottom=positions.length/3;positions.push(0,rings[0][0],0);
  const top=positions.length/3;positions.push(0,rings[rings.length-1][0],0);
  for(let i=0;i<segments;i++){
   const j=(i+1)%segments;triangle([bottom,first+i,first+j],.018);
   const last=first+(rings.length-1)*segments;triangle([top,last+j,last+i],.025);
  }
 };
 stone(8,[[0,.88],[.18,.88]],false);
 stone(16,[[.18,.72],[2.2,.68],[4,.64],[6,.60]],true);
 // A restrained rim suggests classical stone without a decorative capital.
 stone(8,[[5.75,.76],[6,.76]],false);
 return scene.createMesh({positions,indices,triangleColors,smooth:false})
  .setPosition([2.1,0,-1.1]).setEmission(0)
  .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});
}
