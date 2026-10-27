// Original geometry buffers stay on the rendering thread. Only owned copies
// enter the worker; generated output buffers transfer back once per result.
export function createCutWorker() {
  const cache = new WeakMap(), pending = new Map(), registrations = [];
  let worker, sequence = 0, geometryCount = 0, state = 'starting', failure = null;
  let resolveReady, rejectReady;
  const ready = new Promise((resolve, reject) => { resolveReady = resolve; rejectReady = reject; });
  ready.catch(() => {});
  const error = (name, message) => Object.assign(new Error(message), {name});
  const fail = cause => {
    if (state === 'disposed' || state === 'unavailable') return;
    failure = cause; state = 'unavailable';
    rejectReady(cause);
    for (const request of pending.values()) request.reject(cause);
    pending.clear(); worker?.terminate();
  };
  const request = (type, payload = {}, transfers = []) => {
    const id = ++sequence;
    const promise = new Promise((resolve, reject) => {
      if (!worker || state === 'unavailable' || state === 'disposed') {
        reject(failure || error('InvalidStateError', `Cut worker ${state}`)); return;
      }
      if (type === 'cut' && [...pending.values()].filter(entry => entry.type === 'cut').length >= 8) {
        reject(error('QuotaExceededError', 'Cut worker request limit reached')); return;
      }
      pending.set(id, {resolve, reject, type});
      try { worker.postMessage({type, id, ...payload}, transfers); }
      catch (cause) { pending.delete(id); reject(cause); }
    });
    promise.catch(() => {});
    return {id, promise, cancel() {
      const entry = pending.get(id);
      if (!entry) return;
      pending.delete(id); entry.reject(error('AbortError', 'Cut request cancelled'));
      try { worker?.postMessage({type: 'cancel', id}); } catch { /* disposed */ }
    }};
  };
  try {
    if (typeof Worker === 'undefined') throw error('NotSupportedError', 'Module workers unavailable');
    worker = new Worker(new URL('./cut-worker.js', import.meta.url), {type: 'module', name: 'aurelia-mesh-cut'});
    worker.onmessage = ({data}) => {
      if (state === 'disposed' || state === 'unavailable') return;
      if (data.type === 'ready') { state = 'ready'; resolveReady(); return; }
      const entry = pending.get(data.id);
      if (!entry) return;
      pending.delete(data.id);
      if (data.type === 'error') entry.reject(error(data.error?.name || 'Error', data.error?.message || 'Cut worker failed'));
      else entry.resolve(data.type === 'result' ? {meshes: data.results, computeMs: data.computeMs} : undefined);
    };
    worker.onerror = event => { event.preventDefault?.(); fail(error('NotSupportedError', event.message || 'Cut worker failed to load')); };
    worker.onmessageerror = () => fail(error('DataCloneError', 'Invalid cut worker response'));
  } catch (cause) { fail(cause); }
  return {
    ready,
    registerGeometry(geometry) {
      if (state === 'disposed' || state === 'unavailable') throw failure || error('InvalidStateError', `Cut worker ${state}`);
      const existing = cache.get(geometry);
      if (existing !== undefined) return existing;
      const geometryId = ++geometryCount, data = {groups: geometry.groups.map(group => ({...group}))}, transfers = [];
      for (const name of ['position', 'normal', 'uv']) {
        const attribute = geometry.getAttribute(name);
        if (!attribute) continue;
        const size = name === 'uv' ? 2 : 3;
        const sourceArray = attribute.isInterleavedBufferAttribute ? attribute.data.array : attribute.array;
        const ArrayType = sourceArray instanceof Float32Array && !attribute.normalized ? Float32Array : Float64Array;
        const array = new ArrayType(attribute.count * size);
        for (let i = 0; i < attribute.count; i++) {
          array[i * size] = attribute.getX(i); array[i * size + 1] = attribute.getY(i);
          if (size === 3) array[i * size + 2] = attribute.getZ(i);
        }
        data[name] = array; transfers.push(array.buffer);
      }
      if (!data.position) throw new Error('Cut geometry requires position');
      if (geometry.index) {
        data.index = Uint32Array.from({length: geometry.index.count}, (_, i) => geometry.index.getX(i));
        transfers.push(data.index.buffer);
      }
      const registration = request('register', {geometryId, geometry: data}, transfers);
      registrations.push(registration.promise); cache.set(geometry, geometryId);
      return geometryId;
    },
    async prepare() {
      await ready;
      await Promise.all(registrations);
      await request('prepare').promise;
    },
    cut(meshes) { return request('cut', {meshes}); },
    describe() { return {state, registered: geometryCount, pending: pending.size, error: failure?.message || null}; },
    dispose() {
      if (state === 'disposed') return;
      const cause = error('AbortError', 'Cut worker disposed');
      rejectReady(cause);
      for (const entry of pending.values()) entry.reject(cause);
      pending.clear(); worker?.terminate(); state = 'disposed'; failure = cause;
    }
  };
}
