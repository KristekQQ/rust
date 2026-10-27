import test from 'node:test';
import assert from 'node:assert/strict';
// Independent scalar oracle: compare underlying depth texels BEFORE bilinear
// filtering, including clamp-to-edge. This does not model GPU weight precision.
function compareLinear(depth,n,u,v){
 const px=u*n-.5,py=v*n-.5,x=Math.floor(px),y=Math.floor(py),fx=px-x,fy=py-y;
 const sample=(ix,iy)=>.5<=depth[Math.max(0,Math.min(n-1,iy))*n+Math.max(0,Math.min(n-1,ix))]?1:0;
 return(sample(x,y)*(1-fx)+sample(x+1,y)*fx)*(1-fy)+(sample(x,y+1)*(1-fx)+sample(x+1,y+1)*fx)*fy;
}
function nine(depth,n,u,v){let sum=0;for(let y=-1;y<=1;y++)for(let x=-1;x<=1;x++)sum+=compareLinear(depth,n,u+x/n,v+y/n);return sum/9;}
function four(depth,n,u,v){
 const p=[u*n-.5,v*n-.5],base=p.map(Math.floor),f=p.map((v,i)=>v-base[i]),a=f.map(v=>2-v),b=f.map(v=>1+v),low=base.map((v,i)=>(v-.5+1/a[i])/n),high=base.map((v,i)=>(v+1.5+f[i]/b[i])/n);
 return(a[0]*a[1]*compareLinear(depth,n,low[0],low[1])+b[0]*a[1]*compareLinear(depth,n,high[0],low[1])+a[0]*b[1]*compareLinear(depth,n,low[0],high[1])+b[0]*b[1]*compareLinear(depth,n,high[0],high[1]))/9;
}
test('four bilinear fetches preserve all65536 underlying4x4 comparison patterns',t=>{
 const phases=[0,1e-12,.125,.25,.5,.75,.875,1-1e-12],depth=new Float64Array(16);let maximum=0,count=0;
 for(let bits=0;bits<65536;bits++){
  for(let i=0;i<16;i++)depth[i]=(bits>>i)&1;
  for(let i=0;i<phases.length;i++){const u=(1.5+phases[i])/4,v=(1.5+phases[(i*3+1)%phases.length])/4,error=Math.abs(nine(depth,4,u,v)-four(depth,4,u,v));maximum=Math.max(maximum,error);count++;}
 }
 assert.ok(maximum<2e-14,'kernel error '+maximum);assert.equal(count,524288);t.diagnostic(JSON.stringify({patterns:65536,phasePairs:8,cases:count,maxAbsoluteError:maximum}));
});
test('clamp edges, corners, tiny maps, arbitrary depth fields and subtexel boundaries',t=>{
 let seed=1337;const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296;};let maximum=0,count=0;
 for(const n of [1,2,4,8,33])for(let map=0;map<64;map++){
  const depth=Float64Array.from({length:n*n},random),coords=[[0,0],[1,1],[0,1],[1,0],[1e-12,1-1e-12],[.5,.5],[-.01,1.01]];
  for(let k=0;k<32;k++)coords.push([random(),random()]);
  for(const [u,v] of coords){const a=nine(depth,n,u,v),b=four(depth,n,u,v);maximum=Math.max(maximum,Math.abs(a-b));assert.ok(b>=-1e-14&&b<=1+1e-14);count++;}
 }
 assert.ok(maximum<2e-14,'clamp error '+maximum);t.diagnostic(JSON.stringify({cases:count,maxAbsoluteError:maximum}));
});
test('static/dynamic per-tap minimum must retain the original nine-tap path',()=>{
 const a=Float64Array.from({length:16},(_,i)=>((i%4)+(i>>2))%2),b=Float64Array.from(a,v=>1-v),n=4,u=.43,v=.61;let perTap=0;
 for(let y=-1;y<=1;y++)for(let x=-1;x<=1;x++)perTap+=Math.min(compareLinear(a,n,u+x/n,v+y/n),compareLinear(b,n,u+x/n,v+y/n));
 const incorrectlyMovedMinimum=Math.min(four(a,n,u,v),four(b,n,u,v));assert.ok(Math.abs(perTap/9-incorrectlyMovedMinimum)>.01);
});
function uniformSupport(depth,n,u,v){
 const x=Math.floor(u*n-.5),y=Math.floor(v*n-.5),samples=[];for(const dy of [0,2])for(const dx of [0,2])samples.push(compareLinear(depth,n,(x+dx)/n,(y+dy)/n));
 return samples.every(v=>v===1)?1:samples.every(v=>v===0)?0:null;
}
test('four disjoint support samples only bypass the original kernel for uniform comparison texels',t=>{
 const depth=new Float64Array(16),phases=[0,1e-12,.125,.25,.5,.75,.875,1-1e-12];let checks=0,earlyouts=0,maximum=0;
 for(let bits=0;bits<65536;bits++){for(let i=0;i<16;i++)depth[i]=(bits>>i)&1;for(let p=0;p<8;p++){const u=(1.5+phases[p])/4,v=(1.5+phases[(p*3+1)%8])/4,coarse=uniformSupport(depth,4,u,v);checks++;if(coarse!==null){earlyouts++;maximum=Math.max(maximum,Math.abs(coarse-nine(depth,4,u,v)));}}}
 assert.equal(checks,524288);assert.equal(earlyouts,16);assert.ok(maximum<2e-14);t.diagnostic(JSON.stringify({checks,earlyouts,maxAbsoluteError:maximum}));
});
test('f32 boundary guard retains every texel addressed by the original three taps',t=>{
 const f=Math.fround,eps=2**-23,phases=[0,1e-12,1e-8,1e-6,1e-4,.001,.005,.02,.1,.25,.5,.75,.9,.98,.995,.999,1-1e-8];let checked=0,fallback=0;
 const support=(u,n,rounded)=>{const p=rounded?f(f(u*n)-.5):u*n-.5,base=Math.floor(p),fraction=p-base,out=new Set();if(1-fraction>0)out.add(Math.max(0,Math.min(n-1,base)));if(fraction>0)out.add(Math.max(0,Math.min(n-1,base+1)));return out;};
 for(const n of [1,2,3,8,33,256,2048,4096,8192,16384])for(let k=0;k<64;k++)for(const phase of phases){
  const cell=Math.floor(k*(n-1)/63),u=f(Math.max(0,Math.min(1,(cell+.5+phase)/n))),texel=f(1/n),position=f(f(u/texel)-.5),base=Math.floor(position),fraction=f(position-base),margin=f(f(16*eps*f(1/texel))+1/256);
  if(!(fraction>margin&&fraction<f(1-margin))){fallback++;continue;}
  const coarse=[f(base*texel),f((base+2)*texel)];
  for(const rounded of [false,true]){const all=new Set(coarse.flatMap(v=>[...support(v,n,rounded)]));for(const offset of [-1,0,1])for(const index of support(f(u+f(offset*texel)),n,rounded))assert.ok(all.has(index),JSON.stringify({n,u,phase,rounded,index,all:[...all],position,margin}));checked++;}
 }
 assert.ok(checked>1000&&fallback>1000);t.diagnostic(JSON.stringify({checkedHardwareAddressCases:checked,boundaryFallbacks:fallback,maxMapSize:16384}));
});
