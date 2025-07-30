const puppeteer = require('puppeteer');
const { spawn } = require('child_process');
const { PNG } = require('pngjs');
const fs = require('fs');

(async () => {
  // Start a simple Python HTTP server to host the built files.
  const server = spawn('python3', ['-m', 'http.server', '8000']);
  process.on('exit', () => server.kill());

  // Give the server a moment to start.
  await new Promise((resolve) => setTimeout(resolve, 1000));

  const browser = await puppeteer.launch({
    headless: 'new',
    args: [
      '--enable-unsafe-webgpu',
      '--enable-features=Vulkan,Metal,WebGPU',
      '--dawn-backend=swiftshader',
      '--use-angle=swiftshader',
      '--disable-gpu',
      '--no-sandbox'
    ]
  });

  const page = await browser.newPage();
  page.on('console', msg => console.log('PAGE:', msg.text()));
  page.on('requestfailed', req => console.log('REQUEST FAILED', req.url(), req.failure()?.errorText));
  page.on('response', res => {
    if (res.status() >= 400) console.log('RESPONSE', res.status(), res.url());
  });
  await page.goto('http://localhost:8000/cube.html');
  // Wait for the page to draw the cube.
  await new Promise(r => setTimeout(r, 1000));
  let buffer;
  for (let i = 0; i < 10; i++) {
    buffer = await page.screenshot();
    const png = PNG.sync.read(buffer);
    const nonZero = png.data.some((v, idx) => (idx % 4 !== 3) && v !== 0);
    if (nonZero) break;
    await page.waitForTimeout(500);
  }
  fs.writeFileSync('render.png', buffer);

  // Provide the screenshot as a Base64 string for easy copy/paste.
  const base64 = buffer.toString('base64');
  fs.writeFileSync('render_base64.txt', base64);
  console.log('Base64 saved to render_base64.txt');

  await browser.close();
  server.kill();
})();

// Run the capture: node screenshot_render.js
