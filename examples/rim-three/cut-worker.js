import * as THREE from './vendor/three.module.js';
import {prepareGeometry, splitGeometry} from './mesh-split.js';

const geometries = new Map(), queue = [];
let scheduled = false;
const sendError = (id, error) => postMessage({type: 'error', id, error: {name: error.name, message: error.message}});
const serialize = (geometry, source, transfers) => {
  if (!geometry) return null;
  if (geometry === source) return {shared: true};
  const data = {groups: geometry.groups.map(group => ({...group}))};
  for (const name of ['position', 'normal', 'uv']) {
    const attribute = geometry.getAttribute(name);
    if (attribute) { data[name] = attribute.array; transfers.push(attribute.array.buffer); }
  }
  data.boundingBox = geometry.boundingBox ? {min: geometry.boundingBox.min.toArray(), max: geometry.boundingBox.max.toArray()} : null;
  data.boundingSphere = geometry.boundingSphere ? {center: geometry.boundingSphere.center.toArray(), radius: geometry.boundingSphere.radius} : null;
  return data;
};
const pump = () => {
  scheduled = false;
  const job = queue.shift();
  if (!job) return;
  try {
    const started = performance.now();
    const transfers = [], results = [];
    for (const mesh of job.meshes) {
      const geometry = geometries.get(mesh.geometryId);
      if (!geometry) throw new Error(`Unknown cut geometry ${mesh.geometryId}`);
      if (!Array.isArray(mesh.plane) || mesh.plane.length !== 4 || !mesh.plane.every(Number.isFinite)
        || Math.hypot(...mesh.plane.slice(0,3)) === 0) throw new Error('Invalid cut plane');
      const plane = new THREE.Plane(new THREE.Vector3(...mesh.plane.slice(0,3)), mesh.plane[3]);
      const result = splitGeometry(geometry, plane, mesh.capIndex ?? 0);
      results.push({positive: serialize(result.positive, geometry, transfers), negative: serialize(result.negative, geometry, transfers), capTriangles: result.capTriangles});
      for (const owned of result.owned) owned.dispose();
    }
    postMessage({type: 'result', id: job.id, results, computeMs: performance.now() - started}, transfers);
  } catch (error) { sendError(job.id, error); }
  schedule();
};
const schedule = () => {
  if (!scheduled && queue.length) { scheduled = true; setTimeout(pump, 0); }
};
self.onmessage = ({data}) => {
  try {
    if (data.type === 'register') {
      const geometry = new THREE.BufferGeometry();
      for (const name of ['position', 'normal', 'uv']) if (data.geometry[name])
        geometry.setAttribute(name, new THREE.BufferAttribute(data.geometry[name], name === 'uv' ? 2 : 3));
      if (data.geometry.index) geometry.setIndex(new THREE.BufferAttribute(data.geometry.index, 1));
      geometry.groups = data.geometry.groups;
      geometry.computeBoundingBox();
      prepareGeometry(geometry);
      geometries.set(data.geometryId, geometry);
      postMessage({type: 'ack', id: data.id});
    } else if (data.type === 'prepare') {
      postMessage({type: 'ack', id: data.id});
    } else if (data.type === 'cut') {
      queue.push(data); schedule();
    } else if (data.type === 'cancel') {
      const index = queue.findIndex(job => job.id === data.id);
      if (index !== -1) queue.splice(index, 1);
    }
  } catch (error) { sendError(data.id, error); }
};
postMessage({type: 'ready'});
