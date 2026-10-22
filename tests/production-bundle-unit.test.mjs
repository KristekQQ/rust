import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { brotliDecompressSync, gunzipSync } from 'node:zlib';
import { assertProductionBuild, minifyRuntimeFile, precompress, writeProduction } from '../scripts/build-production.mjs';

test('module minification preserves imports, URL bases and public exports', async () => {
    const bytes = await minifyRuntimeFile('examples/demo/main.js', Buffer.from(`
        import { createScene } from './scene.js';
        export async function load() {
            const sdk = await import('../../js/engine.js?v=release');
            return [createScene, sdk, new URL('./assets/model.glb', import.meta.url).href];
        }
    `));
    const code = bytes.toString();
    for (const fragment of ['./scene.js', '../../js/engine.js?v=release', './assets/model.glb', 'import.meta.url']) assert.ok(code.includes(fragment), fragment);
    assert.match(code, /export\{.*load/);
    const html = Buffer.from('<pre> intentional  whitespace </pre>');
    assert.deepEqual(await minifyRuntimeFile('index.html', html), html);
    const index = Buffer.from('<div class="help"><a href="tests/render-effects.html">Dev test</a></div>\n<script type="module" src="js/app.js"></script>');
    assert.equal((await minifyRuntimeFile('index.html', index)).toString(), '<script type="module" src="js/app.js"></script>');
});

test('offline compression round-trips exact bytes', () => {
    const bytes = Buffer.from('module data \0 with UTF-8: čeština\n'.repeat(40));
    const { gzip, brotli } = precompress(bytes);
    assert.deepEqual(gunzipSync(gzip), bytes);
    assert.deepEqual(brotliDecompressSync(brotli), bytes);
});

test('isolated release package has manifest checksums and replaces previous output only on success', async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), 'production-bundle-'));
    const destination = path.join(directory, 'output/production');
    const build = { configuration: { profile: 'release' }, wasm: 'fixture' };
    const fixture = {
        'index.html': '<!doctype html><script type="module" src="js/app.js"></script>',
        'js/app.js': 'export const value = 1 + 2;',
        'examples/demo/main.js': 'import "../../js/app.js";',
        'examples/demo/build-private.mjs': 'private build helper',
        'assets/license.txt': 'Asset license',
        'tests/pixels.js': 'export const pixels = true;',
        'pkg/webgpu_wasm.js': 'export default function init() {}',
        'pkg/webgpu_wasm_bg.wasm': '\0asm',
        'pkg/webgpu_wasm.d.ts': 'export default function init(): void;',
        'pkg/webgpu_wasm_bg.wasm.d.ts': 'export const memory: WebAssembly.Memory;',
        'pkg/verification-build.json': 'private fingerprint',
        '.private-token': 'secret',
    };
    try {
        for (const [file, contents] of Object.entries(fixture)) {
            await mkdir(path.dirname(path.join(directory, file)), { recursive: true });
            await writeFile(path.join(directory, file), contents);
        }
        await mkdir(destination, { recursive: true });
        await writeFile(path.join(destination, 'obsolete.txt'), 'previous');
        const manifest = await writeProduction({ sourceRoot: directory, destination, build });
        assert.equal(manifest.profile, 'release');
        assert.equal(manifest.files.length, 9);
        assert.deepEqual(await assertProductionBuild(destination), manifest);
        assert.deepEqual(JSON.parse(await readFile(path.join(destination, 'production-manifest.json'))), manifest);
        for (const excluded of ['obsolete.txt', '.private-token', 'pkg/verification-build.json', 'examples/demo/build-private.mjs']) {
            await assert.rejects(readFile(path.join(destination, excluded)), { code: 'ENOENT' });
        }
        for (const record of manifest.files) {
            const bytes = await readFile(path.join(destination, record.path));
            assert.equal(createHash('sha256').update(bytes).digest('hex'), record.sha256);
            assert.equal(createHash('sha256').update(await readFile(path.join(directory, record.path))).digest('hex'), record.sourceSha256);
            if (record.gzip) assert.deepEqual(gunzipSync(await readFile(path.join(destination, `${record.path}.gz`))), bytes);
            if (record.brotli) assert.deepEqual(brotliDecompressSync(await readFile(path.join(destination, `${record.path}.br`))), bytes);
        }
        assert.equal(await readFile(path.join(directory, 'pkg/webgpu_wasm.js'), 'utf8'), fixture['pkg/webgpu_wasm.js']);
        await writeFile(path.join(directory, 'js/app.js'), 'export broken syntax');
        await assert.rejects(writeProduction({ sourceRoot: directory, destination, build }));
        assert.deepEqual(JSON.parse(await readFile(path.join(destination, 'production-manifest.json'))), manifest);
        await assert.rejects(writeProduction({ sourceRoot: directory, destination, build: { configuration: { profile: 'dev' } } }), /release/);
        await writeFile(path.join(directory, 'js/app.js'), fixture['js/app.js']);
        await rm(path.join(directory, 'pkg/webgpu_wasm.js'));
        await symlink(path.join(directory, '.private-token'), path.join(directory, 'pkg/webgpu_wasm.js'));
        await assert.rejects(writeProduction({ sourceRoot: directory, destination, build }), /regular file/);
        assert.deepEqual(JSON.parse(await readFile(path.join(destination, 'production-manifest.json'))), manifest);
        await writeFile(path.join(destination, 'js/app.js.gz'), 'corrupt');
        await assert.rejects(assertProductionBuild(destination), /Production payload changed/);
    } finally {
        await rm(directory, { recursive: true, force: true });
    }
});
