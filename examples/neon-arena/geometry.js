const BOARD_Y = 0.2;

/** Closed, origin-centered fruit meshes. Neon seams subdivide the actual
 * surface, so cutting sees closed solids rather than overlapping wire meshes.
 * The renderer discards dark centers; both sides of the luminous seams remain visible.
 * @returns {{name: string, geometry: import('../../js/engine.d.ts').TriangleMeshOptions, capColor: import('../../js/engine.d.ts').Vector3}[]} */
export function fruitGeometries() {
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
    for (const fruit of fruits) accentColors(fruit.name, fruit.geometry);
    return fruits;
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
    for (let i=0;i<mesh.indices.length;i+=3) {
        if (Math.max(colors[i],colors[i+1],colors[i+2])<.25) continue;
        let x=0,y=0;
        for(let j=0;j<3;j++) {const vertex=mesh.indices[i+j]*3;x+=mesh.positions[vertex]/3;y+=mesh.positions[vertex+1]/3;}
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
        if(color) {colors[i]=color[0];colors[i+1]=color[1];colors[i+2]=color[2];}
    }
    mesh.triangleColors=colors;
}

/** Tiny stalks use a complete emissive tube, avoiding six strips per face and subpixel holes.
 * @param {{positions:number[],indices:number[]}} mesh @param {number[]} color
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function solidNeon(mesh,color) {
    const triangleColors=[];
    for(let i=0;i<mesh.indices.length;i+=3)triangleColors.push(...color);
    return {...mesh,triangleColors,smooth:false};
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
        const top = 2 + (ringCount - 1) * segments;
        baseIndices.push(1, top + next, top + i);
        for (let ring = 0; ring < ringCount - 1; ring++) {
            const b = 2 + ring * segments + i, bn = 2 + ring * segments + next;
            baseIndices.push(b, b + segments, bn + segments, b, bn + segments, bn);
        }
    }
    return neonSurface({positions, indices: baseIndices}, neon, dark, striped);
}

/** Apply coplanar luminous seams without adding overlapping surface shells.
 * @param {{positions: number[], indices: number[]}} mesh
 * @param {number[]} neon @param {number[]} dark @param {boolean} [striped]
 * @returns {import('../../js/engine.d.ts').TriangleMeshOptions} */
function neonSurface(mesh, neon, dark, striped = false) {
    const positions = [...mesh.positions], baseIndices = mesh.indices;
    /** @type {number[]} */ const indices = [];
    /** @type {number[]} */ const triangleColors = [];
    for (let face = 0; face < baseIndices.length; face += 3) {
        const outer = baseIndices.slice(face, face + 3);
        const inner = positions.length / 3;
        const centroid = [0, 1, 2].map(axis => outer.reduce((sum, v) => sum + positions[v * 3 + axis], 0) / 3);
        for (const vertex of outer) {
            for (let axis = 0; axis < 3; axis++) {
                positions.push(centroid[axis] + (positions[vertex * 3 + axis] - centroid[axis]) * .88);
            }
        }
        // One dark center and three luminous, coplanar edge trapezoids.
        indices.push(inner, inner + 1, inner + 2);
        const stripe = striped && Math.floor(face / 42) % 2 === 0;
        triangleColors.push(...dark.map(value => value * (stripe ? 2.2 : 1)));
        for (let edge = 0; edge < 3; edge++) {
            const next = (edge + 1) % 3;
            indices.push(outer[edge], outer[next], inner + next,
                outer[edge], inner + next, inner + edge);
            const seam = stripe ? [.48,1,.08] : neon;
            triangleColors.push(...seam, ...seam);
        }
    }
    return {positions, indices, triangleColors, smooth: false};
}

/** A capped, planar curved tube; endpoint radii must be zero.
 * The ring frame follows the centerline tangent, keeping the banana curved in 3D.
 * @param {number[][]} centers @param {number[]} radii @param {number} segments
 * @returns {{positions: number[], indices: number[]}} */
function curvedSolid(centers, radii, segments) {
    const positions = [centers[0][0], centers[0][1], 0,
        centers[centers.length - 1][0], centers[centers.length - 1][1], 0];
    const indices = [];
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
        for (let ring = 0; ring < ringCount - 1; ring++) {
            const a = 2 + ring * segments + i, b = 2 + ring * segments + next;
            indices.push(a, a + segments, b + segments, a, b + segments, b);
        }
    }
    return {positions, indices};
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
