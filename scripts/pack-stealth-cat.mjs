// Technical atlas packing: preserve generated artwork, normalize cell baselines.
import {readFileSync,writeFileSync} from 'node:fs';
import {PNG} from 'pngjs';
const root=new URL('../',import.meta.url);
const source=PNG.sync.read(readFileSync(new URL('examples/neon-arena/assets/stealth-cat-source.png',root)));
const frames=[];
for(let frame=0;frame<8;frame++){
 const x0=Math.floor(frame%4*source.width/4),x1=Math.floor((frame%4+1)*source.width/4);
 const y0=Math.floor(Math.floor(frame/4)*source.height/2),y1=Math.floor((Math.floor(frame/4)+1)*source.height/2);
 let minX=x1,minY=y1,maxX=x0,maxY=y0;
 for(let y=y0;y<y1;y++)for(let x=x0;x<x1;x++)if(source.data[(y*source.width+x)*4+3]>16){minX=Math.min(minX,x);minY=Math.min(minY,y);maxX=Math.max(maxX,x);maxY=Math.max(maxY,y);}
 if(maxX<=minX||maxY<=minY)throw Error(`Empty frame ${frame}`);
 frames.push({minX,minY,maxX,maxY});
}
const scale=Math.min(236/Math.max(...frames.map(f=>f.maxX-f.minX+1)),210/Math.max(...frames.map(f=>f.maxY-f.minY+1)));
const atlas=new PNG({width:1024,height:512});
for(const [i,f] of frames.entries()){
 const width=Math.round((f.maxX-f.minX+1)*scale),height=Math.round((f.maxY-f.minY+1)*scale);
 const ox=i%4*256+Math.floor((256-width)/2),oy=Math.floor(i/4)*256+236-height;
 for(let y=0;y<height;y++)for(let x=0;x<width;x++){
  const sx=Math.min(f.maxX,f.minX+Math.floor(x/scale)),sy=Math.min(f.maxY,f.minY+Math.floor(y/scale));
  source.data.copy(atlas.data,((oy+y)*1024+ox+x)*4,(sy*source.width+sx)*4,(sy*source.width+sx)*4+4);
 }
}
const positions=new Float32Array([-.4,0,0,.4,0,0,.4,.8,0,-.4,.8,0]);
const normals=new Float32Array([0,0,1,0,0,1,0,0,1,0,0,1]);
const uv=new Float32Array([0,1,1,1,1,0,0,0]);const indices=new Uint16Array([0,1,2,0,2,3]);
const arrays=[positions,normals,uv,indices];let offset=0;
const bufferViews=arrays.map(a=>{const v={buffer:0,byteOffset:offset,byteLength:a.byteLength};offset+=a.byteLength;return v;});
const buffer=Buffer.concat(arrays.map(a=>Buffer.from(a.buffer)));
const gltf={asset:{version:'2.0',generator:'pack-stealth-cat.mjs'},extensionsUsed:['KHR_materials_unlit'],scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives:[{attributes:{POSITION:0,NORMAL:1,TEXCOORD_0:2},indices:3,material:0}]}],materials:[{name:'Stealth cat sprite',extensions:{KHR_materials_unlit:{}},alphaMode:'MASK',alphaCutoff:.35,doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[1,1,1,1],baseColorTexture:{index:0},metallicFactor:0,roughnessFactor:1}}],textures:[{source:0,sampler:0}],images:[{uri:'stealth-cat.png'}],samplers:[{magFilter:9729,minFilter:9729,wrapS:33071,wrapT:33071}],buffers:[{byteLength:buffer.length,uri:'data:application/octet-stream;base64,'+buffer.toString('base64')}],bufferViews,accessors:[{bufferView:0,componentType:5126,count:4,type:'VEC3',min:[-.4,0,0],max:[.4,.8,0]},{bufferView:1,componentType:5126,count:4,type:'VEC3'},{bufferView:2,componentType:5126,count:4,type:'VEC2'},{bufferView:3,componentType:5123,count:6,type:'SCALAR'}]};
for(const dir of ['examples/neon-arena/assets/','native/neon/export-source/examples/neon-arena/assets/']){
 writeFileSync(new URL(dir+'stealth-cat.png',root),PNG.sync.write(atlas));
 writeFileSync(new URL(dir+'stealth-cat.gltf',root),JSON.stringify(gltf));
}
console.log('Packed 8 nonempty frames, shared baseline, 1024x512 RGBA atlas; two-triangle glTF card.');
