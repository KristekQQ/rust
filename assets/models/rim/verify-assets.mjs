import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const base=new URL('./',import.meta.url);
const read=async file=>readFile(new URL(file,base));
const layout=JSON.parse(await read('layout.json'));
const manifest=JSON.parse(await read('source-manifest.json'));
const summaries=[];
for(const file of [layout.cityFile,layout.decorationsFile,...layout.models.map(m=>m.file)]) {
  const bytes=await read(file);assert.equal(bytes.readUInt32LE(0),0x46546c67);assert.equal(bytes.readUInt32LE(4),2);assert.equal(bytes.readUInt32LE(8),bytes.length);
  const json=JSON.parse(bytes.subarray(20,20+bytes.readUInt32LE(12)));
  assert(json.buffers.every(buffer=>!buffer.uri));assert((json.images??[]).every(image=>!image.uri));
  let triangles=0;for(const mesh of json.meshes??[])for(const primitive of mesh.primitives){assert.equal(primitive.mode??4,4);triangles+=(json.accessors[primitive.indices??primitive.attributes.POSITION].count)/3;}
  const summary={file,bytes:bytes.length,meshes:json.meshes?.length??0,materials:json.materials?.length??0,triangles,animations:(json.animations??[]).map(a=>a.name),embedded:true};
  if(file===layout.cityFile) {
    assert.equal(triangles,62076);assert.equal(json.meshes.length,19);assert.equal(json.materials.length,19);assert.equal(json.cameras.length,6);
    for(const camera of layout.cameras){assert.equal(json.nodes[camera.nodeIndex].name,camera.name);const perspective=json.cameras[json.nodes[camera.nodeIndex].camera].perspective;assert.equal(perspective.aspectRatio,undefined);assert(Math.abs(perspective.yfov-65*Math.PI/180)<1e-12);}
    assert.equal(createHash('sha256').update(bytes).digest('hex'),manifest.cityGlbSha256);
  }
  const model=layout.models.find(m=>m.file===file);
  if(model){assert.deepEqual(summary.animations,model.clips);assert.equal(createHash('sha256').update(bytes).digest('hex'),manifest.inputsBefore[`models/${file}`]);}
  summaries.push(summary);
}
assert.deepEqual(manifest.inputsBefore,manifest.inputsAfter);
console.log(JSON.stringify({passed:true,sourceUnchanged:manifest.sourceUnchanged,assets:summaries},null,2));
