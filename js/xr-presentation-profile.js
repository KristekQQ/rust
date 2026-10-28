/** @typedef {{ TIME_ELAPSED_EXT: number, GPU_DISJOINT_EXT: number }} ElapsedTimerExtension */
/** @typedef {{ label: string, gpuMs: number|null, gpuStatus: string }} GpuSample */
/** @typedef {GpuSample & { context: string, cpuMs: number|null }} CpuScope */
/** @typedef {{ name: string, gl: WebGL2RenderingContext, ext: ElapsedTimerExtension|null, free: WebGLQuery[], pending: PendingQuery[], active: PendingQuery|null, allocated: number }} ProfileContext */
/** @typedef {{ query: WebGLQuery, row: GpuSample, context: ProfileContext }} PendingQuery */
/** @typedef {{ index: number, timestamp: number, cpuMs: number|null, gpuMode: 'none'|'phases'|'whole-stereo', scopes: CpuScope[], application: GpuSample|null, started?: number, wholeQuery?: PendingQuery|null }} ProfileFrame */

/** Opt-in diagnostics. GPU intervals measure application-command elapsed time,
 * including GPU scheduling gaps; they do not measure GPU busy time or compositor work. */
export class PresentationProfile {
    /** @param {{rendererGl?: WebGL2RenderingContext|null, xrGl?: WebGL2RenderingContext|null, maxFrames?: number, sampleEvery?: number}} [options] */
    constructor({ rendererGl, xrGl, maxFrames = 4096, sampleEvery = 30 } = {}) {
        this.maxFrames = Math.max(1, Math.min(8192, Math.floor(Number(maxFrames) || 4096)));
        this.sampleEvery = Math.max(1, Math.floor(Number(sampleEvery) || 30));
        /** @type {Map<WebGL2RenderingContext, ProfileContext>} */
        this.contexts = new Map();
        /** @type {[string, WebGL2RenderingContext|null|undefined][]} */
        const sources = [['renderer', rendererGl], ['xr', xrGl]];
        for (const [name, gl] of sources) {
            if (!gl || this.contexts.has(gl)) continue;
            /** @type {ElapsedTimerExtension|null} */
            let ext = null;
            try { ext = /** @type {ElapsedTimerExtension|null} */ (gl.getExtension('EXT_disjoint_timer_query_webgl2')); } catch { /* unavailable */ }
            this.contexts.set(gl, { name, gl, ext, free: [], pending: [], active: null, allocated: 0 });
        }
        this.xrGl = xrGl;
        /** @type {ProfileFrame[]} */
        this.frames = [];
        /** @type {ProfileFrame|null} */
        this.frame = null;
        this.frameCount = 0;
        this.droppedFrames = 0;
        this.droppedScopes = 0;
        this.sampleCount = 0;
        /** @type {Record<string, number>} */
        this.reasons = {};
        this.disposed = false;
    }
    /** @param {string} reason */
    _reason(reason) { this.reasons[reason] = (this.reasons[reason] || 0) + 1; }
    /** @param {GpuSample} row @param {string} reason */
    _unavailable(row, reason) { row.gpuMs = null; row.gpuStatus = reason; this._reason(reason); }
    /** @param {ProfileContext} c @param {WebGLQuery} query */
    _delete(c, query) {
        try { c.gl.deleteQuery(query); } catch { /* lost contexts may reject cleanup */ }
        c.allocated--;
    }
    /** @param {ProfileContext} c @param {string} reason */
    _clear(c, reason) {
        if (c.active) this._close(c, c.active);
        for (const item of c.pending) { this._unavailable(item.row, reason); this._delete(c, item.query); }
        c.pending.length = 0;
        for (const query of c.free) this._delete(c, query);
        c.free.length = 0;
    }
    /** @param {ProfileContext} c */
    _poll(c) {
        if (!c.ext) return;
        try {
            if (c.gl.isContextLost()) { this._clear(c, 'context-lost'); return; }
            if (c.gl.getParameter(c.ext.GPU_DISJOINT_EXT)) { this._clear(c, 'disjoint'); return; }
            for (let i = c.pending.length - 1; i >= 0; i--) {
                const item = c.pending[i];
                if (!c.gl.getQueryParameter(item.query, c.gl.QUERY_RESULT_AVAILABLE)) continue;
                const ns = c.gl.getQueryParameter(item.query, c.gl.QUERY_RESULT);
                if (typeof ns === 'number' && Number.isFinite(ns) && ns > 0) {
                    item.row.gpuMs = ns / 1e6;
                    item.row.gpuStatus = 'valid';
                } else this._unavailable(item.row, 'invalid-result');
                c.pending.splice(i, 1);
                c.free.push(item.query);
            }
        } catch { this._clear(c, 'query-error'); }
    }
    /** @param {WebGL2RenderingContext|null|undefined} gl @param {GpuSample} row @returns {PendingQuery|null} */
    _open(gl, row) {
        const c = gl ? this.contexts.get(gl) : undefined;
        if (!gl || !c?.ext) { this._unavailable(row, 'unsupported'); return null; }
        try {
            if (gl.isContextLost()) { this._unavailable(row, 'context-lost'); return null; }
            if (gl.getParameter(c.ext.GPU_DISJOINT_EXT)) { this._clear(c, 'disjoint'); this._unavailable(row, 'disjoint'); return null; }
            if (c.active || gl.getQuery(c.ext.TIME_ELAPSED_EXT, gl.CURRENT_QUERY)) {
                this._unavailable(row, c.active ? 'profile-query-active' : 'foreign-query-active'); return null;
            }
            /** @type {WebGLQuery|null|undefined} */
            let query = c.free.pop();
            if (!query) {
                if (c.allocated >= 16) { this._unavailable(row, 'query-limit'); return null; }
                query = gl.createQuery();
                if (!query) { this._unavailable(row, 'query-unavailable'); return null; }
                c.allocated++;
            }
            try { gl.beginQuery(c.ext.TIME_ELAPSED_EXT, query); }
            catch (error) { this._delete(c, query); throw error; }
            row.gpuStatus = 'pending';
            return (c.active = { query, row, context: c });
        } catch { this._unavailable(row, 'query-error'); return null; }
    }
    /** @param {ProfileContext} c @param {PendingQuery} item */
    _close(c, item) {
        if (c.active !== item) return;
        c.active = null;
        try {
            if (c.gl.isContextLost()) throw new Error('context-lost');
            if (!c.ext) throw new Error('unsupported');
            if (c.gl.getQuery(c.ext.TIME_ELAPSED_EXT, c.gl.CURRENT_QUERY) !== item.query) throw new Error('query-replaced');
            c.gl.endQuery(c.ext.TIME_ELAPSED_EXT);
            c.pending.push(item);
        } catch (error) {
            this._unavailable(item.row, error instanceof Error && error.message === 'context-lost' ? 'context-lost' : 'query-error');
            this._delete(c, item.query);
        }
    }
    /** @param {number} timestamp */
    beginFrame(timestamp) {
        if (this.disposed) return;
        if (this.frame) this.endFrame();
        const started = performance.now();
        for (const c of this.contexts.values()) this._poll(c);
        const index = this.frameCount++;
        const sampled = index % this.sampleEvery === 0;
        const gpuMode = sampled ? (this.sampleCount++ % 2 ? 'whole-stereo' : 'phases') : 'none';
        this.frame = { index, timestamp, cpuMs: null, gpuMode, scopes: [], application: null, started };
        if (gpuMode === 'whole-stereo') {
            /** @type {GpuSample} */
            const row = { label: 'whole-stereo-application', gpuMs: null, gpuStatus: 'pending' };
            this.frame.application = row;
            this.frame.wholeQuery = this._open(this.xrGl, row);
        }
    }
    /** @template T @param {string} label @param {WebGL2RenderingContext} gl @param {() => T} fn @returns {T} */
    scope(label, gl, fn) {
        if (!this.frame || this.disposed) return fn();
        const frame = this.frame;
        if (frame.scopes.length >= 128) { this.droppedScopes++; return fn(); }
        /** @type {CpuScope} */
        const row = { label: String(label).slice(0, 128), context: this.contexts.get(gl)?.name ?? 'unknown', cpuMs: null, gpuMs: null, gpuStatus: frame.gpuMode === 'phases' ? 'pending' : 'not-sampled' };
        frame.scopes.push(row);
        const start = performance.now();
        const query = frame.gpuMode === 'phases' ? this._open(gl, row) : null;
        try { return fn(); }
        finally {
            if (query) this._close(query.context, query);
            row.cpuMs = performance.now() - start;
        }
    }
    endFrame() {
        const frame = this.frame;
        if (!frame) return;
        if (frame.wholeQuery) this._close(frame.wholeQuery.context, frame.wholeQuery);
        frame.cpuMs = performance.now() - (frame.started ?? performance.now());
        delete frame.started;
        delete frame.wholeQuery;
        if (this.frames.length >= this.maxFrames) { this.frames.shift(); this.droppedFrames++; }
        this.frames.push(frame);
        this.frame = null;
    }
    snapshot() {
        const frames = /** @type {ProfileFrame[]} */ (JSON.parse(JSON.stringify(this.frames)));
        let validGpuSamples = 0;
        for (const frame of frames) {
            for (const row of frame.scopes) if (row.gpuStatus === 'valid') validGpuSamples++;
            if (frame.application?.gpuStatus === 'valid') validGpuSamples++;
        }
        return {
            intervalMeaning: 'application-command elapsed; not GPU busy time or compositor time',
            frames, frameCount: this.frameCount, droppedFrames: this.droppedFrames, droppedScopes: this.droppedScopes,
            validGpuSamples, pending: [...this.contexts.values()].reduce((n, c) => n + c.pending.length + Number(Boolean(c.active)), 0),
            reasons: { ...this.reasons }, disposed: this.disposed,
            limits: { maxFrames: this.maxFrames, sampleEvery: this.sampleEvery, maxQueriesPerContext: 16, maxScopesPerFrame: 128 },
            contexts: [...this.contexts.values()].map(c => ({ name: c.name, supported: Boolean(c.ext), allocatedQueries: c.allocated })),
        };
    }
    dispose() {
        if (this.disposed) return;
        this.endFrame();
        for (const c of this.contexts.values()) this._clear(c, 'disposed');
        this.disposed = true;
    }
}
