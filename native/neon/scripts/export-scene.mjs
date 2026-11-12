// Offline authoring only. The Android package contains JSON/assets, never this JS.
import {readFile,writeFile,mkdir,copyFile} from 'node:fs/promises';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const root=fileURLToPath(new URL('../',import.meta.url));
const source=path.join(root,'export-source');
const output=path.join(root,'assets');
globalThis.location={search:'',href:pathToFileURL(path.join(source,'examples/neon-arena/index.html')).href};
const author=await import(pathToFileURL(path.join(source,'examples/neon-arena/scene.js')));
const {planCue,validateRoundPlan}=await import(pathToFileURL(path.join(source,'examples/neon-arena/round-plan.js')));
const {planWave,validateWave}=await import(pathToFileURL(path.join(source,'examples/neon-arena/wave-plan.js')));
const ops=[]; let next=1;
const referenced=new Set();
const assetPath=url=>{
 const file=fileURLToPath(url),relative=path.relative(source,file).split(path.sep).join('/');
 if(relative.startsWith('../'))throw new Error(`Asset outside authoring snapshot: ${relative}`);
 referenced.add(relative); return relative;
};
class Handle {
 constructor(kind){this.kind=kind;this.key=next++;this.id={['$'+kind]:this.key};this.scale=[1,1,1];}
 toJSON(){return this.id;}
 get exists(){return true;}
 get meshInfo(){return {scale:this.scale};}
}
for(const method of ['setPosition','setRotation','setScale','setEmission','setColorCutout','setOpacity','setRenderOptions','spin','setCutter','setBladePhysics','setCutAssist','setBladeContactResponse','setBladeTrail','setFont3D','bindComboIndicator','remove']){
 Handle.prototype[method]=function(...args){if(method==='setScale')this.scale=args[0];ops.push({op:method,id:this.id,args});return this;};
}
const create=(kind,op,args)=>{const handle=new Handle(kind);ops.push({op,id:handle.id,args});return handle;};
const scene={
 clear(){ops.push({op:'clear',args:[]});},
 createMesh(mesh){return create('object','createMesh',[mesh]);},
 createMeshVariants(mesh){const handle=create('asset','createMeshVariants',[mesh]);handle.createInstance=(variant=0)=>create('object','createVariantInstance',[handle.id,variant]);return handle;},
 createTextPanel(options){return create('object','createTextPanel',[options]);},
 createInfoPanel(kind,options){return create('object','createInfoPanel',[kind,options]);},
 createLight(options){return create('object','createLight',[options]);},
 async loadFont3D(url){return create('font','loadFont3D',[assetPath(url)]);},
 async loadModel(url){return create('object','loadModel',[assetPath(url)]);},
};
const world=await author.createScene(scene);
await world.loadDecorations();
const json=value=>JSON.parse(JSON.stringify(value));
const refs={};
for(const [key,value] of Object.entries(world)){
 if(typeof value==='function'||['fruits','playFruits','fruitPatterns','galleryPatterns','geometry','wild','arenaFont'].includes(key))continue;
 refs[key]=value;
}
const variants=ops.filter(o=>o.op==='createMeshVariants').slice(0,10);
const catalog=Object.entries(world.fruitPatterns).map(([name,patterns],index)=>{
 assert.equal(patterns.length,15,`${name} pattern count`);
 return {name,...author.FRUIT_POINTS[name],capColor:patterns[0].capColor,asset:variants[index].id,
 variants:patterns.map((fruit,variant)=>({variant,coverage:fruit.coverage,capColor:fruit.capColor}))};
});
const parse=async file=>JSON.parse(await readFile(path.join(source,'examples/neon-arena',file),'utf8'));
const preset=validateWave((await parse('wave-preset.json')).wave);
const minutePlan=validateRoundPlan(await parse('minute-scene.json'));
const sourceHashes={};
for(const file of ['scene.js','geometry.js','wave-plan.js','round-plan.js','minute-scene.json','wave-preset.json']){
 const name=`examples/neon-arena/${file}`;sourceHashes[name]=createHash('sha256').update(await readFile(path.join(source,name))).digest('hex');
}
for(const name of ['examples/shared/vr-debug-panel.js','assets/fonts/cynatar/glyphs.json'])sourceHashes[name]=createHash('sha256').update(await readFile(path.join(source,name))).digest('hex');
const manifest={version:1,revision:author.ARENA_REVISION,sourceHashes,operations:ops,refs,
 runtime:{fruitScale:author.FRUIT_SCALE,opacity:author.FRUIT_OPACITY,opacityVariation:author.FRUIT_OPACITY_VARIATION,
 cutEffectSize:author.CUT_EFFECT_SIZE,cutPieceFade:author.CUT_PIECE_FADE,glow:author.NEON_GLOW,
 restartPosition:author.RESTART_POSITION,comboOrigin:author.COMBO_DISPLAY_ORIGIN,preset,catalog,
 minutePlan:{...minutePlan,cues:minutePlan.cues.map((cue,i)=>({...cue,recipes:planCue(cue,preset,i)}))},
 waves:Array.from({length:9},(_,i)=>planWave(preset,i)),decorationsReady:world.decorationsReady}};
// glTF resources remain beside their documents. data URIs need no extra file.
for(const file of [...referenced])if(file.endsWith('.gltf')){
 const gltf=JSON.parse(await readFile(path.join(source,file),'utf8'));
 for(const item of [...(gltf.buffers??[]),...(gltf.images??[])])if(item.uri&&!item.uri.startsWith('data:'))referenced.add(path.posix.join(path.posix.dirname(file),item.uri));
}
for(const file of referenced){
 await mkdir(path.dirname(path.join(output,file)),{recursive:true});
 if(!file.endsWith('.gltf')){await copyFile(path.join(source,file),path.join(output,file));continue;}
 const gltf=JSON.parse(await readFile(path.join(source,file),'utf8'));let index=0;
 for(const item of [...(gltf.buffers??[]),...(gltf.images??[])])if(item.uri?.startsWith('data:')){
  const match=/^data:([^;,]*);base64,(.*)$/s.exec(item.uri);assert.ok(match,'Only base64 glTF embedded resources are supported');
  const name=`${path.basename(file,'.gltf')}-embedded-${index++}.${match[1]==='image/png'?'png':match[1]==='image/jpeg'?'jpg':'bin'}`;
  await writeFile(path.join(output,path.dirname(file),name),Buffer.from(match[2],'base64'));item.uri=name;
 }
 await writeFile(path.join(output,file),JSON.stringify(gltf)+'\n');
}
const serialized=JSON.stringify(json(manifest));
await mkdir(output,{recursive:true});await writeFile(path.join(output,'scene.json'),serialized+'\n');
assert.equal(catalog.length,10);assert.equal(refs.scoreboardPanels.length,2);assert.equal(world.decorationsReady,true);
console.log(JSON.stringify({revision:manifest.revision,operations:ops.length,catalog:catalog.length,variants:catalog.reduce((n,c)=>n+c.variants.length,0),cues:minutePlan.cues.length,assetFiles:referenced.size,jsonBytes:serialized.length}));
