const BOARD_Y = 0.2;
export const FRUIT_PATTERN_COUNT = 15;
const FRUIT_PATTERN_CANDIDATES = 40;

/** Authored sparse/dense coverage levels. @param {number} seed */
export function fruitPatternCoverage(seed) {
    return [.35,1,.55,.90,.70,.98,.80,.45][seed%8];
}

/** Compact filled-panel mask matching fs_translucent's quantized facet key.
 * One byte per polygon (both center triangles of a quad count once); seams and
 * solid stalks are excluded. Float32 operations follow the shader hash; GPU
 * contraction/normal rounding can still differ slightly at soft thresholds.
 * @param {import('../../js/engine.d.ts').TriangleMeshOptions} mesh
 * @param {number} coverage
 * @returns {Uint8Array} */
export function fruitPatternDescriptor(mesh, coverage) {
    const f = Math.fround;
    /** @param {number} x */
    const fract = x => f(x - Math.floor(x));
    const colors = mesh.triangleColors ?? [];
    const weights = [];
    for (let i = 0; i < mesh.indices.length;) {
        if (Math.max(colors[i], colors[i+1], colors[i+2]) >= .25) { i += 3; continue; }
        const start = i;
        while (i < colors.length && Math.max(colors[i], colors[i+1], colors[i+2]) < .25) i += 3;
        const centerTriangles = (i-start)/3;
        const [a,b,c] = Array.from(mesh.indices.slice(start,start+3), vertex =>
            Array.from(mesh.positions.slice(vertex*3,vertex*3+3), f));
        const u=b.map((v,j)=>f(v-a[j])), v=c.map((value,j)=>f(value-a[j]));
        const cross=[f(f(u[1]*v[2])-f(u[2]*v[1])),f(f(u[2]*v[0])-f(u[0]*v[2])),f(f(u[0]*v[1])-f(u[1]*v[0]))];
        const inverse=f(1/f(Math.sqrt(f(f(f(cross[0]*cross[0])+f(cross[1]*cross[1]))+f(cross[2]*cross[2])))));
        const normal=cross.map(n=>Math.round(f(f(n*inverse)*4096))/4096);
        const brightness=Math.round(f(f(Math.max(colors[start],colors[start+1],colors[start+2]))*65536))/65536;
        const seed=normal.map((n,j)=>fract(f(f(f(n*f([.1031,.1030,.0973][j]))+f(brightness*f(17.37)))+f([.37,.61,.83][j]))));
        const products=seed.map((n,j)=>f(n*f(seed[(j+1)%3]+f(33.33))));
        const dot=f(f(products[0]+products[1])+products[2]);
        const mixed=seed.map(n=>f(n+dot));
        const hash=fract(f(f(mixed[0]+mixed[1])*mixed[2]));
        const threshold=f(1-coverage), low=f(threshold-f(.06)), high=f(threshold+f(.06));
        const t=Math.max(0,Math.min(1,f(f(hash-low)/f(high-low))));
        const weight=coverage===1?1:coverage===0?0:f(f(t*t)*f(3-f(2*t)));
        weights.push(Math.round(weight*255));
        i += 6*(centerTriangles+2);
    }
    return Uint8Array.from(weights);
}

/** Mean absolute difference of actual polygon fill, rather than seed/RGB.
 * @param {Uint8Array} a @param {Uint8Array} b */
function patternDistance(a,b) {
    return a.reduce((sum,value,i)=>sum+Math.abs(value-b[i]),0)/(255*a.length);
}

/** Retain base, fullest and sparsest, then maximize distance to the nearest
 * selected mask. Near-identical dense candidates lose to distinct panel masks.
 * Order by farthest next neighbor, then improve cyclic adjacency with bounded
 * segment reversals. Index zero remains the original base/gallery object.
 * @param {ReturnType<typeof fruitGeometries>} candidates */
function selectFruitPatterns(candidates) {
    const masks=candidates.map(fruit=>fruitPatternDescriptor(fruit.geometry,fruit.coverage ?? 1));
    const distances=masks.map(a=>masks.map(b=>patternDistance(a,b)));
    const filled=masks.map(mask=>mask.reduce((sum,value)=>sum+value,0));
    const selected=[0];
    for(const value of [Math.max(...filled),Math.min(...filled)]) {
        const index=filled.indexOf(value);
        if(!selected.includes(index)) selected.push(index);
    }
    while(selected.length<FRUIT_PATTERN_COUNT) {
        let best=-1,score=-1;
        for(let i=0;i<candidates.length;i++)if(!selected.includes(i)) {
            const nearest=Math.min(...selected.map(j=>distances[i][j]));
            if(nearest>score){best=i;score=nearest;}
        }
        selected.push(best);
    }
    const order=[0], remaining=selected.filter(i=>i!==0);
    while(remaining.length) {
        let best=0;
        for(let i=1;i<remaining.length;i++)if(distances[order.at(-1) ?? 0][remaining[i]]>distances[order.at(-1) ?? 0][remaining[best]])best=i;
        order.push(remaining.splice(best,1)[0]);
    }
    /** Prefer improving the weakest cyclic transition, then total distance.
     * @param {number[]} cycle */
    const score=cycle=>{
        const edges=cycle.map((value,i)=>distances[value][cycle[(i+1)%cycle.length]]);
        return Math.min(...edges)*1000+edges.reduce((sum,d)=>sum+d,0);
    };
    for(let pass=0;pass<8;pass++) {
        let best=order, bestScore=score(order);
        for(let a=1;a<order.length-1;a++)for(let b=a+1;b<order.length;b++) {
            const trial=[...order.slice(0,a),...order.slice(a,b+1).reverse(),...order.slice(b+1)];
            const trialScore=score(trial);
            if(trialScore>bestScore+1e-9){best=trial;bestScore=trialScore;}
        }
        if(best===order)break;
        order.splice(0,order.length,...best);
    }
    return order.map(i=>candidates[i]);
}

/** Fifteen diverse static panel patterns per kind, selected once before VR
 * from forty candidates. Variants share immutable positions/indices.
 * @returns {{fruits:ReturnType<typeof fruitGeometries>,wild:ReturnType<typeof wildSymbol>,variants:Record<string,ReturnType<typeof fruitGeometries>>}} */
export function createFruitPatternCatalog() {
    const fruits=fruitGeometries(),wild=wildSymbol();
    for(const fruit of [...fruits,wild])fruit.coverage=fruitPatternCoverage(0);
    /** @type {Record<string,ReturnType<typeof fruitGeometries>>} */
    const variants=Object.fromEntries([...fruits,wild].map(fruit=>[fruit.name,[fruit]]));
    for(let seed=1;seed<FRUIT_PATTERN_CANDIDATES;seed++)for(const fruit of [...fruitGeometries({seed}),wildSymbol({seed})]){
        const base=variants[fruit.name][0];
        fruit.geometry.positions=base.geometry.positions;
        fruit.geometry.indices=base.geometry.indices;
        fruit.coverage=fruitPatternCoverage(seed);
        variants[fruit.name].push(fruit);
    }
    for(const name of Object.keys(variants))variants[name]=selectFruitPatterns(variants[name]);
    return {fruits,wild,variants};
}

/** Closed, origin-centered fruit meshes. Neon seams subdivide the actual
 * surface, so cutting sees closed solids rather than overlapping wire meshes.
 * Colored facets stay below the optional .25 cutout threshold. The same mesh
 * supports solid neon fruit and the original open-seam presentation.
 * @param {{seed?: number}} [options]
 * @returns {{name: string, geometry: import('../../js/engine.d.ts').TriangleMeshOptions, capColor: import('../../js/engine.d.ts').Vector3,coverage?:number}[]} */
export function fruitGeometries({seed = 0} = {}) {
    /** @type {ReturnType<typeof fruitGeometries>} */
    const fruits = [
        {
            name: 'lemon',
            geometry: neonFruit([[-1.25,0],[-1.08,.30],[-.78,.70],[-.40,.91],[0,.98],[.40,.91],[.78,.70],[1.08,.30],[1.25,0]],
                [1,.88,.06], [.075,.052,.006], [1,1,.90]),
            capColor: [1,.84,.19],
        },
        {
            name: 'pear',
            geometry: neonFruit([[-1.08,0],[-.91,.47],[-.65,.84],[-.28,.96],[.12,.83],[.49,.57],[.82,.40],[1.08,.32],[1.29,0]],
                [.40,1,.08], [.025,.065,.008], [1,1,1]),
            capColor: [.88,1,.48],
        },
        {
            name: 'watermelon',
            geometry: neonFruit([[-.89,0],[-.82,.38],[-.63,.71],[-.34,.92],[0,1],[.34,.92],[.63,.71],[.82,.38],[.89,0]],
                [.03,1,.50], [.005,.055,.025], [1.28,1,.87], true),
            capColor: [1,.075,.18],
        },
        {
            name: 'plum',
            geometry: neonFruit([[-1.10,0],[-.97,.40],[-.72,.72],[-.38,.90],[0,.95],[.38,.90],[.72,.70],[.97,.36],[1.10,0]],
                [.74,.16,1], [.035,.006,.075], [.91,1,1]),
            capColor: [1,.62,.17],
        },
        {
            name: 'cherry',
            geometry: mergeMeshes([
                berry([-.49,-.32,0], [.46,.49,.43], [1,.035,.13], 8, 4),
                berry([.49,-.32,0], [.46,.49,.43], [1,.035,.13], 8, 4),
                solidNeon(curvedSolid([[-.39,.20],[-.36,.42],[-.25,.65],[-.15,.83],[-.12,.99]],
                    [0,.030,.031,.026,0], 5), [.06,1,.12]),
                solidNeon(curvedSolid([[.39,.20],[.37,.44],[.28,.68],[.17,.88],[.12,1.04]],
                    [0,.030,.031,.026,0], 5), [.06,1,.12]),
            ]),
            capColor: [1,.16,.25],
        },
        {
            name: 'lime',
            geometry: neonFruit([[-1.04,0],[-.91,.38],[-.67,.73],[-.34,.91],[0,.97],[.34,.91],[.67,.73],[.91,.38],[1.04,0]],
                [.38,1,.035], [.022,.067,.003], [1,1,.94]),
            capColor: [.75,1,.24],
        },
        {
            name: 'banana',
            geometry: bananaGeometry(),
            capColor: [1,.93,.52],
        },
        {
            name: 'orange',
            geometry: neonFruit([[-.97,0],[-.90,.38],[-.69,.69],[-.37,.90],[0,.98],[.37,.90],[.69,.69],[.90,.38],[.97,0]],
                [1,.36,.025], [.075,.018,.002], [1,1,1]),
            capColor: [1,.65,.12],
        },
        {
            name: 'grapes',
            geometry: mergeMeshes([
                [-.60,.51,0], [0,.55,.10], [.60,.51,0],
                [-.31,-.08,0], [.31,-.08,0], [0,-.66,0],
            ].map((center, i) => berry(center, [.285,.285,.285],
                i % 2 ? [1,.16,.72] : [.65,.15,1], 6, 3))),
            capColor: [.91,.53,1],
        },
    ];
    for (const fruit of fruits) {
        accentColors(fruit.name, fruit.geometry);
        seedFacetShades(fruit.geometry, seed);
    }
    return fruits;
}

/** Closed three-pronged jester crown. Its two-sided volume can be genuinely sliced.
 * @param {{seed?: number}} [options]
 * @returns {ReturnType<typeof fruitGeometries>[number]} */
export function wildSymbol({seed = 0} = {}) {
    const outline=[[-.8,-.65],[.8,-.65],[.95,.9],[.34,.45],[0,1.15],[-.34,.45],[-.95,.9]];
    const positions=[0,0,.30,0,0,-.30,...outline.flatMap(([x,y])=>[x,y,0])];
    const indices=[];
    for(let i=0;i<outline.length;i++){
        const a=2+i,b=2+(i+1)%outline.length;
        indices.push(0,a,b,1,b,a);
    }
    const geometry=neonSurface({positions,indices},[1,.75,.08],[.045,.008,.055]);
    const colors=Array.from(geometry.triangleColors??[]);
    for(let i=0;i<colors.length;i+=3)if(Math.max(...colors.slice(i,i+3))>.25 && i%12===0)
        colors.splice(i,3,1,.05,.8);
    geometry.triangleColors=colors;
    seedFacetShades(geometry, seed);
    return {name:'wild',geometry,capColor:[1,.35,.72]};
}

// Spatial palettes are baked once into the mesh; no runtime recoloring, textures or extra draws.
const PALETTE = {
    red:[1,.025,.09], pink:[1,.08,.36], green:[.035,1,.12], lime:[.35,1,.025],
    yellow:[1,.82,.025], gold:[1,.52,.015], orange:[1,.23,.012],
    cyan:[.025,.85,1], violet:[.57,.04,1], emerald:[.015,1,.36],
};

/** @param {string} name @param {import('../../js/engine.d.ts').TriangleMeshOptions} mesh */
function accentColors(name, mesh) {
    const colors = Array.from(mesh.triangleColors ?? []);
    for (let i=0;i<mesh.indices.length;) {
        const start=i;
        let centerEnd=i+3,end=i+3;
        if(Math.max(colors[i],colors[i+1],colors[i+2])<.25){
            // A polygon is authored as its center fan then two seam triangles
            // per boundary edge. Choose ONE palette at the whole center's
            // centroid, never separately for the GPU triangles of a quad.
            centerEnd=i;
            while(centerEnd<colors.length&&Math.max(colors[centerEnd],colors[centerEnd+1],colors[centerEnd+2])<.25)centerEnd+=3;
            const centerTriangles=(centerEnd-start)/3;
            end=centerEnd+6*(centerTriangles+2);
        }
        const vertices=[...new Set(Array.from(mesh.indices.slice(start,centerEnd)))];
        let x=0,y=0;
        for(const vertex of vertices){x+=mesh.positions[vertex*3]/vertices.length;y+=mesh.positions[vertex*3+1]/vertices.length;}
        let color;
        switch(name) {
            case 'cherry': color=y>.18?PALETTE.green:x>0?PALETTE.pink:PALETTE.red;break;
            case 'lemon': color=y>.55?PALETTE.lime:PALETTE.yellow;break;
            case 'pear': color=y<-.25?PALETTE.gold:PALETTE.lime;break;
            case 'watermelon': color=Math.abs(x)<.23?PALETTE.pink:PALETTE.emerald;break;
            case 'plum': color=y>.4?PALETTE.cyan:PALETTE.violet;break;
            case 'lime': color=y>.4?PALETTE.yellow:PALETTE.green;break;
            case 'banana': color=Math.abs(x)>.72?PALETTE.green:y<-.08?PALETTE.gold:PALETTE.yellow;break;
            case 'orange': color=y>.5?PALETTE.green:PALETTE.orange;break;
        }
        for(let triangle=start;triangle<end;triangle+=3)if(color) {
            // Preserve the baked facet key across both center triangles of a quad.
            const shade=Math.max(colors[triangle],colors[triangle+1],colors[triangle+2]);
            for(let c=0;c<3;c++)colors[triangle+c]=shade>=.25?color[c]*.80+.20:color[c]*shade;
        }
        i=end;
    }
    mesh.triangleColors=colors;
}

/** Tiny stalks use a complete emissive tube, avoiding six strips per face and subpixel holes.
 * @param {{positions:number[],indices:number[]}} mesh @param {number[]} color
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function solidNeon(mesh,color) {
    const triangleColors=[];
    for(let i=0;i<mesh.indices.length;i+=3)triangleColors.push(...color);
    return {positions:mesh.positions,indices:mesh.indices,triangleColors,smooth:false};
}

/** @param {number[][]} profile Bottom pole, seven latitude rings, top pole.
 * @param {number[]} neon @param {number[]} dark @param {number[]} scale
 * @param {boolean} [striped]
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function neonFruit(profile, neon, dark, scale, striped = false) {
    const segments = 8;
    // Retain the poles, widest waist and neck; omit redundant intermediate latitudes.
    profile=profile.filter((_,i)=>i!==2&&i!==5);
    const centerY = (profile[0][0] + profile[profile.length - 1][0]) / 2;
    const positions = [0, (profile[0][0] - centerY) * scale[1], 0,
        0, (profile[profile.length - 1][0] - centerY) * scale[1], 0];
    /** @type {number[]} */ const baseIndices = [];
    const faces = [];
    for (let ring = 1; ring < profile.length - 1; ring++) {
        const [y, radius] = profile[ring];
        for (let i = 0; i < segments; i++) {
            const phi = i / segments * Math.PI * 2;
            positions.push(Math.cos(phi) * radius * scale[0],
                (y - centerY) * scale[1], Math.sin(phi) * radius * scale[2]);
        }
    }
    const ringCount = profile.length - 2;
    for (let i = 0; i < segments; i++) {
        const next = (i + 1) % segments;
        baseIndices.push(0, 2 + i, 2 + next);
        faces.push([0, 2 + i, 2 + next]);
        const top = 2 + (ringCount - 1) * segments;
        baseIndices.push(1, top + next, top + i);
        faces.push([1, top + next, top + i]);
        for (let ring = 0; ring < ringCount - 1; ring++) {
            const b = 2 + ring * segments + i, bn = 2 + ring * segments + next;
            baseIndices.push(b, b + segments, bn + segments, b, bn + segments, bn);
            faces.push([b, b + segments, bn + segments, bn]);
        }
    }
    return neonSurface({positions, indices: baseIndices, faces}, neon, dark, striped);
}

/** Bake seeded per-facet brightness into maxRGB, which the shader uses as a
 * stable opacity key. Center triangles of one polygon retain the same key.
 * @param {import('../../js/engine.d.ts').TriangleMeshOptions} mesh @param {number} seed */
function seedFacetShades(mesh, seed) {
    const colors = Array.from(mesh.triangleColors ?? []);
    for (let i = 0; i < colors.length; i += 3) {
        const brightness = Math.max(colors[i], colors[i + 1], colors[i + 2]);
        if (brightness >= .25) continue;
        const shade = .15 + .08 * facetRandom(Math.round(brightness * 1e9) ^ (seed | 0));
        for (let axis = 0; axis < 3; axis++) colors[i + axis] *= shade / brightness;
    }
    mesh.triangleColors = colors;
}

/** Integer hash, evaluated only while generating a mesh. @param {number} value */
function facetRandom(value) {
    value = Math.imul(value ^ (value >>> 16), 0x21f0aaad);
    value = Math.imul(value ^ (value >>> 15), 0x735a2d97);
    return ((value ^ (value >>> 15)) >>> 0) / 4294967296;
}

/** Apply coplanar luminous polygon outlines without overlapping surface shells.
 * A quad has two equally shaded center triangles and no luminous diagonal.
 * @param {{positions: number[], indices: number[], faces?: number[][]}} mesh
 * @param {number[]} neon @param {number[]} dark @param {boolean} [striped]
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function neonSurface(mesh, neon, dark, striped = false) {
    const positions = [...mesh.positions];
    const faces = mesh.faces ?? Array.from({length: mesh.indices.length / 3},
        (_, face) => mesh.indices.slice(face * 3, face * 3 + 3));
    /** @type {number[]} */ const indices = [];
    /** @type {number[]} */ const triangleColors = [];
    for (let face = 0; face < faces.length; face++) {
        const outer = faces[face];
        const inner = positions.length / 3;
        const centroid = [0, 1, 2].map(axis => outer.reduce((sum, v) => sum + positions[v * 3 + axis], 0) / outer.length);
        for (const vertex of outer) {
            for (let axis = 0; axis < 3; axis++) {
                positions.push(centroid[axis] + (positions[vertex * 3 + axis] - centroid[axis]) * .88);
            }
        }
        const stripe = striped && Math.floor(face / 14) % 2 === 0;
        const shade = .15 + .08 * facetRandom(face + 1);
        for (let vertex = 1; vertex < outer.length - 1; vertex++) {
            indices.push(inner, inner + vertex, inner + vertex + 1);
            triangleColors.push(...neon.map(value => value * shade));
        }
        for (let edge = 0; edge < outer.length; edge++) {
            const next = (edge + 1) % outer.length;
            indices.push(outer[edge], outer[next], inner + next,
                outer[edge], inner + next, inner + edge);
            const seam = stripe ? [.48,1,.08] : neon;
            triangleColors.push(...seam, ...seam);
        }
    }
    return {positions, indices, triangleColors, smooth: false};
}

/** A curved tube cell can stay a quad only if all four vertices are coplanar.
 * @param {number[]} positions @param {number[]} face */
function planarQuad(positions, face) {
    /** @param {number} vertex */
    const point = vertex => positions.slice(vertex * 3, vertex * 3 + 3);
    const [a, b, c, d] = face.map(point);
    const ab = b.map((v, i) => v - a[i]), ac = c.map((v, i) => v - a[i]);
    const normal = [ab[1]*ac[2]-ab[2]*ac[1], ab[2]*ac[0]-ab[0]*ac[2], ab[0]*ac[1]-ab[1]*ac[0]];
    const length = Math.hypot(...normal);
    return length > 1e-12 && Math.abs(normal.reduce((sum, v, i) => sum + v * (d[i] - a[i]), 0)) <= length * 1e-9;
}

/** A capped, planar curved tube; endpoint radii must be zero.
 * The ring frame follows the centerline tangent, keeping the banana curved in 3D.
 * @param {number[][]} centers @param {number[]} radii @param {number} segments
 * @returns {{positions: number[], indices: number[], faces: number[][]}} */
function curvedSolid(centers, radii, segments) {
    const positions = [centers[0][0], centers[0][1], 0,
        centers[centers.length - 1][0], centers[centers.length - 1][1], 0];
    const indices = [], faces = [];
    for (let ring = 1; ring < centers.length - 1; ring++) {
        const dx = centers[ring + 1][0] - centers[ring - 1][0];
        const dy = centers[ring + 1][1] - centers[ring - 1][1];
        const length = Math.hypot(dx, dy);
        for (let i = 0; i < segments; i++) {
            const phi = i / segments * Math.PI * 2;
            const radial = Math.cos(phi) * radii[ring];
            positions.push(centers[ring][0] + dy / length * radial,
                centers[ring][1] - dx / length * radial, Math.sin(phi) * radii[ring]);
        }
    }
    const ringCount = centers.length - 2;
    for (let i = 0; i < segments; i++) {
        const next = (i + 1) % segments;
        const top = 2 + (ringCount - 1) * segments;
        indices.push(0, 2 + i, 2 + next, 1, top + next, top + i);
        faces.push([0, 2 + i, 2 + next], [1, top + next, top + i]);
        for (let ring = 0; ring < ringCount - 1; ring++) {
            const a = 2 + ring * segments + i, b = 2 + ring * segments + next;
            indices.push(a, a + segments, b + segments, a, b + segments, b);
            const quad = [a, a + segments, b + segments, b];
            if (planarQuad(positions, quad)) faces.push(quad);
            else faces.push([a, a + segments, b + segments], [a, b + segments, b]);
        }
    }
    return {positions, indices, faces};
}

/** @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function bananaGeometry() {
    const centers = [], radii = [];
    for (let i = 0; i <= 8; i++) {
        const u = i / 8, angle = (u - .5) * 3;
        centers.push([Math.sin(angle) * .99, .477 - Math.cos(angle) * .69]);
        radii.push(i === 0 || i === 8 ? 0 : .215 * Math.sin(u * Math.PI) ** .65);
    }
    return neonSurface(curvedSolid(centers, radii, 8), [1,.89,.035], [.074,.055,.002]);
}

/** @param {number[]} center @param {number[]} scale @param {number[]} neon
 * @param {number} segments @param {number} rings
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function berry(center, scale, neon, segments, rings) {
    const centers = [], radii = [];
    for (let ring = 0; ring <= rings; ring++) {
        const angle = ring / rings * Math.PI;
        centers.push([0, -Math.cos(angle)]);
        radii.push(ring === 0 || ring === rings ? 0 : Math.sin(angle));
    }
    const mesh = curvedSolid(centers, radii, segments);
    mesh.positions = mesh.positions.map((value, i) => value * scale[i % 3] + center[i % 3]);
    return neonSurface(mesh, neon, [.045,.004,.035]);
}

/** @param {import('../../js/engine.d.ts').TriangleMeshOptions[]} meshes
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function mergeMeshes(meshes) {
    const positions = [], indices = [], triangleColors = [];
    for (const mesh of meshes) {
        const offset = positions.length / 3;
        positions.push(...mesh.positions);
        indices.push(...Array.from(mesh.indices, index => index + offset));
        triangleColors.push(...(mesh.triangleColors ?? []));
    }
    return {positions, indices, triangleColors, smooth: false};
}

/** Lobed shoulders and a sunken crown, rather than a scaled sphere.
 * @param {number} [segments] @param {number} [rings]
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
export function appleGeometry(segments=32, rings=18) {
    const positions = [0, BOARD_Y, 0, 0, BOARD_Y + 1.92, 0];
    const indices = [];
    for (let j = 1; j < rings; j++) {
        const theta = j / rings * Math.PI;
        for (let i = 0; i < segments; i++) {
            const phi = i / segments * Math.PI * 2;
            const radius = Math.sin(theta) * 1.1 * (1 - .1 * Math.cos(theta)) * (1 + .045 * Math.cos(phi * 5));
            const y = BOARD_Y + 1.1 * (1 - Math.cos(theta)) - .28 * Math.exp(-(((Math.PI-theta)/.38) ** 2));
            positions.push(Math.cos(phi)*radius, y, Math.sin(phi)*radius);
        }
    }
    for (let i = 0; i < segments; i++) {
        const next = (i + 1) % segments;
        indices.push(0, 2+i, 2+next);
        const top = 2 + (rings-2)*segments;
        indices.push(1, top+next, top+i);
        for (let j = 0; j < rings-2; j++) {
            const b = 2+j*segments+i, bn = 2+j*segments+next, t = b+segments, tn = bn+segments;
            indices.push(b,t,tn,b,tn,bn);
        }
    }
    return {positions, indices, color: [.72, .022, .014], smooth: true};
}


/** Grip-space sword: -Z runs through the thumb, +X is the back of the right hand.
 * The broad blade lies in YZ, so its flat normal follows the back of the hand.
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
export function swordGeometry() {
    /** @type {number[]} */ const positions = [];
    /** @type {number[]} */ const indices = [];
    /** @type {number[]} */ const triangleColors = [];
    /** @param {number[]} size @param {number[]} center @param {number[]} color */
    const box = (size, center, color) => {
        const corners = [-1,-1,-1,1,-1,-1,1,1,-1,-1,1,-1,-1,-1,1,1,-1,1,1,1,1,-1,1,1];
        const faces = [0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5];
        const base = positions.length / 3;
        // Roll the authored mesh 90 degrees about its handle axis. Cutter endpoints
        // remain on -Z; render geometry and collision therefore stay aligned.
        for (let i=0; i<corners.length; i+=3) {
            const x = corners[i] * size[0] / 2 + center[0];
            const y = corners[i+1] * size[1] / 2 + center[1];
            const z = corners[i+2] * size[2] / 2 + center[2];
            positions.push(-y, x, z);
        }
        indices.push(...faces.map(i => i+base));
        for (let i=0; i<faces.length; i+=3) triangleColors.push(...color);
    };
    box([0.045,0.008,0.85],[0,0,-0.525],[0.72,0.82,0.95]);
    // Gold seam identifies the single sharp edge (after the grip roll: local -Y).
    box([0.002,0.0085,0.85],[-0.0215,0,-0.525],[1,.68,.12]);
    box([0.18,0.025,0.025],[0,0,-0.095],[0.7,0.43,0.09]);
    box([0.035,0.035,0.15],[0,0,0],[0.14,0.07,0.03]);
    return {positions, indices, triangleColors};
}

/** Closed rounded rectangle swept with a circular neon tube, in physical metres.
 * Bright front bands supply continuous glow boundaries without polygon ribs.
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
export function neonFrameGeometry() {
    const positions=[],indices=[],triangleColors=[];
    const half=.15,bend=.035,tube=.006,sides=24;
    const centers=[[half-bend,half-bend],[-half+bend,half-bend],[-half+bend,-half+bend],[half-bend,-half+bend]];
    const points=[];
    for(let corner=0;corner<4;corner++)for(let step=0;step<=8;step++){
        const angle=(corner+step/8)*Math.PI/2;
        points.push([centers[corner][0]+bend*Math.cos(angle),centers[corner][1]+bend*Math.sin(angle),Math.cos(angle),Math.sin(angle)]);
    }
    for(const [x,y,nx,ny] of points)for(let side=0;side<sides;side++){
        const angle=side/sides*Math.PI*2;
        positions.push(x+nx*tube*Math.cos(angle),y+ny*tube*Math.cos(angle),tube*Math.sin(angle));
    }
    for(let ring=0;ring<points.length;ring++)for(let side=0;side<sides;side++){
        const a=ring*sides+side,b=((ring+1)%points.length)*sides+side;
        const c=((ring+1)%points.length)*sides+(side+1)%sides,d=ring*sides+(side+1)%sides;
        indices.push(a,b,c,a,c,d);
        const front=Math.max(0,Math.sin((side+.5)/sides*Math.PI*2))**12;
        const color=[.003+.32*front,.035+.52*front,.20+.80*front];
        triangleColors.push(...color,...color);
    }
    return {positions,indices,triangleColors,smooth:true};
}
