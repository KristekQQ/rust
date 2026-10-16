// Content-bound local PASS receipts. No HEAD/timestamp key: committing does not rerun tests.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFile, mkdir, writeFile, stat, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { root, assertFreshBuild } from './build-fingerprint.mjs';

const inputs = ['src', 'js', 'examples', 'tests', 'scripts', 'assets', '.github',
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml',
    'package.json', 'package-lock.json', 'tsconfig.json'];
const localOption = key => {
    try { return execFileSync('git', ['config', '--local', '--get', key], { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim(); }
    catch { return ''; }
};
export async function verificationKey(mode, args, directory = root) {
    const hash = createHash('sha256');
    const files = execFileSync('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', ...inputs], { cwd: directory, encoding: 'utf8' });
    for (const file of [...new Set(files.split('\0').filter(Boolean))].sort()) {
        if (path.basename(file) === '.DS_Store') continue;
        hash.update(file).update('\0');
        try {
            hash.update(await readFile(path.join(directory, file)));
            hash.update(String((await stat(path.join(directory, file))).mode & 0o111));
        }
        catch (error) { if (error.code !== 'ENOENT') throw error; hash.update('<removed>'); }
        hash.update('\0');
    }
    hash.update(JSON.stringify({ mode, args, platform: process.platform, arch: process.arch, node: process.version,
        env: Object.fromEntries(Object.entries(process.env).filter(([key]) => /^(BROWSER_|PUPPETEER_|CARGO_|RUST|GITHUB_ACTIONS$)/.test(key))),
        browser: localOption('verification.browser'), browserNode: localOption('verification.node'),
    }));
    hash.update(execFileSync('rustc', ['-Vv'], { encoding: 'utf8' }));
    for (const [tool, args] of [['cargo', ['-V']], ['wasm-pack', ['--version']]]) {
        try { hash.update(execFileSync(tool, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] })); }
        catch { hash.update(`${tool}:missing`); }
    }
    const browser = mode === 'native' ? '' : process.env.BROWSER_EXECUTABLE || localOption('verification.browser')
        || (await import('puppeteer')).default.executablePath();
    // Detect replaced browser/dependency installations without starting a browser.
    for (const file of [browser,
        process.env.BROWSER_NODE || localOption('verification.node'), path.join(directory, 'node_modules/.package-lock.json')].filter(Boolean)) {
        try { const info = await stat(file); hash.update(`${file}:${info.size}:${info.mtimeMs}`); }
        catch { hash.update(`${file}:missing`); }
    }
    return hash.digest('hex');
}
export function receiptMatches(receipt, key) { return receipt?.passed === true && receipt.key === key; }
async function generatedTypes() {
    try { return createHash('sha256').update(await readFile(path.join(root, 'pkg/webgpu_wasm.d.ts'))).digest('hex'); }
    catch { return null; }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
    const [command, mode, ...args] = process.argv.slice(2);
    const expected = command === 'record' ? args.shift() : undefined;
    const key = await verificationKey(mode, args);
    const destination = path.join(root, 'output/verification', `pass-${mode}-${createHash('sha256').update(JSON.stringify(args)).digest('hex').slice(0, 12)}.json`);
    if (command === 'key') console.log(key);
    else if (command === 'check') {
        try {
            const receipt = JSON.parse(await readFile(destination, 'utf8'));
            if (process.env.VERIFY_FORCE === '1' || !receiptMatches(receipt, key) || receipt.types !== await generatedTypes()) process.exit(1);
            if (mode !== 'native' && JSON.stringify(receipt.build) !== JSON.stringify(await assertFreshBuild())) process.exit(1);
            console.log(`PASS reused: ${mode} ${args.join(' ')} (unchanged inputs)`);
        } catch { process.exit(1); }
    } else if (command === 'clear') {
        await rm(destination, { force: true });
    } else if (command === 'record') {
        if (key !== expected) throw new Error('Verification inputs changed during checks; PASS not cached.');
        await mkdir(path.dirname(destination), { recursive: true });
        const build = mode === 'native' ? null : await assertFreshBuild();
        await writeFile(destination, JSON.stringify({ passed: true, key, mode, args, build, types: await generatedTypes(), finished: new Date().toISOString() }) + '\n');
    } else throw new Error('Expected key, check, clear or record');
}
