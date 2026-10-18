// Column-major rigid transforms, shared by the test platform and its unit tests.
export function matrixFromPose({position = [0, 0, 0], orientation = [0, 0, 0, 1]} = {}) {
    if (position.length !== 3 || orientation.length !== 4 || ![...position, ...orientation].every(Number.isFinite)) throw new TypeError('Invalid XR pose');
    const length = Math.hypot(...orientation);
    if (length < 1e-12) throw new TypeError('Invalid XR orientation');
    const [x,y,z,w] = orientation.map(value => value / length);
    return new Float32Array([
        1-2*(y*y+z*z), 2*(x*y+z*w), 2*(x*z-y*w), 0,
        2*(x*y-z*w), 1-2*(x*x+z*z), 2*(y*z+x*w), 0,
        2*(x*z+y*w), 2*(y*z-x*w), 1-2*(x*x+y*y), 0,
        ...position, 1,
    ]);
}
export function multiply(a, b) {
    const result = new Float32Array(16);
    for (let column=0; column<4; column++) for (let row=0; row<4; row++) {
        for (let k=0; k<4; k++) result[column*4+row] += a[k*4+row]*b[column*4+k];
    }
    return result;
}
export function inverse(matrix) {
    // Gauss-Jordan also handles valid non-orthogonal matrices supplied by tests.
    if (matrix.length !== 16 || !Array.from(matrix).every(Number.isFinite)) throw new TypeError('Invalid XR matrix');
    const rows = Array.from({length:4}, (_, r) => Array.from({length:8}, (_, c) => c<4 ? matrix[c*4+r] : Number(c-4===r)));
    for (let c=0; c<4; c++) {
        let pivot=c;
        for (let r=c+1; r<4; r++) if (Math.abs(rows[r][c])>Math.abs(rows[pivot][c])) pivot=r;
        if (Math.abs(rows[pivot][c])<1e-12) throw new TypeError('Singular XR matrix');
        [rows[c],rows[pivot]]=[rows[pivot],rows[c]];
        const divisor=rows[c][c]; rows[c]=rows[c].map(value=>value/divisor);
        for (let r=0;r<4;r++) if (r!==c) {
            const scale=rows[r][c]; rows[r]=rows[r].map((value,i)=>value-scale*rows[c][i]);
        }
    }
    return new Float32Array(Array.from({length:16}, (_,i)=>rows[i%4][4+Math.floor(i/4)]));
}
