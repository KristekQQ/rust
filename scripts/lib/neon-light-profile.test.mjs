import test from 'node:test';import assert from 'node:assert/strict';
import{profile,lightAt,lightContribution,floorIntersection}from'./neon-light-profile.mjs';
test('both shaft axes target actor torso and continue onto floor',()=>{
 assert.equal(profile.lights.length,2);
 for(const l of profile.lights){assert.deepEqual(l.aimTarget,[0,1.1,0]);const p=floorIntersection(l);assert.ok(Math.abs(p[1])<1e-8);assert.ok(Math.abs(lightContribution(l,p[0],p[2])-.3)<1e-8);}
});
test('actor feet lit, bounded symmetric stage spill, finite floor edges black',()=>{
 assert.ok(lightAt(0,0)>.4);assert.equal(lightAt(0,-4),0);assert.equal(lightAt(0,2.25),0);
 for(let x=-3;x<=3;x+=.13)for(let z=-5;z<=2.2;z+=.17){const a=lightAt(x,z);assert.ok(Number.isFinite(a)&&a>=0&&a<=1);assert.ok(Math.abs(a-lightAt(-x,z))<1e-10);}
});

test('two distinct forward floor pools stay brighter than the gap',()=>{
 assert.ok(lightAt(.68,-.65)>lightAt(0,-.65)*1.5);
 assert.ok(lightAt(-.68,-.65)>lightAt(0,-.65)*1.5);
});
