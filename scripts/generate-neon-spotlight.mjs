#!/usr/bin/env node
// Deterministic baked illumination; no runtime light or shadow pass required.
import {mkdir, writeFile} from 'node:fs/promises';
import {PNG} from 'pngjs';
import {profile,lightAt} from './lib/neon-light-profile.mjs';
import {createDebugPillar} from '../examples/neon-arena/debug-pillar.js';

const root = new URL('../', import.meta.url);
const bounds = {min: [-3, 0, -5.25], max: [3, 0, 2.25]};
const smooth = x => {const t = Math.max(0, Math.min(1, x)); return t * t * (3 - 2 * t);};
const srgb = x => Math.round(255 * (x <= .0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - .055));
const worldAt = (x, y, width, height) => [-3 + (x + .5) * 6 / width, -5.25 + (y + .5) * 7.5 / height];
const size = 1024, tile = .75;
const color = new PNG({width: size, height: size});
for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const [wx, wz] = worldAt(x, y, size, size);
    // Preserve the original tile color, bevel and grain formula in world space.
    const u = (wx + 3) / tile, v = (wz + 5.25) / tile;
    const fx = u % 1, fy = v % 1, edge = Math.min(fx, 1 - fx, fy, 1 - fy) * tile;
    const checker = (Math.floor(u) + Math.floor(v) + 1) % 2;
    const top = checker ? [.032, .047, .070] : [.052, .073, .10], grout = [.006, .012, .020];
    const bevel = smooth((edge - .004) / .009);
    const grain = 1 + .014 * Math.sin(u * 31.4159265359) * Math.sin(v * 25.1327412287);
    const light = lightAt(wx, wz), i = (y * size + x) * 4;
    for (let c = 0; c < 3; c++) color.data[i + c] = srgb((grout[c] * (1 - bevel) + top[c] * grain * bevel) * light);
    color.data[i + 3] = 255;
}
const mask = Buffer.alloc(profile.mask.width * profile.mask.height);
for (let y = 0; y < profile.mask.height; y++) for (let x = 0; x < profile.mask.width; x++) {
    mask[y * profile.mask.width + x] = Math.round(255 * lightAt(...worldAt(x, y, profile.mask.width, profile.mask.height)));
}

const chunks = [], bufferViews = [], accessors = [];
let byteLength = 0;
function accessor(array, type, count, target, extra = {}) {
    const padding = (4 - byteLength % 4) % 4;
    if (padding) {chunks.push(Buffer.alloc(padding)); byteLength += padding;}
    const bufferView = bufferViews.length;
    bufferViews.push({buffer: 0, byteOffset: byteLength, byteLength: array.byteLength, target});
    chunks.push(Buffer.from(array.buffer, array.byteOffset, array.byteLength)); byteLength += array.byteLength;
    accessors.push({bufferView, componentType: array instanceof Uint16Array ? 5123 : 5126, count, type, ...extra});
    return accessors.length - 1;
}
function primitive(positions, normals, uvs, indices, material) {
    const min = [0, 1, 2].map(c => Math.min(...positions.filter((_, i) => i % 3 === c)));
    const max = [0, 1, 2].map(c => Math.max(...positions.filter((_, i) => i % 3 === c)));
    return {attributes: {
        POSITION: accessor(new Float32Array(positions), 'VEC3', positions.length / 3, 34962, {min, max}),
        NORMAL: accessor(new Float32Array(normals), 'VEC3', normals.length / 3, 34962),
        TEXCOORD_0: accessor(new Float32Array(uvs), 'VEC2', uvs.length / 2, 34962),
    }, indices: accessor(new Uint16Array(indices), 'SCALAR', indices.length, 34963), material};
}
const floor = primitive([-3, 0, -5.25, 3, 0, -5.25, 3, 0, 2.25, -3, 0, 2.25],
    [0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0], [0, 0, 1, 0, 1, 1, 0, 1], [0, 2, 1, 0, 3, 2], 0);
// Two forward-side fixtures aim inward toward the player. Geometry is baked once;
// matching materials stay merged, with no runtime lights or volumetric passes.
const normalize = v => {const n = Math.hypot(...v); return v.map(x => x / n);};
function triangles() {
    const positions = [], normals = [], uvs = [], indices = [];
    return {
        add(a, b, c) {
            const ab = b.map((v, i) => v - a[i]), ac = c.map((v, i) => v - a[i]);
            const n = normalize([ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0]]);
            for (const vertex of [a, b, c]) {indices.push(positions.length / 3); positions.push(...vertex); normals.push(...n); uvs.push(0, 0);}
        },
        primitive: material => primitive(positions, normals, uvs, indices, material),
    };
}
const housing = triangles(), emitter = triangles(), segments = 16;
// Shared feathered mask: bright at the source, softly fading along each shaft.
const shaftTexture = new PNG({width: 64, height: 128});
for (let y = 0; y < 128; y++) for (let x = 0; x < 64; x++) {
    const u = (x + .5) / 64, v = (y + .5) / 128;
    const edge = Math.pow(Math.max(0, 1 - Math.abs(u * 2 - 1)), 2.4);
    const axial = smooth(v / .025) * Math.exp(-v * 1.9) * (1 - smooth((v - .65) / .35));
    const wisps = .78 + .22 * Math.pow(Math.cos(u * 26), 2), i = (y * 64 + x) * 4;
    shaftTexture.data[i] = 255; shaftTexture.data[i + 1] = 255; shaftTexture.data[i + 2] = 255;
    shaftTexture.data[i + 3] = Math.round(255 * edge * axial * wisps);
}
const shaftPositions = [], shaftNormals = [], shaftUvs = [], shaftIndices = [];
const glarePositions = [], glareNormals = [], glareUvs = [], glareIndices = [];
for (const light of profile.lights) {
    const direction = normalize(light.aimTarget.map((v, i) => v - light.position[i]));
    const up = direction.map(v => -v);
    const right = normalize([up[2], 0, -up[0]]);
    const back = [right[1] * up[2] - right[2] * up[1], right[2] * up[0] - right[0] * up[2], right[0] * up[1] - right[1] * up[0]];
    const lampPoint = ([x, y, z]) => light.position.map((v, i) => v + x * right[i] + (y - 3.2) * up[i] + (z + .75) * back[i]);
    const addHousing = (a, b, c) => housing.add(lampPoint(a), lampPoint(b), lampPoint(c));
    const ring = (radius, y, angle) => [radius * Math.cos(angle), y, -.75 + radius * Math.sin(angle)];
    for (let i = 0; i < segments; i++) {
        const a = i * 2 * Math.PI / segments, b = (i + 1) * 2 * Math.PI / segments;
        const bottomA = ring(.20, 3.12, a), bottomB = ring(.20, 3.12, b);
        const topA = ring(.12, 3.28, a), topB = ring(.12, 3.28, b);
        addHousing(bottomA, topA, topB); addHousing(bottomA, topB, bottomB);
        addHousing([0, 3.28, -.75], topB, topA);
        const innerA = ring(.075, 3.12, a), innerB = ring(.075, 3.12, b);
        addHousing(innerA, bottomA, bottomB); addHousing(innerA, bottomB, innerB);
        emitter.add(...[[0, 3.115, -.75], ring(.075, 3.115, a), ring(.075, 3.115, b)].map(lampPoint));
    }
    // Three bounded cards per fixture; both axes converge on the actor.
    const start = light.position.map((v, i) => v + direction[i] * .09);
    const shaftLength = Math.hypot(...light.aimTarget.map((v,i)=>v-light.position[i]))*.92;
    const end = light.position.map((v, i) => v + direction[i] * shaftLength);
    for (let card = 0; card < 3; card++) {
        const angle = card * Math.PI / 3, axis = right.map((v, i) => v * Math.cos(angle) + back[i] * Math.sin(angle));
        const vertices = [start.map((v, i) => v - axis[i] * .085), start.map((v, i) => v + axis[i] * .085),
            end.map((v, i) => v + axis[i] * .42), end.map((v, i) => v - axis[i] * .42)];
        const b = shaftPositions.length / 3; vertices.forEach(v => shaftPositions.push(...v));
        for (let i = 0; i < 4; i++) shaftNormals.push(...back);
        shaftUvs.push(0, 0, 1, 0, 1, 1, 0, 1); shaftIndices.push(b, b + 1, b + 2, b, b + 2, b + 3);
    }
    // Each source has its own bounded, fixed-axis glare, never a camera billboard.
    const center = light.position.map((v, i) => v + direction[i] * .095), b = glarePositions.length / 3;
    for (const [x, y] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
        glarePositions.push(...center.map((v, i) => v + right[i] * x * .28 + back[i] * y * .28));
        glareNormals.push(...direction);
    }
    glareUvs.push(0, 0, 1, 0, 1, 1, 0, 1); glareIndices.push(b, b + 1, b + 2, b, b + 2, b + 3);
}
const shaft = primitive(shaftPositions, shaftNormals, shaftUvs, shaftIndices, 3);
const glare = primitive(glarePositions, glareNormals, glareUvs, glareIndices, 4);
// Bake the column once; preserve primitive bounds for independent culling.
let stoneMesh, stonePosition;
const authoringHandle={setPosition(p){stonePosition=p;return this;},setEmission(){return this;},setRenderOptions(){return this;}};
createDebugPillar({createMesh(mesh){stoneMesh=mesh;return authoringHandle;}});
const stonePositions=[],stoneColors=[],stoneNormals=[],stoneUvs=[],stoneIndices=[];
for(let i=0;i<stoneMesh.indices.length;i++){
 const v=stoneMesh.indices[i],face=Math.floor(i/3);
 for(let c=0;c<3;c++){stonePositions.push(stoneMesh.positions[v*3+c]+stonePosition[c]);stoneColors.push(stoneMesh.triangleColors[face*3+c]);}
 stoneNormals.push(0,1,0);stoneUvs.push(0,0);stoneIndices.push(i);
}
const stone=primitive(stonePositions,stoneNormals,stoneUvs,stoneIndices,5);
stone.attributes.COLOR_0=accessor(new Float32Array(stoneColors),'VEC3',stoneColors.length/3,34962);
// Arena presentation deliberately excludes fixtures, emitter disks and source glare.
const primitives = [floor, shaft];
const geometry = Buffer.concat(chunks);
const gltf = {
    asset: {version: '2.0', generator: 'scripts/generate-neon-spotlight.mjs'},
    extensionsUsed: ['KHR_materials_unlit'],
    scene: 0, scenes: [{nodes: [0]}], nodes: [{name: 'Baked floor and two light shafts without visible fixtures', mesh: 0}], meshes: [{primitives}],
    materials: [
        {name: 'Tiles with baked radial illumination', extensions: {KHR_materials_unlit: {}}, pbrMetallicRoughness: {baseColorFactor: [1, 1, 1, 1], baseColorTexture: {index: 0}, metallicFactor: 0, roughnessFactor: .95}, emissiveFactor: [0, 0, 0]},
        {name: 'Dark metallic lamp housing', pbrMetallicRoughness: {baseColorFactor: [.025, .030, .035, 1], metallicFactor: .75, roughnessFactor: .4}, emissiveFactor: [.004, .005, .006]},
        {name: 'Warm downward emitter', pbrMetallicRoughness: {baseColorFactor: [0, 0, 0, 1], metallicFactor: 0, roughnessFactor: 1}, emissiveFactor: [1, .94, .78]},
        {name:'Baked light shaft',extensions:{KHR_materials_unlit:{}},alphaMode:'BLEND',doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[1,.92,.76,.10],baseColorTexture:{index:1},metallicFactor:0,roughnessFactor:1}},
        {name:'Angle-dependent source glare',extras:{viewGlare:true},extensions:{KHR_materials_unlit:{}},alphaMode:'BLEND',doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[1,.94,.80,.35],metallicFactor:0,roughnessFactor:1}},
        {name:'Baked matte Greek stone',extensions:{KHR_materials_unlit:{}},pbrMetallicRoughness:{baseColorFactor:[1,1,1,1],metallicFactor:0,roughnessFactor:1}},
    ],
    buffers: [{byteLength: geometry.length, uri: `data:application/octet-stream;base64,${geometry.toString('base64')}`}], bufferViews, accessors,
    images: [{uri: 'neon-spotlight-color.png'},{uri:'neon-spotlight-shaft.png'}], textures: [{source: 0, sampler: 0},{source:1,sampler:0}],
    samplers: [{magFilter: 9729, minFilter: 9987, wrapS: 33071, wrapT: 33071}],
};
const files = {
    'neon-stone-pillar.gltf': Buffer.from(JSON.stringify({...gltf,meshes:[{primitives:[{...stone,material:0}]}],materials:[gltf.materials[5]],images:[],textures:[],samplers:[]})+'\n'),
    'neon-source-glare.gltf': Buffer.from(JSON.stringify({...gltf,meshes:[{primitives:[{...glare,material:0}]}],materials:[gltf.materials[4]],images:[],textures:[],samplers:[]})+'\n'),
    'neon-spotlight-color.png': PNG.sync.write(color),
    'neon-spotlight-shaft.png': PNG.sync.write(shaftTexture),
    'neon-spotlight.gltf': Buffer.from(JSON.stringify(gltf, null, 2) + '\n'),
    'neon-spotlight-mask.bin': mask,
    'neon-spotlight-profile.json': Buffer.from(JSON.stringify(profile, null, 2) + '\n'),
};
for (const target of ['examples/neon-arena/assets/', 'native/neon/export-source/examples/neon-arena/assets/']) {
    const dir = new URL(target, root); await mkdir(dir, {recursive: true});
    for (const [name, data] of Object.entries(files)) await writeFile(new URL(name, dir), data);
}
const trianglesCount = primitives.reduce((sum, p) => sum + accessors[p.indices].count / 3, 0);
console.log(`Generated ${Object.keys(files).length} identical assets per directory: ${trianglesCount} triangles, ${primitives.length} primitives, ${size}x${size} color, ${profile.mask.width}x${profile.mask.height} R8 mask.`);
