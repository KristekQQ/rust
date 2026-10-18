const BOARD_Y = 0.2;
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
