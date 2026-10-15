// Capture a verified real Rust/WASM scene. Build first with npm run verify:browser.
const { spawnSync } = require('node:child_process');
const path = require('node:path');
const result = spawnSync(process.execPath, [path.join(__dirname, 'scripts/test-browser.mjs'),
    '--page', 'render-mirrored', '--backend', 'webgpu', '--capture', ...process.argv.slice(2)], { stdio: 'inherit' });
if (result.error) console.error(result.error.message);
process.exitCode = result.status ?? 1;
