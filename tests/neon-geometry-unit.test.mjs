import test from 'node:test';
import assert from 'node:assert/strict';
import {fruitGeometries, wildSymbol, createFruitPatternCatalog, FRUIT_PATTERN_COUNT, fruitPatternCoverage, fruitPatternDescriptor, neonFrameGeometry} from '../examples/neon-arena/geometry.js';

test('neon frame is a closed circular tube with smooth bends and bright front bands',()=>{
    const mesh=neonFrameGeometry(),edges=new Map();
    for(let i=0;i<mesh.indices.length;i+=3){
        const tri=mesh.indices.slice(i,i+3);
        for(let j=0;j<3;j++){
            const a=tri[j],b=tri[(j+1)%3],key=[Math.min(a,b),Math.max(a,b)].join(':');
            const edge=edges.get(key)??[0,0];edge[0]++;edge[1]+=a<b?1:-1;edges.set(key,edge);
        }
    }
    assert([...edges.values()].every(([count,winding])=>count===2&&winding===0),'closed consistently oriented surface');
    const zs=mesh.positions.filter((_,i)=>i%3===2);
    assert(Math.abs(Math.max(...zs)-Math.min(...zs)-.012)<1e-9,'12 mm circular depth');
    assert(mesh.triangleColors.some((c,i)=>i%3===2&&c>.9),'luminous front');
    assert(mesh.triangleColors.some((c,i)=>i%3===2&&c<.25),'dim blue sides');
});

const meshes = seed => [...fruitGeometries({seed}), wildSymbol({seed})];
const brightness = (mesh, triangle) => Math.max(...mesh.triangleColors.slice(triangle * 3, triangle * 3 + 3));
const triangle = (mesh, index) => mesh.indices.slice(index * 3, index * 3 + 3);

// A facet consists of its filled center followed by luminous boundary trapezoids.
function facets(mesh) {
    const result = [];
    for (let i = 0; i < mesh.indices.length / 3;) {
        if (brightness(mesh, i) >= .25) { i++; continue; } // Solid stalks.
        const center = [], seams = [];
        while (i < mesh.indices.length / 3 && brightness(mesh, i) < .25) center.push(i++);
        const boundaryTriangles = 2 * (center.length + 2);
        for (let edge = 0; edge < boundaryTriangles; edge++) {
            assert.ok(brightness(mesh, i) >= .25, 'bright polygon boundary');
            seams.push(i++);
        }
        result.push({center, seams});
    }
    return result;
}

test('seeded facet brightness is stable, varied within every fruit and bounded below cutout', () => {
    const first = meshes(17), same = meshes(17), other = meshes(18);
    assert.deepEqual(first, same);
    for (let i = 0; i < first.length; i++) {
        const {name, geometry} = first[i];
        assert.deepEqual(geometry.positions, other[i].geometry.positions, name);
        assert.deepEqual(geometry.indices, other[i].geometry.indices, name);
        assert.notDeepEqual(geometry.triangleColors, other[i].geometry.triangleColors, name);
        const values = facets(geometry).flatMap(({center}) => center.map(t => brightness(geometry, t)));
        assert.ok(new Set(values.map(v => v.toFixed(7))).size >= 12, `${name}: varied panels`);
        assert.ok(values.every(v => v >= .15 - 1e-12 && v <= .23 + 1e-12), `${name}: brightness range`);
        for (let t = 0; t < geometry.indices.length / 3; t++) {
            if (brightness(geometry, t) >= .25) {
                assert.deepEqual(geometry.triangleColors.slice(t * 3, t * 3 + 3),
                    other[i].geometry.triangleColors.slice(t * 3, t * 3 + 3), `${name}: seed preserves seams`);
            }
        }
    }
    assert.doesNotThrow(() => fruitGeometries());
    assert.doesNotThrow(() => wildSymbol());
});

test('lathed fruit panels are quads with one shared opacity key and no luminous diagonal', () => {
    for (const {name, geometry} of fruitGeometries()) {
        if (name === 'banana') continue; // Its bent tube cells are not coplanar.
        const polygons = facets(geometry);
        const quads = polygons.filter(({center}) => center.length === 2);
        assert.ok(quads.length >= 6, `${name}: actual quad facets`);
        for (const {center, seams} of quads) {
            assert.equal(seams.length, 8, `${name}: four boundary trapezoids`);
            assert.ok(Math.abs(brightness(geometry, center[0]) - brightness(geometry, center[1])) < 1e-12,
                `${name}: center triangles share baked brightness`);
            assert.deepEqual(geometry.triangleColors.slice(center[0]*3,center[0]*3+3),geometry.triangleColors.slice(center[1]*3,center[1]*3+3),`${name}: entire quad has one RGB palette, not two colored triangles`);
            const a = triangle(geometry, center[0]), b = triangle(geometry, center[1]);
            const diagonal = a.filter(vertex => b.includes(vertex));
            assert.equal(diagonal.length, 2);
            for (const seam of seams) {
                const indices = triangle(geometry, seam);
                assert.ok(!diagonal.every(vertex => indices.includes(vertex)), `${name}: no luminous diagonal`);
            }
            const [p, q, r] = a.map(v => geometry.positions.slice(v * 3, v * 3 + 3));
            const d = geometry.positions.slice(b.find(v => !a.includes(v)) * 3).slice(0, 3);
            const u = q.map((v, i) => v - p[i]), v = r.map((v, i) => v - p[i]);
            const n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]];
            assert.ok(Math.abs(n.reduce((sum, value, i) => sum + value * (d[i] - p[i]), 0)) < 1e-10);
        }
    }
});

test('every fruit and wild symbol remains an indexed closed solid with consistent outward winding', () => {
    for (const {name, geometry} of meshes(42)) {
        const edges = new Map();
        let volume = 0;
        for (let t = 0; t < geometry.indices.length / 3; t++) {
            const indices = triangle(geometry, t);
            const [a, b, c] = indices.map(v => geometry.positions.slice(v * 3, v * 3 + 3));
            const ab = b.map((v, i) => v - a[i]), ac = c.map((v, i) => v - a[i]);
            const normal = [ab[1]*ac[2]-ab[2]*ac[1], ab[2]*ac[0]-ab[0]*ac[2], ab[0]*ac[1]-ab[1]*ac[0]];
            assert.ok(Math.hypot(...normal) > 1e-12, `${name}: nondegenerate triangle ${t}`);
            volume += a[0]*(b[1]*c[2]-b[2]*c[1]) + a[1]*(b[2]*c[0]-b[0]*c[2]) + a[2]*(b[0]*c[1]-b[1]*c[0]);
            for (let e = 0; e < 3; e++) {
                const a = indices[e], b = indices[(e + 1) % 3];
                const key = `${Math.min(a,b)},${Math.max(a,b)}`;
                const edge = edges.get(key) ?? {count: 0, direction: 0};
                edge.count++;
                edge.direction += a < b ? 1 : -1;
                edges.set(key, edge);
            }
        }
        assert.ok(volume > 0, `${name}: outward signed volume`);
        for (const [key, edge] of edges) assert.deepEqual(edge, {count: 2, direction: 0}, `${name}: closed edge ${key}`);
    }
});


const maskDistance = (a,b) => a.reduce((sum,value,i)=>sum+Math.abs(value-b[i]),0)/(255*a.length);
const maskFill = mask => mask.reduce((sum,value)=>sum+value,0);

test('each fruit retains fifteen visibly distinct masks, original base and shared topology', () => {
    const {fruits,wild,variants}=createFruitPatternCatalog();
    assert.equal(FRUIT_PATTERN_COUNT,15);
    for(const base of [...fruits,wild]){
        const choices=variants[base.name];
        assert.equal(choices.length,15,base.name);
        assert.equal(choices[0],base,`${base.name}: preserve gallery/base object`);
        const masks=choices.map(fruit=>fruitPatternDescriptor(fruit.geometry,fruit.coverage));
        assert.equal(new Set(masks.map(mask=>mask.join(','))).size,15,`${base.name}: unique actual fill masks`);
        const pairDistances=masks.flatMap((mask,i)=>masks.slice(i+1).map(other=>maskDistance(mask,other)));
        assert.ok(Math.min(...pairDistances)>=.28,`${base.name}: every pair differs on at least 28% of panel fill`);
        assert.ok(choices.every(fruit=>fruit.geometry.positions===base.geometry.positions&&fruit.geometry.indices===base.geometry.indices),`${base.name}: shared topology`);
        for(const {geometry} of choices)for(const {center} of facets(geometry)) {
            for(const t of center)assert.deepEqual(geometry.triangleColors.slice(t*3,t*3+3),geometry.triangleColors.slice(center[0]*3,center[0]*3+3),`${base.name}: grouped quad RGB`);
        }
    }
});

test('cyclic pattern order maximizes real fill changes and preserves opaque and sparse extremes', () => {
    const {variants}=createFruitPatternCatalog();
    const candidateFills=Object.fromEntries(Object.keys(variants).map(name=>[name,[]]));
    for(let seed=0;seed<40;seed++)for(const fruit of meshes(seed)) {
        candidateFills[fruit.name].push(maskFill(fruitPatternDescriptor(fruit.geometry,fruitPatternCoverage(seed))));
    }
    for(const [name,choices] of Object.entries(variants)) {
        const masks=choices.map(fruit=>fruitPatternDescriptor(fruit.geometry,fruit.coverage));
        const adjacent=masks.map((mask,i)=>maskDistance(mask,masks[(i+1)%masks.length]));
        assert.ok(Math.min(...adjacent)>=.49,`${name}: adjacent masks including wrap differ by at least 49%`);
        const fills=masks.map(maskFill);
        assert.equal(Math.max(...fills),Math.max(...candidateFills[name]),`${name}: retain strongest full mask`);
        assert.equal(Math.min(...fills),Math.min(...candidateFills[name]),`${name}: retain sparsest mask`);
        assert.equal(masks.filter(mask=>mask.every(value=>value===255)).length,1,`${name}: exactly one entirely opaque variant`);
        assert.ok(choices.every(fruit=>fruit.coverage>=0&&fruit.coverage<=1));
    }
    assert.deepEqual(createFruitPatternCatalog(),createFruitPatternCatalog(),'selection and cyclic order are deterministic');
});

test('fill descriptor counts polygon centers once and follows coverage threshold endpoints', () => {
    for(const {name,geometry} of meshes(17)) {
        const panels=facets(geometry);
        const clear=fruitPatternDescriptor(geometry,0),full=fruitPatternDescriptor(geometry,1);
        assert.equal(clear.length,panels.length,`${name}: one entry per polygon, excluding seams/stalks`);
        assert.ok(clear.every(value=>value===0));
        assert.ok(full.every(value=>value===255));
    }
    // +Z normal, maxRGB .2: quantized float32 shader hash = .746337890625.
    const mesh={positions:[0,0,0,1,0,0,0,1,0],indices:[0,1,2],triangleColors:[.2,.1,.05]};
    assert.deepEqual([...fruitPatternDescriptor(mesh,.15)],[0]);
    assert.deepEqual([...fruitPatternDescriptor(mesh,.55)],[255]);
    assert.deepEqual([...fruitPatternDescriptor(mesh,.25)],[116]);
});
