// Capture only the renderer canvas, after its frame, never text/UI around it.
/** @param {HTMLCanvasElement} canvas @returns {Promise<ImageData>} */
export async function capturePixels(canvas) {
    for (let i = 0; i < 4; i++) await new Promise(requestAnimationFrame);
    return new Promise((resolve, reject) => requestAnimationFrame(() => {
        try {
        const copy = document.createElement('canvas');
        copy.width = canvas.width; copy.height = canvas.height;
        const context = copy.getContext('2d', { willReadFrequently: true });
        if (!context) throw new Error('Canvas pixel capture unavailable');
        context.drawImage(canvas, 0, 0);
        resolve(context.getImageData(0, 0, copy.width, copy.height));
        } catch (error) { reject(error); }
    }));
}

/** @param {ImageData} image @param {string} label */
export function assertForeground(image, label, minimum = 100) {
    const data = image.data;
    let foreground = 0;
    for (let i = 0; i < data.length; i += 4) {
        if (Math.abs(data[i] - data[0]) + Math.abs(data[i + 1] - data[1]) + Math.abs(data[i + 2] - data[2]) > 20) foreground++;
    }
    if (foreground < minimum) throw new Error(`${label}: missing rendered geometry (${foreground} pixels)`);
    return foreground;
}

/** @param {ImageData} before @param {ImageData} after @param {string} label */
export function assertChanged(before, after, label, minimum = 50) {
    if (before.width !== after.width || before.height !== after.height) throw new Error(`${label}: capture dimensions differ`);
    let changed = 0;
    for (let i = 0; i < before.data.length; i += 4) {
        if (Math.abs(before.data[i] - after.data[i]) + Math.abs(before.data[i + 1] - after.data[i + 1]) + Math.abs(before.data[i + 2] - after.data[i + 2]) > 20) changed++;
    }
    if (changed < minimum) throw new Error(`${label}: image did not change (${changed} pixels)`);
    return changed;
}
