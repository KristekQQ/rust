import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, copyFile, readFile, rm } from 'node:fs/promises';
import { execFileSync, spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

test('pre-push propagates test/generator failure and does not destroy a previous source dump', async t => {
    const root = await mkdtemp(path.join(tmpdir(), 'rust-hook-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    await mkdir(path.join(root, 'scripts')); await mkdir(path.join(root, 'bin')); await mkdir(path.join(root, 'src'));
    execFileSync('git', ['init', '-q', root]);
    await copyFile(new URL('../scripts/pre-push', import.meta.url), path.join(root, 'scripts/pre-push'));
    await copyFile(new URL('../scripts/dump_sources.sh', import.meta.url), path.join(root, 'scripts/dump_sources.sh'));
    execFileSync('chmod', ['+x', path.join(root, 'scripts/dump_sources.sh')]);
    await writeFile(path.join(root, 'bin/cargo'), '#!/bin/sh\nexit "${TEST_CARGO_EXIT:-0}"\n', { mode: 0o755 });
    await writeFile(path.join(root, 'all_sources.txt'), 'previous valid dump');
    const run = exit => spawnSync('bash', ['scripts/pre-push'], { cwd: root, env: { ...process.env, PATH: `${root}/bin:${process.env.PATH}`, TEST_CARGO_EXIT: String(exit) } });
    assert.equal(run(37).status, 37, 'Cargo failure must block push');
    assert.notEqual(run(0).status, 0, 'Missing manifest must block push');
    assert.equal(await readFile(path.join(root, 'all_sources.txt'), 'utf8'), 'previous valid dump', 'Failed dump must be atomic');
    await writeFile(path.join(root, 'Cargo.toml'), '[package]\nname="fixture"\n');
    await writeFile(path.join(root, 'src/lib.rs'), 'pub fn verified() {}\n');
    assert.equal(run(0).status, 0, 'Successful check and dump allow push');
    assert.match(await readFile(path.join(root, 'all_sources.txt'), 'utf8'), /pub fn verified\(\)/);
});

test('browser verification rejects missing builds, changed sources and replaced WASM/bindings', async t => {
    const root = await mkdtemp(path.join(tmpdir(), 'rust-build-marker-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    for (const directory of ['src', 'scripts', 'pkg', '.cargo']) await mkdir(path.join(root, directory));
    await copyFile(new URL('../scripts/build-fingerprint.mjs', import.meta.url), path.join(root, 'scripts/build-fingerprint.mjs'));
    const files = ['src/lib.rs', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml', 'pkg/webgpu_wasm.js', 'pkg/webgpu_wasm_bg.wasm'];
    for (const file of files) await writeFile(path.join(root, file), `fixture: ${file}`);
    const { fingerprint, assertFreshBuild } = await import(pathToFileURL(path.join(root, 'scripts/build-fingerprint.mjs')));
    await assert.rejects(assertFreshBuild(), /No verified WASM build/);
    const expected = await fingerprint();
    await writeFile(path.join(root, 'pkg/verification-build.json'), JSON.stringify(expected));
    assert.deepEqual(await assertFreshBuild(), expected);
    for (const file of ['src/lib.rs', 'pkg/webgpu_wasm.js', 'pkg/webgpu_wasm_bg.wasm']) {
        await writeFile(path.join(root, file), 'replaced bytes');
        await assert.rejects(assertFreshBuild(), /changed since WASM build/);
        await writeFile(path.join(root, file), `fixture: ${file}`);
    }
    assert.deepEqual(await assertFreshBuild(), expected);
});

test('pixel assertions reject blank presentation and unchanged deformation', async () => {
    const source = await readFile(new URL('./pixels.js', import.meta.url), 'utf8');
    const { assertForeground, assertChanged } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
    const black = { width: 20, height: 20, data: new Uint8ClampedArray(20 * 20 * 4) };
    assert.throws(() => assertForeground(black, 'blank target'), /missing rendered geometry/);
    assert.throws(() => assertChanged(black, black, 'frozen pose'), /image did not change/);
    const geometry = { ...black, data: black.data.slice() };
    for (let i = 100; i < 300; i++) geometry.data[i * 4] = 200;
    assert.equal(assertForeground(geometry, 'visible target'), 200);
    assert.equal(assertChanged(black, geometry, 'deformed pose'), 200);
});
