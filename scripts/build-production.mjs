import { createHash } from 'node:crypto';
import { lstat, mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { brotliCompressSync, constants, gzipSync } from 'node:zlib';
import { transform } from 'esbuild';
import { assertFreshBuild, root } from './build-fingerprint.mjs';

const runtimeExtensions = new Set(['.html', '.js', '.css', '.json', '.gltf', '.glb', '.bin', '.png', '.jpg', '.jpeg', '.webp', '.svg', '.woff', '.woff2', '.md', '.txt']);
const compressExtensions = new Set(['.html', '.js', '.css', '.json', '.gltf', '.svg', '.wasm']);
const extraFiles = ['index.html', 'tests/pixels.js',
    'pkg/webgpu_wasm.js', 'pkg/webgpu_wasm_bg.wasm', 'pkg/webgpu_wasm.d.ts', 'pkg/webgpu_wasm_bg.wasm.d.ts'];
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

/** Transform each module in place: relative imports and import.meta.url keep their original base URL. */
export async function minifyRuntimeFile(relativePath, bytes) {
    if (relativePath === 'examples/neon-arena/index.html') {
        return Buffer.from(bytes.toString('utf8').replace('<meta charset=', '<meta name="build-profile" content="release"><meta charset='));
    }
    if (relativePath === 'index.html') {
        // These development-only pages are intentionally absent from the isolated web root.
        return Buffer.from(bytes.toString('utf8').replace(/^[ \t]*<div class="help"><a href="tests\/[^"<>]+">[^<>]*<\/a><\/div>[ \t]*\r?\n/gm, ''));
    }
    const extension = path.extname(relativePath);
    if (extension !== '.js' && extension !== '.css') return bytes;
    const result = await transform(bytes.toString('utf8'), {
        loader: extension === '.js' ? 'js' : 'css',
        minify: true, target: 'es2022', legalComments: 'inline',
        ...(extension === '.js' ? { format: 'esm' } : {}),
    });
    return Buffer.from(result.code);
}

export function precompress(bytes) {
    return {
        gzip: gzipSync(bytes, { level: 9 }),
        brotli: brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }),
    };
}

async function runtimeFiles(sourceRoot, directory) {
    const files = [];
    for (const entry of await readdir(path.join(sourceRoot, directory), { withFileTypes: true })) {
        if (entry.name.startsWith('.')) continue;
        const relativePath = path.posix.join(directory, entry.name);
        if (entry.isSymbolicLink()) throw new Error(`Production input must not be a symlink: ${relativePath}`);
        if (entry.isDirectory()) files.push(...await runtimeFiles(sourceRoot, relativePath));
        else if (entry.isFile() && runtimeExtensions.has(path.extname(entry.name))) files.push(relativePath);
    }
    return files;
}

async function productionFiles(sourceRoot) {
    return [...new Set([...extraFiles, ...await runtimeFiles(sourceRoot, 'js'),
        ...await runtimeFiles(sourceRoot, 'examples'), ...await runtimeFiles(sourceRoot, 'assets')])].sort();
}

/** Assemble an isolated web root from an already checked build. Never writes into pkg/. */
export async function writeProduction({ sourceRoot = root, destination = path.join(root, 'output/production'), build }) {
    if (build?.configuration?.profile !== 'release') throw new Error('Production packaging requires a verified release WASM build.');
    const files = await productionFiles(sourceRoot);
    await mkdir(path.dirname(destination), { recursive: true });
    const staging = await mkdtemp(path.join(path.dirname(destination), '.production-'));
    const backup = `${staging}-previous`;
    const manifest = { version: 1, profile: 'release', build, files: [], totals: { files: 0, bytes: 0, sourceBytes: 0, gzipBytes: 0, brotliBytes: 0 } };
    try {
        for (const file of files) {
            if (!(await lstat(path.join(sourceRoot, file))).isFile()) throw new Error(`Production input must be a regular file: ${file}`);
            const source = await readFile(path.join(sourceRoot, file));
            const bytes = await minifyRuntimeFile(file, source);
            const target = path.join(staging, file);
            await mkdir(path.dirname(target), { recursive: true });
            await writeFile(target, bytes);
            const record = { path: file, sourceBytes: source.length, sourceSha256: sha256(source), bytes: bytes.length, sha256: sha256(bytes) };
            manifest.totals.sourceBytes += source.length;
            manifest.totals.bytes += bytes.length;
            if (compressExtensions.has(path.extname(file))) {
                const compressed = precompress(bytes);
                for (const [encoding, suffix] of [['gzip', 'gz'], ['brotli', 'br']]) {
                    await writeFile(`${target}.${suffix}`, compressed[encoding]);
                    record[encoding] = { bytes: compressed[encoding].length, sha256: sha256(compressed[encoding]) };
                    manifest.totals[`${encoding}Bytes`] += compressed[encoding].length;
                }
            }
            manifest.files.push(record);
        }
        manifest.totals.files = manifest.files.length;
        // Manifest describes payload files only, avoiding a self-referential checksum.
        const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`);
        const manifestPath = path.join(staging, 'production-manifest.json');
        await writeFile(manifestPath, manifestBytes);
        const manifestCompressed = precompress(manifestBytes);
        await writeFile(`${manifestPath}.gz`, manifestCompressed.gzip);
        await writeFile(`${manifestPath}.br`, manifestCompressed.brotli);
        let movedPrevious = false;
        try { await rename(destination, backup); movedPrevious = true; }
        catch (error) { if (error.code !== 'ENOENT') throw error; }
        try { await rename(staging, destination); }
        catch (error) { if (movedPrevious) await rename(backup, destination); throw error; }
        if (movedPrevious) await rm(backup, { recursive: true, force: true });
        return manifest;
    } finally {
        await rm(staging, { recursive: true, force: true });
    }
}

export async function buildProduction() {
    // Matches verify.sh; buildConfiguration normalizes duplicate unstable-API flags.
    process.env.VERIFY_WASM_PROFILE = 'release';
    const build = await assertFreshBuild();
    return writeProduction({ build });
}

/** Validate deployed bytes, optionally against the current verified release source/pkg fingerprint. */
export async function assertProductionBuild(directory, { checkCurrentSource = false } = {}) {
    const manifest = JSON.parse(await readFile(path.join(directory, 'production-manifest.json'), 'utf8'));
    if (manifest.version !== 1 || manifest.profile !== 'release' || manifest.build?.configuration?.profile !== 'release' || !Array.isArray(manifest.files)) {
        throw new Error('Invalid release production manifest. Run npm run build:production.');
    }
    const seen = new Set();
    for (const record of manifest.files) {
        if (typeof record.path !== 'string' || record.path.includes('\\') || path.posix.normalize(record.path) !== record.path || record.path.startsWith('/') || record.path.startsWith('../') || seen.has(record.path)) {
            throw new Error('Invalid production manifest path.');
        }
        seen.add(record.path);
        for (const [suffix, expected] of [['', record], ['.gz', record.gzip], ['.br', record.brotli]]) {
            if (!expected) continue;
            const bytes = await readFile(path.join(directory, record.path + suffix));
            if (bytes.length !== expected.bytes || sha256(bytes) !== expected.sha256) throw new Error(`Production payload changed: ${record.path}${suffix}`);
        }
    }
    for (const required of ['index.html', 'pkg/webgpu_wasm.js', 'pkg/webgpu_wasm_bg.wasm']) {
        if (!seen.has(required)) throw new Error(`Production manifest is missing ${required}`);
    }
    if (checkCurrentSource) {
        process.env.VERIFY_WASM_PROFILE = 'release';
        const current = await assertFreshBuild();
        if (JSON.stringify(current) !== JSON.stringify(manifest.build)) throw new Error('Production package is stale. Run npm run build:production.');
        const files = await productionFiles(root);
        if (JSON.stringify(files) !== JSON.stringify([...seen].sort())) throw new Error('Production source file set changed. Run npm run build:production.');
        for (const record of manifest.files) {
            if (!(await lstat(path.join(root, record.path))).isFile() || sha256(await readFile(path.join(root, record.path))) !== record.sourceSha256) {
                throw new Error(`Production source changed: ${record.path}. Run npm run build:production.`);
            }
        }
    }
    return manifest;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    try {
        const manifest = await buildProduction();
        console.log(`Production: output/production (${manifest.totals.files} files, ${manifest.totals.bytes} bytes; release WASM).`);
    } catch (error) {
        console.error(`Production build failed: ${error.message}`);
        process.exitCode = 1;
    }
}
