// Authored, embedded geometry; immutable alpha classes and active scene membership
// are independent inputs. No model download, animation clock or random geometry.
export function createOverlayFixture(blend=true){
 const bytes=new Uint8Array(140);
 new Float32Array(bytes.buffer,0,12).set([-.32,-.3,0,.32,-.3,0,.32,.3,0,-.32,.3,0]);
 new Float32Array(bytes.buffer,48,12).set([0,0,1,0,0,1,0,0,1,0,0,1]);
 new Uint16Array(bytes.buffer,96,6).set([0,1,2,0,2,3]);
 new Float32Array(bytes.buffer,108,8).set([0,0,1,0,1,1,0,1]);
 const mask=document.createElement('canvas');mask.width=2;mask.height=2;
 const ctx=mask.getContext('2d');ctx.fillStyle='white';ctx.fillRect(0,0,1,1);ctx.fillRect(1,1,1,1);
 const modes=blend?['OPAQUE','MASK','BLEND']:['OPAQUE','MASK'];
 const nodes=Array.from({length:6},(_,i)=>({mesh:0,translation:[(i%3-1)*.72,(i<3?-.38:.38),0]}));
 nodes.push({mesh:1,translation:[-.55,0,.15]});
 if(blend)nodes.push({mesh:2,translation:[.55,0,.25]});
 return {asset:{version:'2.0'},buffers:[{byteLength:140,uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...bytes))}],
 bufferViews:[{buffer:0,byteOffset:0,byteLength:48},{buffer:0,byteOffset:48,byteLength:48},{buffer:0,byteOffset:96,byteLength:12},{buffer:0,byteOffset:108,byteLength:32}],
 accessors:[{bufferView:0,componentType:5126,count:4,type:'VEC3',min:[-.32,-.3,0],max:[.32,.3,0]},{bufferView:1,componentType:5126,count:4,type:'VEC3'},{bufferView:2,componentType:5123,count:6,type:'SCALAR'},{bufferView:3,componentType:5126,count:4,type:'VEC2'}],
 images:[{uri:mask.toDataURL()}],textures:[{source:0,sampler:0}],samplers:[{magFilter:9728,minFilter:9728}],
 materials:modes.map((alphaMode,i)=>({doubleSided:true,alphaMode,alphaCutoff:.2,emissiveFactor:i===2?[.1,.5,.08]:[.08,.02,.01],pbrMetallicRoughness:{baseColorFactor:i===2?[.12,1,.1,.6]:[.8,.12,.04,1],roughnessFactor:.8,metallicFactor:0,...(i===1?{baseColorTexture:{index:0}}:{})}})),
 meshes:modes.map((_,material)=>({primitives:[{attributes:{POSITION:0,NORMAL:1,TEXCOORD_0:3},indices:2,material}]})),nodes,
 scenes:[{nodes:nodes.map((_,i)=>i)},{nodes:[0,1,2,3,4,5,6]},...(blend?[{nodes:[7]}]:[])],scene:0};
}
