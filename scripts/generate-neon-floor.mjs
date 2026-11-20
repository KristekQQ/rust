#!/usr/bin/env node
// Deterministic authored floor. Run from any directory: node scripts/generate-neon-floor.mjs.
import {writeFile, mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {PNG} from 'pngjs';

const root=new URL('../',import.meta.url);
const size=512, tile=.75, repeat=tile*2;
const color=new PNG({width:size,height:size}), normal=new PNG({width:size,height:size});
const srgb=x=>Math.round(255*(x<=.0031308?12.92*x:1.055*x**(1/2.4)-.055));
const smooth=x=>{const t=Math.max(0,Math.min(1,x));return t*t*(3-2*t);};
const height=(u,v)=>{
    const x=u-Math.floor(u),y=v-Math.floor(v);
    return .002*smooth((Math.min(x,1-x,y,1-y)*tile-.004)/.013);
};
for(let y=0;y<size;y++)for(let x=0;x<size;x++){
    const u=(x+.5)*2/size,v=(y+.5)*2/size,fx=u%1,fy=v%1;
    const edge=Math.min(fx,1-fx,fy,1-fy)*tile;
    const checker=(Math.floor(u)+Math.floor(v)+1)%2;
    const top=checker?[.032,.047,.070]:[.052,.073,.10],grout=[.006,.012,.020];
    const bevel=smooth((edge-.004)/.009);
    // Subtle broad stone grain, no subpixel high-contrast sparkle.
    const grain=1+.014*Math.sin(u*31.4159265359)*Math.sin(v*25.1327412287);
    const i=(y*size+x)*4;
    for(let c=0;c<3;c++)color.data[i+c]=srgb(grout[c]*(1-bevel)+top[c]*grain*bevel);
    color.data[i+3]=255;
    const step=2/size;
    const dx=(height(u+step,v)-height(u-step,v))/(2*step*tile);
    const dy=(height(u,v+step)-height(u,v-step))/(2*step*tile);
    const length=Math.hypot(dx,dy,1);
    normal.data[i]=Math.round(255*(.5-.5*dx/length));
    normal.data[i+1]=Math.round(255*(.5-.5*dy/length));
    normal.data[i+2]=Math.round(255*(.5+.5/length));normal.data[i+3]=255;
}
const positions=new Float32Array([-3,0,-5.25,3,0,-5.25,3,0,2.25,-3,0,2.25]);
const normals=new Float32Array([0,1,0,0,1,0,0,1,0,0,1,0]);
const tangents=new Float32Array([1,0,0,-1,1,0,0,-1,1,0,0,-1,1,0,0,-1]);
const uvs=new Float32Array([0,0,4,0,4,5,0,5]);
const indices=new Uint16Array([0,2,1,0,3,2]);
const arrays=[positions,normals,tangents,uvs,indices];
const geometry=Buffer.concat(arrays.map(a=>Buffer.from(a.buffer)));
let offset=0;
const views=arrays.map((a,i)=>{const view={buffer:0,byteOffset:offset,byteLength:a.byteLength,target:i===4?34963:34962};offset+=a.byteLength;return view;});
const files={'neon-floor-color.png':PNG.sync.write(color),'neon-floor-normal.png':PNG.sync.write(normal)};
for(const variant of ['texture','normal']){
    const lit=variant==='normal';
    const material={name:`Dark authored tiles (${variant})`,pbrMetallicRoughness:{baseColorFactor:lit?[.08,.08,.08,1]:[0,0,0,1],metallicFactor:0,roughnessFactor:.95,...(lit?{baseColorTexture:{index:0}}:{})},emissiveFactor:lit?[.992,.992,.992]:[1,1,1],emissiveTexture:{index:0},...(lit?{normalTexture:{index:1,scale:.55}}:{})};
    const gltf={asset:{version:'2.0',generator:'scripts/generate-neon-floor.mjs'},scene:0,scenes:[{nodes:[0]}],nodes:[{name:'Neon floor',mesh:0}],meshes:[{primitives:[{attributes:{POSITION:0,NORMAL:1,TANGENT:2,TEXCOORD_0:3},indices:4,material:0}]}],materials:[material],buffers:[{byteLength:geometry.length,uri:`data:application/octet-stream;base64,${geometry.toString('base64')}`}],bufferViews:views,accessors:[{bufferView:0,componentType:5126,count:4,type:'VEC3',min:[-3,0,-5.25],max:[3,0,2.25]},{bufferView:1,componentType:5126,count:4,type:'VEC3'},{bufferView:2,componentType:5126,count:4,type:'VEC4'},{bufferView:3,componentType:5126,count:4,type:'VEC2'},{bufferView:4,componentType:5123,count:6,type:'SCALAR'}],images:[{uri:'neon-floor-color.png'},...(lit?[{uri:'neon-floor-normal.png'}]:[])],textures:[{source:0,sampler:0},...(lit?[{source:1,sampler:0}]:[])],samplers:[{magFilter:9729,minFilter:9987,wrapS:10497,wrapT:10497}]};
    files[`neon-floor-${variant}.gltf`]=Buffer.from(JSON.stringify(gltf,null,2)+'\n');
}
const metadata={generator:'node scripts/generate-neon-floor.mjs',tileSpacing:tile,bounds:{min:[-3,0,-5.25],max:[3,0,2.25]},grid:[8,10],vertices:4,triangles:2,primitives:1,textureSize:[size,size],textureRepeatMeters:repeat,extensions:[],reflectivity:.94,normalScale:.55,baseline:{tileVertices:640,tileTriangles:960,groutVertices:8,groutTriangles:12},files:Object.fromEntries(Object.entries(files).map(([name,data])=>[name,{bytes:data.length,sha256:createHash('sha256').update(data).digest('hex')}]))};
files['neon-floor-metadata.json']=Buffer.from(JSON.stringify(metadata,null,2)+'\n');
for(const target of ['examples/neon-arena/assets/','native/neon/export-source/examples/neon-arena/assets/']){
    const dir=new URL(target,root);await mkdir(dir,{recursive:true});
    for(const [name,data] of Object.entries(files))await writeFile(new URL(name,dir),data);
}
console.log(`Generated ${Object.keys(files).length} identical floor assets per scene: 4 vertices, 2 triangles, ${size}x${size} textures.`);
