import test from 'node:test';
import assert from 'node:assert/strict';
import {arena,faces,projectArenaRay} from '../examples/neon-arena/environment-projection.mjs';
const near=(a,b)=>assert.ok(Math.abs(a-b)<1e-10,`${a} != ${b}`);
test('arena horizon and floor correspond to metric eye height, not photo midpoint',()=>{
    const horizon=projectArenaRay([0,0,-1]);
    assert.equal(horizon.surface,'wall');
    near(horizon.v,.075+(1-arena.eyeHeight/arena.height)*.535);
    assert.equal(projectArenaRay([0,-arena.eyeHeight/arena.radius-.0001,-1]).surface,'floor');
    assert.equal(projectArenaRay([0,(arena.height-arena.eyeHeight)/arena.radius+.0001,-1]).surface,'sky');
    assert.equal(projectArenaRay([0,1,0]).surface,'sky');
    assert.equal(projectArenaRay([0,-1,0]).surface,'floor');
});
test('projection is angular and cube edge directions agree',()=>{
    const map=new Map(faces);
    for(const t of [-.9,-.3,0,.4,.9]) {
        assert.deepEqual(map.get('px')(-1,t),map.get('pz')(1,t));
        assert.deepEqual(map.get('px')(1,t),map.get('nz')(-1,t));
        assert.deepEqual(map.get('px')(t,-1),map.get('py')(1,-t));
        const a=projectArenaRay(map.get('px')(-1,t)),b=projectArenaRay(map.get('pz')(1,t));
        assert.deepEqual(a,b);
        const c=projectArenaRay([1,.1,-1]),d=projectArenaRay([10,1,-10]);
        assert.equal(c.surface,d.surface);near(c.u,d.u);near(c.v,d.v);
    }
});
