import test from 'node:test';
import assert from 'node:assert/strict';
import { PresentationProfile } from '../js/xr-presentation-profile.js';

function mockGl({ supported = true } = {}) {
    let next = 0;
    const gl = {
        QUERY_RESULT_AVAILABLE: 1, QUERY_RESULT: 2, CURRENT_QUERY: 3,
        ext: { TIME_ELAPSED_EXT: 4, GPU_DISJOINT_EXT: 5 },
        available: false, disjoint: false, lost: false, current: null,
        created: 0, deleted: 0, resultReads: 0, begins: 0, ends: 0, deletedQueries: new Set(),
        getExtension(name) { assert.equal(name, 'EXT_disjoint_timer_query_webgl2'); return supported ? this.ext : null; },
        isContextLost() { return this.lost; },
        getParameter(p) { assert.equal(p, this.ext.GPU_DISJOINT_EXT); return this.disjoint; },
        getQuery(target, p) { assert.equal(target, this.ext.TIME_ELAPSED_EXT); assert.equal(p, this.CURRENT_QUERY); return this.current; },
        createQuery() { this.created++; return { id: ++next, ns: 2500000 }; },
        beginQuery(target, query) { assert.equal(target, this.ext.TIME_ELAPSED_EXT); assert.equal(this.current, null, 'elapsed queries must never nest'); this.current = query; this.begins++; },
        endQuery(target) { assert.equal(target, this.ext.TIME_ELAPSED_EXT); assert.ok(this.current); this.current = null; this.ends++; },
        getQueryParameter(query, p) {
            if (p === this.QUERY_RESULT_AVAILABLE) return this.available;
            assert.equal(p, this.QUERY_RESULT); assert.equal(this.available, true, 'result read before availability');
            this.resultReads++; return query.ns;
        },
        deleteQuery(query) { assert.ok(!this.deletedQueries.has(query), 'query deleted twice'); this.deletedQueries.add(query); this.deleted++; },
    };
    return gl;
}
function frame(profile, gl, n = 0) {
    profile.beginFrame(n);
    profile.scope('render-eye', gl, () => 42);
    profile.endFrame();
}

test('CPU frames retained every callback, snapshots detached, GPU resolves only after availability', () => {
    const gl = mockGl();
    const profile = new PresentationProfile({ rendererGl: gl, xrGl: gl, sampleEvery: 2 });
    frame(profile, gl, 1);
    frame(profile, gl, 2);
    let snapshot = profile.snapshot();
    assert.equal(snapshot.frames.length, 2);
    assert.equal(snapshot.frames[0].scopes[0].gpuMs, null);
    assert.equal(snapshot.frames[1].gpuMode, 'none');
    assert.ok(snapshot.frames.every(f => Number.isFinite(f.cpuMs) && f.cpuMs >= 0 && f.scopes[0].cpuMs >= 0));
    assert.equal(gl.resultReads, 0);
    snapshot.frames[0].scopes[0].label = 'mutated';
    gl.available = true;
    frame(profile, gl, 3);
    snapshot = profile.snapshot();
    assert.equal(snapshot.frames[0].scopes[0].label, 'render-eye');
    assert.equal(snapshot.frames[0].scopes[0].gpuMs, 2.5);
    assert.equal(snapshot.validGpuSamples, 1);
    assert.equal(snapshot.frames[2].gpuMode, 'whole-stereo');
    assert.match(snapshot.intervalMeaning, /not GPU busy/);
    profile.dispose();
});

test('same-context and recursive phases never nest elapsed queries; whole interval alternates', () => {
    const gl = mockGl();
    const p = new PresentationProfile({ rendererGl: gl, xrGl: gl, sampleEvery: 1 });
    p.beginFrame(0);
    assert.equal(p.scope('outer', gl, () => p.scope('inner', gl, () => 'value')), 'value');
    p.endFrame();
    frame(p, gl, 1);
    const s = p.snapshot();
    assert.equal(s.contexts.length, 1);
    assert.equal(gl.begins, 2);
    assert.equal(gl.ends, 2);
    assert.equal(s.frames[0].scopes[1].gpuStatus, 'profile-query-active');
    assert.equal(s.frames[1].application.gpuStatus, 'pending');
    assert.equal(s.frames[1].scopes[0].gpuStatus, 'not-sampled');
    p.dispose();
    assert.equal(gl.deleted, gl.created);
});

test('foreign active elapsed query is preserved', () => {
    const gl = mockGl();
    const foreign = { id: 'foreign' };
    gl.current = foreign;
    const p = new PresentationProfile({ rendererGl: gl, xrGl: gl, sampleEvery: 1 });
    frame(p, gl);
    frame(p, gl);
    assert.equal(gl.current, foreign);
    assert.equal(gl.ends, 0);
    assert.equal(gl.created, 0);
    assert.equal(p.snapshot().reasons['foreign-query-active'], 2);
    p.dispose();
    assert.equal(gl.current, foreign);
});

test('query allocation per context bounded at sixteen and frames/scopes bounded', () => {
    const a = mockGl(), b = mockGl();
    const p = new PresentationProfile({ rendererGl: a, xrGl: b, maxFrames: 3, sampleEvery: 1 });
    for (let i = 0; i < 70; i++) {
        p.beginFrame(i);
        for (let j = 0; j < 140; j++) p.scope('render', a, () => {});
        p.endFrame();
    }
    const s = p.snapshot();
    assert.equal(s.frames.length, 3);
    assert.equal(s.droppedFrames, 67);
    assert.equal(s.droppedScopes, 70 * 12);
    assert.equal(a.created, 16);
    assert.equal(b.created, 16);
    assert.equal(s.pending, 32);
    assert.ok(s.reasons['query-limit'] > 0);
    assert.ok(s.frames.every(f => f.scopes.length === 128));
    p.dispose();
    assert.equal(a.deleted, 16);
    assert.equal(b.deleted, 16);
    assert.equal(new PresentationProfile({ maxFrames: 99999 }).snapshot().limits.maxFrames, 8192);
});

test('completed queries reused rather than repeatedly allocated', () => {
    const gl = mockGl(); gl.available = true;
    const p = new PresentationProfile({ rendererGl: gl, xrGl: gl, sampleEvery: 1 });
    for (let i = 0; i < 100; i++) frame(p, gl, i);
    assert.equal(gl.created, 1);
    p.dispose();
    assert.equal(gl.deleted, 1);
    assert.equal(p.snapshot().pending, 0);
});

test('disjoint invalidates pending samples without reading results', () => {
    const gl = mockGl();
    const p = new PresentationProfile({ rendererGl: gl, xrGl: gl, sampleEvery: 1 });
    frame(p, gl);
    gl.disjoint = true;
    frame(p, gl);
    const s = p.snapshot();
    assert.equal(s.frames[0].scopes[0].gpuStatus, 'disjoint');
    assert.equal(s.frames[0].scopes[0].gpuMs, null);
    assert.equal(s.frames[1].application.gpuStatus, 'disjoint');
    assert.equal(gl.resultReads, 0);
    assert.equal(gl.deleted, gl.created);
    p.dispose();
});

test('unsupported and invalid results remain null with explicit reasons', () => {
    const no = mockGl({ supported: false });
    const p = new PresentationProfile({ rendererGl: no, xrGl: no, sampleEvery: 1 });
    frame(p, no);
    assert.equal(p.snapshot().frames[0].scopes[0].gpuStatus, 'unsupported');
    assert.equal(p.snapshot().frames[0].scopes[0].gpuMs, null);
    for (const value of [NaN, -1, 0, '1000']) {
        const gl = mockGl();
        const q = new PresentationProfile({ rendererGl: gl, xrGl: gl });
        frame(q, gl);
        q.contexts.get(gl).pending[0].query.ns = value;
        gl.available = true;
        frame(q, gl);
        assert.equal(q.snapshot().frames[0].scopes[0].gpuStatus, 'invalid-result');
        assert.equal(q.snapshot().frames[0].scopes[0].gpuMs, null);
        q.dispose();
    }
    p.dispose();
});

test('throwing callbacks keep CPU evidence and close queries; dispose is idempotent', () => {
    const gl = mockGl();
    const p = new PresentationProfile({ rendererGl: gl, xrGl: gl, sampleEvery: 1 });
    p.beginFrame(0);
    assert.throws(() => p.scope('throw', gl, () => { throw new Error('original'); }), /original/);
    assert.equal(gl.current, null);
    p.endFrame();
    assert.ok(p.snapshot().frames[0].scopes[0].cpuMs >= 0);
    p.beginFrame(1);
    assert.ok(gl.current);
    p.dispose(); p.dispose();
    assert.equal(gl.current, null);
    assert.equal(gl.deleted, gl.created);
    assert.equal(p.snapshot().pending, 0);
    assert.equal(p.scope('after', gl, () => 9), 9);
});

test('context loss in callback and between callbacks invalidates and releases queries', () => {
    const gl = mockGl();
    const p = new PresentationProfile({ rendererGl: gl, xrGl: gl, sampleEvery: 1 });
    p.beginFrame(0);
    p.scope('lost', gl, () => { gl.lost = true; gl.current = null; });
    p.endFrame();
    assert.equal(p.snapshot().frames[0].scopes[0].gpuStatus, 'context-lost');
    assert.equal(gl.deleted, gl.created);
    gl.lost = false;
    frame(p, gl, 1);
    gl.lost = true;
    frame(p, gl, 2);
    assert.equal(p.snapshot().frames[1].application.gpuStatus, 'context-lost');
    assert.equal(gl.deleted, gl.created);
    assert.equal(gl.resultReads, 0);
    p.dispose();
});

test('maximum retained frame cap remains bounded across more than 8192 callbacks', () => {
    const p = new PresentationProfile({ maxFrames: 1e9 });
    for (let i = 0; i < 8200; i++) { p.beginFrame(i); p.endFrame(); }
    const s = p.snapshot();
    assert.equal(s.frames.length, 8192);
    assert.equal(s.droppedFrames, 8);
    assert.equal(s.frames[0].index, 8);
    p.dispose();
});

test('query API failures release allocated resources and preserve callback result', () => {
    const gl = mockGl();
    gl.beginQuery = () => { throw new Error('driver'); };
    const p = new PresentationProfile({ rendererGl: gl, xrGl: gl });
    p.beginFrame(0);
    assert.equal(p.scope('safe', gl, () => 17), 17);
    p.endFrame();
    assert.equal(p.snapshot().frames[0].scopes[0].gpuStatus, 'query-error');
    assert.equal(gl.deleted, gl.created);
    p.dispose();
    const other = mockGl();
    const q = new PresentationProfile({ rendererGl: other, xrGl: other });
    frame(q, other);
    other.getQueryParameter = () => { throw new Error('driver'); };
    frame(q, other);
    assert.equal(q.snapshot().frames[0].scopes[0].gpuStatus, 'query-error');
    assert.equal(other.deleted, other.created);
    q.dispose();
});
