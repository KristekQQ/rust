// Load a regular texture once. Rust uploads it; the GPU samples it each frame.
/** @param {import('../../js/engine.js').Engine} engine */
export async function loadPuddleMask(engine) {
    const image = new Image();
    image.src = new URL('./puddle-mask.svg', import.meta.url).href;
    await image.decode();
    const canvas = document.createElement('canvas');
    canvas.width = 256; canvas.height = 256;
    const context = canvas.getContext('2d');
    if (!context) throw new Error('Masku nelze načíst');
    context.drawImage(image, 0, 0, 256, 256);
    const rgba = context.getImageData(0, 0, 256, 256).data;
    const pixels = Uint8Array.from({ length: 256 * 256 }, (_, i) => rgba[i * 4]);
    engine.postprocessing.setReflectionMask({ width: 256, height: 256, pixels, center: [0, 0], size: [9, 9] });
}
