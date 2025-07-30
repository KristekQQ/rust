const puppeteer = require('puppeteer');

(async () => {
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
  await page.goto('about:blank');

  const { webgpu, webgl } = await page.evaluate(() => {
    const webgpu = typeof navigator.gpu !== 'undefined';
    const canvas = document.createElement('canvas');
    const webgl = !!(canvas.getContext('webgl') || canvas.getContext('webgl2'));
    return { webgpu, webgl };
  });

  console.log(webgpu ? 'WebGPU supported' : 'WebGPU not supported');
  console.log(webgl ? 'WebGL supported' : 'WebGL not supported');

  await browser.close();
})();

// Install dependencies: npm i puppeteer
// Run the script: node check_webgpu.js
