// Opt-in viewer-only LAN gateway. The publisher and private native API stay loopback-only.
import http from 'node:http';
import {randomBytes, timingSafeEqual} from 'node:crypto';
import {isIP} from 'node:net';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {WebSocket, WebSocketServer} from 'ws';

const cookieName = 'observer_viewer';
const dashboardPath = '/tools/scene-observer/';
const dashboardRoute = dashboardPath + '?source=native&backend=webgl&viewer=1';
const files = new Set([
  '/favicon.ico',
  ...['index.html','view.html','observer.css','dashboard.js','viewer.js','protocol.mjs','scene.js','byte-delta.mjs','interpolation-policy.mjs'].map(name => `/tools/scene-observer/${name}`),
  ...['engine.js','webxr.js','xr-presentation-profile.js','audio.js'].map(name => `/js/${name}`),
  '/examples/shared/audio-controls.js', '/examples/neon-arena/geometry.js',
  '/pkg/webgpu_wasm.js', '/pkg/webgpu_wasm_bg.wasm', '/assets/audio/neon/manifest.json',
]);
const statusKeys = new Set(['enabled','connected','publisher','viewers','session','sequence','cacheBytes','uncertaintyMs',
  'accepted','rejected','packets','bytes','limit','full','rawBytes','sentBytes','patchFrames','fullFrames','deltaFrames']);
function publicStatus(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  const result = {};
  for (const [key, item] of Object.entries(value)) {
    if (statusKeys.has(key) && (item === null || ['string','number','boolean'].includes(typeof item))) result[key] = item;
    else if (['scene','recording','egress'].includes(key)) result[key] = publicStatus(item);
  }
  return result;
}
function equalToken(value, token) {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value) && timingSafeEqual(Buffer.from(value), Buffer.from(token));
}

export async function createLanViewer({host, port = 8894, upstream = 'http://127.0.0.1:8891', token = randomBytes(32).toString('hex')} = {}) {
  // Explicit IPv4 bind also prevents accidentally publishing on all interfaces.
  if (isIP(host) !== 4 || host === '0.0.0.0') throw Error('An explicit local IPv4 --host is required');
  if (!/^[a-f0-9]{64}$/.test(token)) throw Error('Viewer token must be 32 random bytes encoded as hex');
  const source = new URL(upstream);
  if (source.protocol !== 'http:' || source.hostname !== '127.0.0.1' || source.username || source.password || source.pathname !== '/' || source.search || source.hash) throw Error('Upstream must be loopback HTTP');
  let authority;
  const pending = new Set(), sockets = new Set();
  const wss = new WebSocketServer({noServer:true, maxPayload:4096, perMessageDeflate:false});
  function allowedOrigin(req) { return req.headers.host === authority && (!req.headers.origin || req.headers.origin === `http://${authority}`); }
  function authenticated(req) {
    return (req.headers.cookie ?? '').split(';').some(part => {
      const [name, ...rest] = part.trim().split('=');
      return name === cookieName && equalToken(rest.join('='), token);
    });
  }
  function headers(res) {
    res.setHeader('Cache-Control','no-store');
    res.setHeader('Referrer-Policy','no-referrer');
    res.setHeader('X-Content-Type-Options','nosniff');
    res.setHeader('X-Frame-Options','SAMEORIGIN');
  }
  const server = http.createServer(async (req, res) => {
    headers(res);
    if (!allowedOrigin(req)) { res.writeHead(403).end('Forbidden origin'); return; }
    if (!['GET','HEAD'].includes(req.method)) { res.writeHead(405).end(); return; }
    let url;
    try { url = new URL(req.url, `http://${authority}`); } catch { res.writeHead(400).end(); return; }
    if (url.searchParams.has('token')) {
      if (req.method !== 'GET' || !['/','/tools/scene-observer/','/tools/scene-observer/index.html'].includes(url.pathname) || !equalToken(url.searchParams.get('token'), token)) { res.writeHead(401).end('Unauthorized'); return; }
      const clean = new URL(dashboardRoute, `http://${authority}`);
      res.setHeader('Set-Cookie',`${cookieName}=${token}; Path=/; HttpOnly; SameSite=Strict`);
      res.writeHead(303,{Location:clean.pathname + clean.search}).end(); return;
    }
    if (!authenticated(req)) { res.writeHead(401).end('Unauthorized'); return; }
    if (['/',dashboardPath,dashboardPath+'index.html'].includes(url.pathname) && url.pathname + url.search !== dashboardRoute) {
      res.writeHead(303,{Location:dashboardRoute}).end(); return;
    }
    const pathname = url.pathname === '/tools/scene-observer/' ? '/tools/scene-observer/index.html' : url.pathname;
    const status = ['/api/native','/api/scene','/api/status'].includes(pathname);
    if (!status && !files.has(pathname) && !/^\/assets\/audio\/neon\/[a-z0-9_-]+\.wav$/.test(pathname)) { res.writeHead(404).end(); return; }
    // Never pass cookies, bearer credentials, Origin, or client-controlled proxy headers upstream.
    const target = new URL(pathname, source);
    if (!status && url.searchParams.has('v')) target.searchParams.set('v',url.searchParams.get('v'));
    const proxy = http.get(target, {timeout:10000}, reply => {
      if (status) {
        const chunks = []; let length = 0;
        reply.on('data', chunk => { length += chunk.length; if (length > 1024 * 1024) reply.destroy(Error('Status too large')); else chunks.push(chunk); });
        reply.on('end', () => {
          try { const body = JSON.stringify(publicStatus(JSON.parse(Buffer.concat(chunks).toString()))); res.writeHead(reply.statusCode,{'Content-Type':'application/json'}).end(req.method === 'HEAD' ? undefined : body); }
          catch { if (!res.headersSent) res.writeHead(502).end(); }
        });
      } else {
        const type = pathname.endsWith('.wav') ? 'audio/wav' : reply.headers['content-type'] ?? 'application/octet-stream';
        res.writeHead(reply.statusCode, {'Content-Type':type});
        if (req.method === 'HEAD') { reply.resume(); res.end(); } else reply.pipe(res);
      }
      reply.on('error', () => { if (!res.headersSent) res.writeHead(502).end(); else res.destroy(); });
    });
    proxy.on('timeout', () => proxy.destroy(Error('Upstream timeout')));
    proxy.on('error', () => { if (!res.headersSent) res.writeHead(502).end(); else res.destroy(); });
    res.on('close', () => proxy.destroy());
  });
  server.on('upgrade', (req, socket, head) => {
    let url;
    try { url = new URL(req.url, `http://${authority}`); } catch { socket.destroy(); return; }
    const allowed = allowedOrigin(req) && authenticated(req) && url.pathname === '/observer' &&
      url.searchParams.getAll('role').length === 1 && url.searchParams.get('role') === 'viewer' &&
      [...url.searchParams.keys()].every(key => ['role','source','encoding'].includes(key)) &&
      (!url.searchParams.has('source') || url.searchParams.get('source') === 'native') &&
      (!url.searchParams.has('encoding') || url.searchParams.get('encoding') === 'ndp1');
    if (!allowed) { socket.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n'); return; }
    const target = new URL('/observer' + url.search, source); target.protocol = 'ws:';
    const peer = new WebSocket(target, {handshakeTimeout:5000, perMessageDeflate:false});
    pending.add(peer);
    socket.once('close', () => { if (peer.readyState === WebSocket.CONNECTING) peer.terminate(); });
    peer.on('error', () => socket.destroy());
    peer.once('open', () => {
      pending.delete(peer);
      if (socket.destroyed) { peer.terminate(); return; }
      wss.handleUpgrade(req, socket, head, viewer => {
        const close = () => { viewer.terminate(); peer.terminate(); };
        peer.on('message', (data, binary) => { if (viewer.readyState === WebSocket.OPEN) viewer.send(data,{binary}); });
        viewer.on('message', (data, binary) => {
          try {
            const control = JSON.parse(data.toString());
            if (binary || !['ready','ack','clock'].includes(control.type) || (control.type === 'clock' && !Number.isFinite(control.sentAt))) throw Error('Invalid viewer control');
            if (peer.readyState === WebSocket.OPEN) peer.send(data,{binary:false});
          } catch { viewer.close(1008,'Viewer controls only'); }
        });
        viewer.on('error',close); viewer.on('close', () => peer.close());
        peer.on('close', () => viewer.close()); peer.on('error',close);
      });
    });
    peer.once('close', () => pending.delete(peer));
  });
  server.on('connection', socket => { sockets.add(socket); socket.once('close', () => sockets.delete(socket)); });
  await new Promise((resolve,reject) => { server.once('error',reject); server.listen(port,host,resolve); });
  authority = `${host}:${server.address().port}`;
  return {server, token, url:`http://${authority}/tools/scene-observer/?token=${token}`, async close() {
    for (const peer of pending) peer.terminate();
    for (const viewer of wss.clients) viewer.terminate();
    for (const socket of sockets) socket.destroy();
    await new Promise(resolve => server.close(resolve)); wss.close();
  }};
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2), options = {};
  for (let i = 0; i < args.length; i += 2) {
    const name = args[i];
    if (!['--host','--port','--upstream'].includes(name) || !args[i+1]) throw Error('Usage: node tools/scene-observer/lan-viewer.mjs --host 192.168.0.17 [--port 8894] [--upstream http://127.0.0.1:8891]');
    options[name.slice(2)] = name === '--port' ? Number(args[i+1]) : args[i+1];
  }
  const gateway = await createLanViewer(options);
  console.log(`Private viewer URL (share only with trusted viewers): ${gateway.url}`);
  for (const signal of ['SIGINT','SIGTERM']) process.once(signal, () => { void gateway.close(); });
}
