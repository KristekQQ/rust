// Self-contained for evaluateOnNewDocument. The minimal path allocates its
// bounded storage before loading the application, never in a measured RAF.
export function installFrameRecorder(mode = 'sampled', capacity = 20000) {
  const original = window.requestAnimationFrame.bind(window);
  const m = window.perfMeasure = {
    active: false, mode, capacity, count: 0, overflow: false, frames: [],
    stamps: mode === 'minimal' ? new Float64Array(capacity) : null,
    cpu: mode === 'minimal' ? new Float64Array(capacity) : null,
  };
  m.reset = () => { m.count = 0; m.overflow = false; m.frames = []; };
  m.snapshot = () => {
    if (m.active) throw new Error('Cannot materialize frame rows during timing');
    return mode === 'sampled' ? m.frames : Array.from({length: m.count},
      (_, i) => ({stamp: m.stamps[i], cpu: m.cpu[i]}));
  };
  window.requestAnimationFrame = fn => original(t => {
    if (!m.active) return fn(t);
    let row, index;
    if (mode === 'minimal') {
      index = m.count - 1;
      if (index < 0 || m.stamps[index] !== t) {
        if (m.count === capacity) { m.overflow = true; return fn(t); }
        index = m.count++;
        m.stamps[index] = t;
        m.cpu[index] = 0;
      }
    } else {
      row = m.frames.at(-1);
      if (!row || row.stamp !== t) {
        if (m.frames.length === capacity) { m.overflow = true; return fn(t); }
        row = {stamp: t, cpu: 0}; m.frames.push(row);
      }
    }
    const start = performance.now();
    try {
      if (mode === 'sampled') window.renderGlRafTimer?.beginFrame(t);
      return fn(t);
    } finally {
      const elapsed = performance.now() - start;
      if (mode === 'minimal') m.cpu[index] += elapsed;
      else row.cpu += elapsed;
    }
  });
}
