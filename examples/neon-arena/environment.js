const faces = ['px', 'nx', 'py', 'ny', 'pz', 'nz'];
/** Decode authored PNG assets once; all direction sampling and glow run in Rust/WGSL.
 * @param {string} layer */
async function loadFaces(layer) {
    const images = await Promise.all(faces.map(async face => {
        const image = new Image();
        image.src = new URL(`./assets/${layer}-${face}.png`, import.meta.url).href;
        await image.decode();
        return image;
    }));
    const size = images[0].naturalWidth;
    const pixels = new Uint8Array(size * size * 4 * 6);
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = size;
    const context = canvas.getContext('2d', {willReadFrequently: true});
    if (!context) throw new Error('Nelze dekódovat cubemapu');
    images.forEach((image, index) => {
        if (image.naturalWidth !== size || image.naturalHeight !== size) throw new Error('Stěny cubemapy musí mít shodný čtvercový rozměr');
        context.clearRect(0, 0, size, size);
        context.drawImage(image, 0, 0);
        pixels.set(context.getImageData(0, 0, size, size).data, index * size * size * 4);
    });
    return {size, pixels};
}
export async function loadEnvironment() {
    const [color, emission] = await Promise.all([loadFaces('color'), loadFaces('emission')]);
    if (color.size !== emission.size) throw new Error('Barevná a emisní cubemapa se rozměrově liší');
    return {size: color.size, color: color.pixels, emission: emission.pixels,
        projection:{center:/** @type {import('../../js/engine.d.ts').Vector3} */([0,1.65,0]),
            halfExtents:/** @type {import('../../js/engine.d.ts').Vector3} */([7,10.35,7])}};
}
