/** Offline conversion only; Three.js is never a dependency of the Rust example. */
import { createServer } from 'node:http';
import { readFile, writeFile, copyFile, mkdir } from 'node:fs/promises';
import { resolve, dirname, extname, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import puppeteer from 'puppeteer';

const source = resolve(process.argv[2] || '/Users/Honza/Documents/Codex/2026-10-22/ude/outputs/anticke-mesto/dist');
const output = dirname(fileURLToPath(import.meta.url));
const inputs = ['main.js', '../README.md', 'city.js', 'collision-world.js', 'characters.js', 'vendor/BufferGeometryUtils.js', 'vendor/GLTFExporter.js', 'vendor/GLTFLoader.js', 'vendor/three.module.js', 'vendor/LICENSE-three.txt', ...['blade-wraith','core-tank','drone-orbiter','madmechanic'].map(slug => `models/${slug}.glb`)];
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
async function snapshot() { return Object.fromEntries(await Promise.all(inputs.map(async path => [path, sha256(await readFile(resolve(source, path)))]))); }
const before = await snapshot();
const html = '<!doctype html><html><head><script type="importmap">{"imports":{"three":"/vendor/three.module.js"}}</script></head><body></body></html>';
const server = createServer(async (req, res) => {
  try {
    const path = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    if (path === '/') { res.setHeader('Content-Type', 'text/html'); return res.end(html); }
    const file = resolve(source, '.' + path);
    if (relative(source, file).startsWith('..' + sep)) { res.writeHead(403); return res.end(); }
    res.setHeader('Content-Type', extname(file) === '.js' ? 'text/javascript' : 'application/octet-stream');
    res.end(await readFile(file));
  } catch { res.writeHead(404); res.end(); }
});
await new Promise(ok => server.listen(0, '127.0.0.1', ok));
let browser;
try {
  browser = await puppeteer.launch({headless: true});
  const page = await browser.newPage();
  await page.setViewport({width:1280,height:800,deviceScaleFactor:1});
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  const result = await page.evaluate(async () => {
    const THREE = await import('/vendor/three.module.js');
    const {createCity} = await import('/city.js');
    const {createCharacters} = await import('/characters.js');
    const {GLTFExporter} = await import('/vendor/GLTFExporter.js');
    const city = createCity(THREE);
    const staticCity = city.group.clone(true);
    const cameras = [
      {name:'gate',position:[0,1.7,42],target:[0,1.7,-20]},
      {name:'forum',position:[0,1.7,8],target:[0,3,-20]},
      {name:'temple',position:[0,2.4,-8],target:[0,7,-36]},
      {name:'market',position:[0,1.7,30],target:[8,2,15]},
      {name:'characters',position:[0,1.7,8],target:[0,1.6,-12]},
      {name:'aerial',position:[65,85,85],target:[0,0,-12]},
    ];
    for(const item of cameras) {
      const camera=new THREE.PerspectiveCamera(65,1,.08,350);
      camera.name=item.name; camera.position.fromArray(item.position); camera.lookAt(...item.target);
      item.quaternion=camera.quaternion.toArray(); item.fovDegrees=65; item.near=.08; item.far=350;
      staticCity.add(camera);
    }
    const characters = createCharacters(city, {shadowMap: {needsUpdate: false}});
    await characters.ready;
    if (characters.state.failed.length) throw Error(JSON.stringify(characters.state.failed));
    const transform = object => ({position: object.position.toArray(), quaternion: object.quaternion.toArray(), scale: object.scale.toArray()});
    const bounds = object => { const b = new THREE.Box3().setFromObject(object); return {min: b.min.toArray(), max: b.max.toArray()}; };
    const stats = object => {
      let meshes = 0, triangles = 0; const materials = new Set();
      object.traverse(o => { if (!o.isMesh) return; meshes++; triangles += (o.geometry.index?.count ?? o.geometry.attributes.position.count)/3; for (const m of [].concat(o.material)) materials.add(m.uuid); });
      return {meshes, triangles, materials: materials.size, bounds: bounds(object)};
    };
    const renderer=new THREE.WebGLRenderer({antialias:true,powerPreference:'high-performance'});
    renderer.setPixelRatio(1);renderer.setSize(1280,800);renderer.outputColorSpace=THREE.SRGBColorSpace;
    renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1.25;
    renderer.shadowMap.enabled=true;renderer.shadowMap.type=THREE.PCFSoftShadowMap;renderer.shadowMap.autoUpdate=false;renderer.shadowMap.needsUpdate=true;
    const scene=new THREE.Scene();scene.background=new THREE.Color('#b8d0ca');scene.fog=new THREE.Fog('#b8d0ca',90,245);
    scene.add(new THREE.HemisphereLight('#e5f4ff','#b79868',2.5));
    const sun=new THREE.DirectionalLight('#ffe0ab',3.1);sun.position.set(-45,75,35);sun.castShadow=true;sun.shadow.mapSize.set(2048,2048);
    Object.assign(sun.shadow.camera,{left:-95,right:95,top:95,bottom:-95,near:1,far:220});sun.shadow.normalBias=.12;sun.shadow.bias=-.00015;sun.target.position.set(0,0,-15);scene.add(sun.target,sun);
    staticCity.traverse(o=>{if(o.isMesh){o.castShadow=true;o.receiveShadow=true;}});
    // Render the original live tree, matching source main.js city shadow flags.
    city.group.children.filter(o=>o.isMesh).forEach(o=>{o.castShadow=true;o.receiveShadow=true;});scene.add(city.group);
    const camera=new THREE.PerspectiveCamera(65,1280/800,.08,350);camera.position.set(0,1.7,42);
    document.body.style.margin='0';document.body.append(renderer.domElement);renderer.render(scene,camera);
    const sourceRenderSettings={referenceFile:'source-gate.png',referenceViewport:[1280,800],referencePose:'initial unadvanced character pose',background:'#b8d0ca',fog:{color:'#b8d0ca',near:90,far:245},hemisphere:{sky:'#e5f4ff',ground:'#b79868',intensity:2.5},sun:{color:'#ffe0ab',intensity:3.1,position:[-45,75,35],target:[0,0,-15]},toneMapping:'ACESFilmic',exposure:1.25,outputColorSpace:'sRGB',antialias:true,shadows:'PCFSoft',shadowMapSize:2048};
    const models = characters.state.entries.map(e => ({slug:e.slug, name:e.name, file:`${e.slug}.glb`, height:e.height, holder:transform(e.holder), root:transform(e.root), clips:e.clips, bounds:bounds(e.root)}));
    const decorations = new THREE.Group(); decorations.name='Original character plaques and ground shadows';
    for(const e of characters.state.entries) { const holder=e.holder.clone(true); holder.remove(holder.children[0]); decorations.add(holder); }
    const exporter = new GLTFExporter();
    const binary = await exporter.parseAsync(staticCity, {binary:true, onlyVisible:true});
    const base64 = binary => { const bytes = new Uint8Array(binary); let s = ''; for(let i=0;i<bytes.length;i+=32768)s+=String.fromCharCode(...bytes.subarray(i,i+32768)); return btoa(s); };
    const decorationBinary=await exporter.parseAsync(decorations,{binary:true,onlyVisible:true});
    return {cityBase64:base64(binary), decorationsBase64:base64(decorationBinary), layout:{schemaVersion:1, units:'metres', upAxis:'Y', sourceScene:'Aurelia — antické město ve VR', cityFile:'rim-city.glb', decorationsFile:'rim-decorations.glb', sourceRenderSettings, cameras, city:stats(staticCity), sourceStats:city.stats, landmarks:[...city.landmarks,{name:'Naše postavy',x:0,z:8}], camera:{position:[0,1.7,42], forward:[0,0,-1], fovDegrees:65, near:.08, far:350}, models}};
  });
  await page.screenshot({path:resolve(output,'source-gate.png')});
  const after = await snapshot();
  if (JSON.stringify(before) !== JSON.stringify(after)) throw Error('Source inputs changed during export; outputs rejected');
  let bytes = Buffer.from(result.cityBase64, 'base64');
  // GLTFExporter writes its authoring aspect ratio; omit it for viewport-native projection.
  const originalJsonLength=bytes.readUInt32LE(12);
  const originalJson=JSON.parse(bytes.subarray(20,20+originalJsonLength).toString());
  for(const camera of originalJson.cameras ?? []) delete camera.perspective.aspectRatio;
  const compactJson=Buffer.from(JSON.stringify(originalJson));
  const paddedJson=Buffer.alloc(Math.ceil(compactJson.length/4)*4,0x20);compactJson.copy(paddedJson);
  const remainder=bytes.subarray(20+originalJsonLength);
  const header=Buffer.alloc(20);bytes.copy(header,0,0,20);header.writeUInt32LE(20+paddedJson.length+remainder.length,8);header.writeUInt32LE(paddedJson.length,12);
  bytes=Buffer.concat([header,paddedJson,remainder]);
  const jsonLength = bytes.readUInt32LE(12);
  const gltf = JSON.parse(bytes.subarray(20,20+jsonLength).toString());
  if (gltf.buffers.some(b => b.uri) || gltf.images?.some(i => i.uri && !i.uri.startsWith('data:'))) throw Error('External GLB dependency');
  if (result.layout.city.triangles !== 62076 || result.layout.city.meshes !== 19) throw Error('Unexpected city geometry');
  await mkdir(output, {recursive:true});
  await writeFile(resolve(output,'rim-city.glb'),bytes);
  const decorationBytes=Buffer.from(result.decorationsBase64,'base64');
  const decorationJson=JSON.parse(decorationBytes.subarray(20,20+decorationBytes.readUInt32LE(12)).toString());
  if(decorationJson.buffers.some(b=>b.uri)||decorationJson.images.some(i=>i.uri))throw Error('External decorations dependency');
  await writeFile(resolve(output,'rim-decorations.glb'),decorationBytes);
  result.layout.cameras.forEach(camera=>{camera.nodeIndex=gltf.nodes.findIndex(node=>node.name===camera.name && node.camera!==undefined);});
  for (const model of result.layout.models) await copyFile(resolve(source, 'models', model.file),resolve(output,model.file));
  await copyFile(resolve(source,'vendor/LICENSE-three.txt'),resolve(output,'LICENSE-three.txt'));
  await writeFile(resolve(output,'layout.json'),JSON.stringify(result.layout,null,2)+'\n');
  await writeFile(resolve(output,'source-manifest.json'),JSON.stringify({sourceDirectory:source, algorithm:'SHA-256', inputsBefore:before, inputsAfter:after, sourceUnchanged:true, cityGlbSha256:sha256(bytes), cityGlbBytes:bytes.length, glb:{meshes:gltf.meshes.length, materials:gltf.materials.length, buffers:gltf.buffers.length, embedded:true}},null,2)+'\n');
  console.log(JSON.stringify({city:result.layout.city, glbBytes:bytes.length, models:result.layout.models.map(({slug,clips})=>({slug,clips})), sourceUnchanged:true}));
} finally { await browser?.close(); await new Promise(ok=>server.close(ok)); }
