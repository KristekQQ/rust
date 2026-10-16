import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

export const root = fileURLToPath(new URL('../', import.meta.url));
export function wasmProfile(env = process.env) {
    const profile = env.VERIFY_WASM_PROFILE || 'dev';
    if (profile !== 'dev' && profile !== 'release') {
        throw new Error(`Invalid VERIFY_WASM_PROFILE: ${profile}. Expected dev or release.`);
    }
    return profile;
}
export function buildConfiguration(env = process.env) {
    const flags = (env.RUSTFLAGS || '').trim().split(/\s+/).filter(flag => flag && flag !== '--cfg=web_sys_unstable_apis');
    flags.push('--cfg=web_sys_unstable_apis');
    return {
        rustflags: flags.join(' '),
        cargo: Object.fromEntries(Object.entries(env).filter(([key]) =>
            /^(CARGO_ENCODED_RUSTFLAGS$|CARGO_BUILD_RUSTFLAGS$|CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS$|CARGO_PROFILE_(DEV|RELEASE)_)/.test(key)).sort()),
        rustc: execFileSync('rustc', ['-Vv'], { encoding: 'utf8' }),
        profile: wasmProfile(env),
    };
}
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
    return { source: hash.digest('hex'), wasm, bindings, configuration: buildConfiguration() };
}
export async function assertFreshBuild() {
    const current = await fingerprint();
    let recorded;
    try { recorded = JSON.parse(await readFile(path.join(root, 'pkg/verification-build.json'), 'utf8')); }
    catch { throw new Error('No verified WASM build. Run npm run verify:browser to build matching pkg/.'); }
    if (['source', 'wasm', 'bindings', 'configuration'].some(key => JSON.stringify(current[key]) !== JSON.stringify(recorded[key]))) {
        throw new Error('Source/pkg/build configuration changed since WASM build. Run npm run verify:browser.');
    }
    return current;
}
if (process.argv[1] === fileURLToPath(import.meta.url) && process.argv[2] === '--write') {
    await writeFile(path.join(root, 'pkg/verification-build.json'), JSON.stringify(await fingerprint(), null, 2) + '\n');
}
