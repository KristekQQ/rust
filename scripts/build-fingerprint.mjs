import { createHash } from 'node:crypto';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

export const root = fileURLToPath(new URL('../', import.meta.url));
async function sources(directory) {
    const entries = await readdir(path.join(root, directory), { withFileTypes: true });
    const files = await Promise.all(entries.map(entry => entry.isDirectory()
        ? sources(path.join(directory, entry.name))
        : /\.(rs|wgsl)$/.test(entry.name) ? [path.join(directory, entry.name)] : []));
    return files.flat();
}
export async function fingerprint() {
    const files = [...await sources('src'), 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml'].sort();
    const hash = createHash('sha256');
    for (const file of files) hash.update(file).update('\0').update(await readFile(path.join(root, file))).update('\0');
    const wasm = createHash('sha256').update(await readFile(path.join(root, 'pkg/webgpu_wasm_bg.wasm'))).digest('hex');
    const bindings = createHash('sha256').update(await readFile(path.join(root, 'pkg/webgpu_wasm.js'))).digest('hex');
    return { source: hash.digest('hex'), wasm, bindings };
}
export async function assertFreshBuild() {
    const current = await fingerprint();
    let recorded;
    try { recorded = JSON.parse(await readFile(path.join(root, 'pkg/verification-build.json'), 'utf8')); }
    catch { throw new Error('No verified WASM build. Run npm run verify:browser to build matching pkg/.'); }
    if (['source', 'wasm', 'bindings'].some(key => current[key] !== recorded[key])) {
        throw new Error('Source/pkg changed since WASM build. Run npm run verify:browser.');
    }
    return current;
}
if (process.argv[1] === fileURLToPath(import.meta.url) && process.argv[2] === '--write') {
    await writeFile(path.join(root, 'pkg/verification-build.json'), JSON.stringify(await fingerprint(), null, 2) + '\n');
}
