const puppeteer = require('puppeteer');
const fs = require('fs');

(async () => {
  const browser = await puppeteer.launch({
    headless: true,
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
  await page.exposeFunction('notifyReady', async () => {
    await page.screenshot({ path: 'render.png' });
    const base64 = fs.readFileSync('render.png').toString('base64');
    fs.writeFileSync('render_base64.txt', base64);
    console.log('Base64 saved to render_base64.txt');
    await browser.close();
  });

  await page.setContent(`<!DOCTYPE html>
<html>
<body>
<canvas id="c" width="256" height="256"></canvas>
<script type="module">
async function fallback() {
  const canvas = document.getElementById('c');
  const gl = canvas.getContext('webgl2');
  if (!gl) return;
  const vs = '#version 300 es\n'
    + 'in vec2 position;\n'
    + 'void main(){gl_Position=vec4(position,0.0,1.0);}';
  const fs = '#version 300 es\n'
    + 'precision mediump float;\n'
    + 'out vec4 outColor;\n'
    + 'void main(){outColor=vec4(0.0,1.0,0.0,1.0);}';
  const vsShader = gl.createShader(gl.VERTEX_SHADER);
  gl.shaderSource(vsShader, vs); gl.compileShader(vsShader);
  const fsShader = gl.createShader(gl.FRAGMENT_SHADER);
  gl.shaderSource(fsShader, fs); gl.compileShader(fsShader);
  const program = gl.createProgram();
  gl.attachShader(program, vsShader);
  gl.attachShader(program, fsShader);
  gl.linkProgram(program);
  gl.useProgram(program);
  const vertices = new Float32Array([0,0.5,-0.5,-0.5,0.5,-0.5]);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(program, 'position');
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  gl.clearColor(0,0,0,1);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
  await window.notifyReady();
}

async function run() {
  const canvas = document.getElementById('c');
  if (!('gpu' in navigator)) {
    await fallback();
    return;
  }

  const adapter = await navigator.gpu.requestAdapter();
  const device = await adapter.requestDevice();
  const context = canvas.getContext('webgpu');
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format });
  const shaderModule = device.createShaderModule({ code: \`
@vertex fn vs_main(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32>{
  var pos=array<vec2<f32>,3>(
    vec2<f32>(0.0,0.5),
    vec2<f32>(-0.5,-0.5),
    vec2<f32>(0.5,-0.5));
  return vec4<f32>(pos[i],0.0,1.0);
}
@fragment fn fs_main()->@location(0) vec4<f32>{
  return vec4<f32>(0.0,1.0,0.0,1.0);
}
  \` });
  const pipeline = device.createRenderPipeline({
    layout: 'auto',
    vertex: { module: shaderModule, entryPoint: 'vs_main' },
    fragment: { module: shaderModule, entryPoint: 'fs_main', targets: [{ format }] },
    primitive: { topology: 'triangle-list' }
  });
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({
    colorAttachments: [{
      view: context.getCurrentTexture().createView(),
      clearValue: { r: 0, g: 0, b: 0, a: 1 },
      loadOp: 'clear',
      storeOp: 'store'
    }]
  });
  pass.setPipeline(pipeline);
  pass.draw(3);
  pass.end();
  device.queue.submit([encoder.finish()]);
  await window.notifyReady();
}
run();
</script>
</body>
</html>`);
})();
