import * as THREE from '/vendor/three.module.js';
import { insertionOrder } from '/scripts/render-performance-order.mjs';

const query = new URLSearchParams(location.search);
const width=Number(query.get('width')||960),height=Number(query.get('height')||640);if(!Number.isInteger(width)||!Number.isInteger(height)||width<64||height<64||width>4096||height>4096)throw Error('Invalid render dimensions');
const family = query.get('family') || 'batching';
if (family !== 'batching') throw new Error(`Three.js fixture supports batching only, got ${family}`);
const representation = query.get('representation') || 'instanced';
if (!['objects', 'instanced'].includes(representation)) throw new Error(`Invalid representation: ${representation}`);
const load = Number(query.get('load') || 1);
if (!Number.isInteger(load) || load < 1 || load > 65536) throw new Error('Invalid load');
const order = query.get('order') || 'spatial';
const seed = Number(query.get('seed') ?? 1337);
const overlap = Number(query.get('overlap') ?? 0);
if (!Number.isFinite(overlap) || overlap < 0 || overlap > 1) throw new Error('Invalid overlap');
const insertion = insertionOrder(load, order, seed);
const canvas = document.querySelector('#gpu-canvas');
const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, alpha: false });
renderer.setPixelRatio(1);
renderer.setSize(width, height, false);
renderer.setClearColor(0x080b15, 1);
renderer.toneMapping = THREE.NoToneMapping;
renderer.outputColorSpace = THREE.SRGBColorSpace;

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x080b15);
const camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 100);
camera.position.set(0, 2, 9);
camera.lookAt(0, 0, 0);

// This reference uses explicit neutral illumination because the Rust fixture's
// scene lighting is engine-owned. Its PBR response is therefore not pixel matched.
scene.add(new THREE.AmbientLight(0xffffff, 1.5));
const key = new THREE.DirectionalLight(0xffffff, 2.5);
key.position.set(-3, 6, 5);
scene.add(key);

const geometry = new THREE.BufferGeometry();
geometry.setAttribute('position', new THREE.Float32BufferAttribute([
  -0.3,-0.3,0, 0.3,-0.3,0, 0.3,0.3,0, -0.3,0.3,0,
], 3));
geometry.setAttribute('normal', new THREE.Float32BufferAttribute([
  0,0,1, 0,0,1, 0,0,1, 0,0,1,
], 3));
geometry.setIndex([0,1,2,0,2,3]);
const material = new THREE.MeshStandardMaterial({ color: 0xffffff, metalness: 1, roughness: 1 });
const batches = [];
const roots = [];
const operations = { transforms: 0, spawns: 0, removes: 0, cuts: 0 };
const side = Math.ceil(Math.sqrt(load));
const grid = i => [
  ((i % side) - (side - 1) / 2) * 6 / side * (1 - overlap),
  (Math.floor(i / side) - (side - 1) / 2) * 4 / side * (1 - overlap),
  0,
];
const dummy = new THREE.Object3D();
dummy.rotation.order = 'YXZ';
const batchSize = 1024;
if (representation === 'objects') {
  for (let i = 0; i < load; i++) {
    const logicalId = insertion[i];
    const position = grid(logicalId);
    const scale = 0.5 / Math.sqrt(load);
    const root = new THREE.Group();
    root.rotation.order = 'YXZ';
    root.position.set(...position);
    root.scale.setScalar(scale);
    const mesh = new THREE.Mesh(geometry, material);
    mesh.frustumCulled = false;
    root.add(mesh);
    scene.add(root);
    roots.push({ logicalId, root });
  }
} else {
  for (let start = 0; start < load; start += batchSize) {
    const count = Math.min(batchSize, load - start);
    const mesh = new THREE.InstancedMesh(geometry, material, count);
    mesh.count = count;
    mesh.frustumCulled = false;
    for (let local = 0; local < count; local++) {
      const logicalId = insertion[start + local];
      const position = grid(logicalId);
      const scale = 0.5 / Math.sqrt(load);
      dummy.position.set(...position);
      dummy.scale.setScalar(scale);
      dummy.rotation.set(0, 0, 0);
      dummy.updateMatrix();
      mesh.setMatrixAt(local, dummy.matrix);
    }
    mesh.instanceMatrix.needsUpdate = true;
    scene.add(mesh);
    batches.push(mesh);
  }
}

let epoch = performance.now();
let active = false;
function pose(seconds) {
  if (representation === 'objects') {
    for (const { logicalId, root } of roots) {
      root.rotation.set(0.12 * Math.sin(seconds), seconds + logicalId * 0.12, 0);
      operations.transforms++;
    }
  } else {
    for (let batchIndex = 0; batchIndex < batches.length; batchIndex++) {
      const mesh = batches[batchIndex];
      const start = batchIndex * batchSize;
      for (let local = 0; local < mesh.count; local++) {
        const logicalId = insertion[start + local];
        const position = grid(logicalId);
        const scale = 0.5 / Math.sqrt(load);
        dummy.position.set(...position);
        dummy.rotation.set(0.12 * Math.sin(seconds), seconds + logicalId * 0.12, 0);
        dummy.scale.setScalar(scale);
        dummy.updateMatrix();
        mesh.setMatrixAt(local, dummy.matrix);
        operations.transforms++;
      }
      mesh.instanceMatrix.needsUpdate = true;
    }
  }
}
function tick(timestamp) {
  if (active) pose(Math.max(0, (timestamp - epoch) / 1000));
  renderer.render(scene, camera);
  requestAnimationFrame(tick);
}
requestAnimationFrame(tick);

const actualStats = () => {
  const { render, memory } = renderer.info;
  return {
    drawCalls: render.calls,
    mainDrawCalls: render.calls,
    calls: render.calls,
    triangles: render.triangles,
    points: render.points,
    lines: render.lines,
    geometries: memory.geometries,
    textures: memory.textures,
  };
};
const adapter = {
  renderer_backend: () => 'ThreeWebGL2',
  renderer_stats: actualStats,
  debugStats: actualStats,
  frameProfile: () => ({ enabled: false, cpu: {}, gpu: {
    supported: false, available: false, passesMs: null,
    sampleFrame: null, ageFrames: null,
  } }),
  postprocessing: {
    stats: () => ({ enabled: false, passes: 0 }),
    graph: () => ({ passes: [] }),
  },
};
window.bench = {
  engine: adapter,
  family,
  load,
  operations,
  start() { epoch = performance.now(); active = true; },
  stop() { active = false; },
  capturePose() { active = false; pose(0.375); renderer.render(scene, camera); },
  metadata: {
    family, load, representation, viewport: { width, height, deviceScaleFactor: 1 },
    order, seed, overlap, insertionApplied: true,
    insertionAlgorithm: 'LCG32 Fisher-Yates v1; alternating low/high v1; logical grid independent of insertion',
    firstLogicalIds: insertion.slice(0, 32), poseClock: 'elapsed performance.now seconds', rotationOrder: 'YXZ (Rust Ry*Rx*Rz)',
    scene: 'diagnostic workload; native Three.js WebGL2', unmatchedThreeReference: false,
    loadUnit: 'visible instance count',
    geometry: {
      vertices: 4, indices: 6, normals: '+Z', allObjectsVisible: true,
      logicalRoots: load,
      physicalNodes: representation === 'objects' ? load * 2 : batches.length,
      batches: representation === 'instanced' ? batches.length : 0,
      batchSize: representation === 'instanced' ? batchSize : null,
      frustumCulled: false,
      counterScope: 'renderer.info per actual render; objects expects N calls and 2N triangles',
    },
    material: { type: 'MeshStandardMaterial', baseColor: '#ffffff', metalness: 1, roughness: 1 },
    renderer: { version: THREE.REVISION, fov: 45, antialias: false, effects: false, shadows: false, culling: 'disabled for all fixture meshes' },
    differences: 'PBR illumination and color management differ from Rust; this compares matched geometry/work only, not full Rome visual equivalence.',
    counters: 'renderer.info only; frame CPU/GPU timing and Rust engine counters unavailable',
  },
};

window.addEventListener('pagehide', () => {
  active = false;
  geometry.dispose();
  material.dispose();
  renderer.dispose();
}, { once: true });
