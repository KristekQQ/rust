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
    await writeFile(path.join(root, 'bin/npm'), '#!/bin/sh\nexit "${TEST_VERIFY_EXIT:-0}"\n', { mode: 0o755 });
    await writeFile(path.join(root, 'all_sources.txt'), 'previous valid dump');
    const run = exit => spawnSync('bash', ['scripts/pre-push'], { cwd: root, env: { ...process.env, PATH: `${root}/bin:${process.env.PATH}`, TEST_VERIFY_EXIT: String(exit) } });
    assert.equal(run(37).status, 37, 'Full verification failure must block push');
    assert.notEqual(run(0).status, 0, 'Missing manifest must block push');
    assert.equal(await readFile(path.join(root, 'all_sources.txt'), 'utf8'), 'previous valid dump', 'Failed dump must be atomic');
    await writeFile(path.join(root, 'Cargo.toml'), '[package]\nname="fixture"\n');
    await writeFile(path.join(root, 'src/lib.rs'), 'pub fn verified() {}\n');
    assert.equal(run(0).status, 0, 'Successful check and dump allow push');
    assert.match(await readFile(path.join(root, 'all_sources.txt'), 'utf8'), /pub fn verified\(\)/);
});

test('Chromium sandbox exception is limited to explicit disposable Linux CI', async () => {
    const { browserArgs } = await import('../scripts/browser-options.mjs');
    const noSandbox = options => browserArgs({ uid: 1001, env: {}, ...options }).includes('--no-sandbox');
    assert.equal(noSandbox({ platform: 'linux' }), false);
    assert.equal(noSandbox({ platform: 'linux', env: { GITHUB_ACTIONS: 'true' } }), false);
    assert.equal(noSandbox({ platform: 'linux', env: { BROWSER_CI_NO_SANDBOX: '1' } }), false);
    assert.equal(noSandbox({ platform: 'linux', env: { GITHUB_ACTIONS: 'true', BROWSER_CI_NO_SANDBOX: '1' } }), true);
    assert.equal(noSandbox({ platform: 'darwin', env: { GITHUB_ACTIONS: 'true', BROWSER_CI_NO_SANDBOX: '1' } }), false);
    assert.equal(noSandbox({ platform: 'linux', uid: 0 }), false);
    assert.ok(browserArgs({ software: true }).includes('--use-angle=swiftshader'));
});

test('verification receipt survives commit but not changed inputs, arguments or failures', async t => {
    const root = await mkdtemp(path.join(tmpdir(), 'rust-pass-receipt-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    execFileSync('git', ['init', '-q', root]);
    await mkdir(path.join(root, 'src'));
    await writeFile(path.join(root, 'src/lib.rs'), 'original');
    const { verificationKey, receiptMatches } = await import('../scripts/verification-cache.mjs');
    const key = await verificationKey('all', [], root);
    assert.equal(receiptMatches({ passed: true, key }, key), true);
    assert.equal(receiptMatches({ passed: false, key }, key), false);
    execFileSync('git', ['add', 'src/lib.rs'], { cwd: root });
    execFileSync('git', ['-c', 'core.hooksPath=/dev/null', '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-qm', 'fixture'], { cwd: root });
    assert.equal(await verificationKey('all', [], root), key, 'Commit/staging must not invalidate a content-identical PASS');
    await writeFile(path.join(root, 'README.md'), 'docs only');
    assert.equal(await verificationKey('all', [], root), key, 'Prose does not change tested inputs');
    assert.notEqual(await verificationKey('all', ['--page', 'lifecycle'], root), key, 'Targeted run cannot satisfy full gate');
    await writeFile(path.join(root, 'src/lib.rs'), 'changed');
    assert.notEqual(await verificationKey('all', [], root), key);
    await rm(path.join(root, 'src/lib.rs'));
    assert.notEqual(await verificationKey('all', [], root), key, 'Removed inputs invalidate PASS');
});

test('verification records PASS only after all steps and a repeat skips the commands', async t => {
    const root = await mkdtemp(path.join(tmpdir(), 'rust-gate-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    await mkdir(path.join(root, 'scripts')); await mkdir(path.join(root, 'bin'));
    await copyFile(new URL('../scripts/verify.sh', import.meta.url), path.join(root, 'scripts/verify.sh'));
    await writeFile(path.join(root, 'bin/node'), `#!/bin/sh
case "$1:$2" in
scripts/verification-cache.mjs:check) test "\${TEST_FORCE:-0}" != 1 && test -f receipt ;;
scripts/verification-cache.mjs:key) echo fixture ;;
scripts/verification-cache.mjs:clear) rm -f receipt ;;
scripts/verification-cache.mjs:record) touch receipt ;;
scripts/test-browser.mjs:*) echo browser >> runs; exit "\${TEST_BROWSER_EXIT:-0}" ;;
*) exit 0 ;;
esac
`, { mode: 0o755 });
    for (const command of ['cargo', 'npm']) {
        await writeFile(path.join(root, `bin/${command}`), `#!/bin/sh\necho ${command} >> runs\nexit 0\n`, { mode: 0o755 });
    }
    const run = (exit = 0, force = 0, profile = 'dev') => spawnSync('bash', ['scripts/verify.sh', 'all'], {
        cwd: root, env: { ...process.env, PATH: `${root}/bin:${process.env.PATH}`, TEST_BROWSER_EXIT: String(exit), TEST_FORCE: String(force), VERIFY_WASM_PROFILE: profile },
    });
    const invalidProfile = run(0, 0, 'fast');
    assert.equal(invalidProfile.status, 2);
    assert.match(invalidProfile.stderr.toString(), /VERIFY_WASM_PROFILE must be dev or release/);
    assert.equal(run(19).status, 19, 'Browser failure must propagate');
    await assert.rejects(readFile(path.join(root, 'receipt')), /ENOENT/, 'No early PASS before browser success');
    assert.equal(run().status, 0);
    const commands = await readFile(path.join(root, 'runs'), 'utf8');
    assert.equal(run().status, 0);
    assert.equal(await readFile(path.join(root, 'runs'), 'utf8'), commands, 'Matching PASS must skip every test/build command');
    assert.equal(run(19, 1).status, 19);
    await assert.rejects(readFile(path.join(root, 'receipt')), /ENOENT/, 'Failed forced refresh must invalidate the previous PASS');
});

test('browser verification rejects missing builds, changed sources and replaced WASM/bindings', async t => {
    const root = await mkdtemp(path.join(tmpdir(), 'rust-build-marker-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    for (const directory of ['src', 'scripts', 'pkg', '.cargo', 'third_party/wgpu/src']) await mkdir(path.join(root, directory), {recursive:true});
    await copyFile(new URL('../scripts/build-fingerprint.mjs', import.meta.url), path.join(root, 'scripts/build-fingerprint.mjs'));
    const files = ['third_party/wgpu/src/lib.rs', 'third_party/wgpu/Cargo.toml', 'src/lib.rs', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml', 'pkg/webgpu_wasm.js', 'pkg/webgpu_wasm_bg.wasm'];
    for (const file of files) await writeFile(path.join(root, file), `fixture: ${file}`);
    const { fingerprint, assertFreshBuild, buildConfiguration, wasmProfile } = await import(pathToFileURL(path.join(root, 'scripts/build-fingerprint.mjs')));
    await assert.rejects(assertFreshBuild(), /No verified WASM build/);
    const expected = await fingerprint();
    await writeFile(path.join(root, 'pkg/verification-build.json'), JSON.stringify(expected));
    assert.deepEqual(await assertFreshBuild(), expected);
    assert.equal(buildConfiguration({}).profile, 'dev');
    assert.equal(buildConfiguration({ VERIFY_WASM_PROFILE: 'release' }).profile, 'release');
    assert.throws(() => wasmProfile({ VERIFY_WASM_PROFILE: 'optimized' }), /must be dev or release|Expected dev or release/);
    const previousProfile = process.env.VERIFY_WASM_PROFILE;
    try {
        process.env.VERIFY_WASM_PROFILE = expected.configuration.profile === 'release' ? 'dev' : 'release';
        await assert.rejects(assertFreshBuild(), /configuration changed/, 'Changing profiles must invalidate the active build marker');
    } finally {
        if (previousProfile === undefined) delete process.env.VERIFY_WASM_PROFILE;
        else process.env.VERIFY_WASM_PROFILE = previousProfile;
    }
    const previousFlags = process.env.RUSTFLAGS;
    try {
        process.env.RUSTFLAGS = `${previousFlags || ''} -C opt-level=1`;
        await assert.rejects(assertFreshBuild(), /configuration changed/, 'A binary from other build flags is not reusable');
    } finally {
        if (previousFlags === undefined) delete process.env.RUSTFLAGS;
        else process.env.RUSTFLAGS = previousFlags;
    }
    assert.deepEqual(buildConfiguration({}), buildConfiguration({ RUSTFLAGS: '--cfg=web_sys_unstable_apis --cfg=web_sys_unstable_apis' }));
    for (const file of ['third_party/wgpu/src/lib.rs', 'third_party/wgpu/Cargo.toml', 'src/lib.rs', 'pkg/webgpu_wasm.js', 'pkg/webgpu_wasm_bg.wasm']) {
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
