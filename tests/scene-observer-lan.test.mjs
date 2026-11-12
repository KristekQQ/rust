import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import {once} from 'node:events';
import {WebSocket, WebSocketServer} from 'ws';
import {createLanViewer} from '../tools/scene-observer/lan-viewer.mjs';

async function fixture(t) {
  const requests = [];
  const upstream = http.createServer((req,res) => {
    requests.push({url:req.url, headers:req.headers});
    if (req.url.startsWith('/api/')) {
      res.setHeader('Content-Type','application/json');
      res.end(JSON.stringify({enabled:true,connected:true,token:'native-secret',port:8892,password:'password-secret',
        connectionUrl:'secret-url',lastError:'error with native-secret',latest:{token:'nested-secret'},
        scene:{connected:true,token:'scene-secret',port:8893},egress:{packets:3}}));
    } else res.end('static asset');
  });
  const wss = new WebSocketServer({server:upstream});
  const upgrades = [];
  wss.on('connection',(ws,req) => {
    upgrades.push({url:req.url,headers:req.headers});
    ws.send('session');
    ws.on('message',(data,binary) => ws.send(data,{binary}));
  });
  upstream.listen(0,'127.0.0.1'); await once(upstream,'listening');
  const gateway = await createLanViewer({host:'127.0.0.1',port:0,upstream:`http://127.0.0.1:${upstream.address().port}`});
  t.after(async () => {
    await gateway.close();
    for(const ws of wss.clients) ws.terminate();
    wss.close(); await new Promise(resolve => upstream.close(resolve));
  });
  const base = new URL(gateway.url).origin;
  const bootstrap = await fetch(gateway.url,{redirect:'manual'});
  const cookie = bootstrap.headers.get('set-cookie').split(';')[0];
  return {gateway,base,cookie,bootstrap,requests,upgrades};
}

test('LAN viewer authenticates bootstrap, cookie, Host and same-origin requests',async t => {
  const f = await fixture(t);
  assert.equal(f.bootstrap.status,303);
  assert.match(f.bootstrap.headers.get('set-cookie'),/HttpOnly; SameSite=Strict/);
  assert.equal(f.bootstrap.headers.get('location'),'/tools/scene-observer/?source=native&backend=webgl&viewer=1');
  for (const route of ['/tools/scene-observer/','/api/native','/pkg/webgpu_wasm_bg.wasm']) {
    assert.equal((await fetch(f.base+route)).status,401);
  }
  assert.equal((await fetch(f.base+'/?token=bad',{redirect:'manual'})).status,401);
  const canonical='/tools/scene-observer/?source=native&backend=webgl&viewer=1';
  for (const route of ['/','/?demo=1','/tools/scene-observer/','/tools/scene-observer/?source=native',
    '/tools/scene-observer/?demo=1','/tools/scene-observer/?source=native&backend=webgl&viewer=0',
    '/tools/scene-observer/index.html?source=native&backend=webgl&viewer=1',canonical+'&demo=1']) {
    const reply=await fetch(f.base+route,{headers:{cookie:f.cookie},redirect:'manual'});
    assert.equal(reply.status,303,route); assert.equal(reply.headers.get('location'),canonical,route);
    assert.equal(reply.headers.get('set-cookie'),null,'Existing viewer cookie remains sufficient');
  }
  assert.equal((await fetch(f.base+canonical,{headers:{cookie:f.cookie},redirect:'manual'})).status,200);

  const allowed = await fetch(f.base+'/tools/scene-observer/',{headers:{cookie:f.cookie,Origin:f.base}});
  assert.equal(allowed.status,200);
  assert.equal(allowed.headers.get('access-control-allow-origin'),null);
  assert.equal(allowed.headers.get('referrer-policy'),'no-referrer');
  assert.equal((await fetch(f.base+'/api/native',{headers:{cookie:f.cookie,Origin:'http://evil.invalid'}})).status,403);
  assert.equal(await new Promise((resolve,reject) => {
    http.get(f.base+'/api/native',{headers:{cookie:f.cookie,Host:'evil.invalid'}},res => {res.resume(); resolve(res.statusCode);}).on('error',reject);
  }),403);
  assert.equal((await fetch(f.base+'/api/native',{method:'POST',headers:{cookie:f.cookie}})).status,405);
});

test('LAN viewer serves only runtime files and strips upstream credentials/status secrets',async t => {
  const f = await fixture(t), headers={cookie:f.cookie,Authorization:'Bearer do-not-forward'};
  for (const route of ['/tools/scene-observer/viewer.js','/js/engine.js','/js/audio.js','/pkg/webgpu_wasm_bg.wasm?v=123',
    '/examples/shared/audio-controls.js','/examples/neon-arena/geometry.js','/assets/audio/neon/manifest.json','/assets/audio/neon/fruit_cut_soft_1.wav']) {
    assert.equal((await fetch(f.base+route,{headers})).status,200,route);
  }
  const count=f.requests.length;
  for (const route of ['/.env','/package.json','/src/lib.rs','/tools/scene-observer/server.mjs','/tools/scene-observer/native-poses.mjs',
    '/tools/scene-observer/lan-viewer.mjs','/api/recording','/assets/audio/neon/private.json','/js/engine.d.ts','/%2e%2e/.env']) {
    assert.equal((await fetch(f.base+route,{headers})).status,404,route);
  }
  assert.equal(f.requests.length,count);
  for (const route of ['/api/native','/api/scene','/api/status']) {
    const reply=await fetch(f.base+route,{headers});
    assert.deepEqual(await reply.json(),{enabled:true,connected:true,scene:{connected:true},egress:{packets:3}});
  }
  for (const req of f.requests) {
    assert.equal(req.headers.cookie,undefined); assert.equal(req.headers.authorization,undefined); assert.equal(req.headers.origin,undefined);
  }
});

function rejected(url,options={}) {
  return new Promise((resolve,reject) => {
    const ws=new WebSocket(url,options);
    ws.on('unexpected-response',(_req,res) => {res.resume(); ws.terminate(); resolve(res.statusCode);});
    ws.on('open',() => {ws.terminate(); reject(Error('Unexpected accepted websocket'));});
    ws.on('error',() => {});
  });
}
test('LAN websocket accepts authenticated viewers, rejects publishers and binary/control writes',async t => {
  const f=await fixture(t), base=f.base.replace('http:','ws:'), headers={Cookie:f.cookie,Origin:f.base};
  for (const route of ['/observer?role=publisher','/observer','/observer?role=viewer&role=publisher','/observer?role=viewer&token=x','/other?role=viewer']) {
    assert.equal(await rejected(base+route,{headers}),403,route);
  }
  assert.equal(await rejected(base+'/observer?role=viewer'),403);
  assert.equal(await rejected(base+'/observer?role=viewer',{headers:{...headers,Origin:'http://evil.invalid'}}),403);
  assert.equal(f.upgrades.length,0);
  const ws=new WebSocket(base+'/observer?role=viewer&source=native&encoding=ndp1',{headers});
  const session=once(ws,'message'); await once(ws,'open');
  assert.equal((await session)[0].toString(),'session');
  const echo=once(ws,'message'); ws.send(JSON.stringify({type:'clock',sentAt:123}));
  assert.deepEqual(JSON.parse((await echo)[0]),{type:'clock',sentAt:123});
  assert.equal(f.upgrades[0].url,'/observer?role=viewer&source=native&encoding=ndp1');
  assert.equal(f.upgrades[0].headers.cookie,undefined);
  const closed=once(ws,'close'); ws.send(Buffer.from('publish'));
  assert.equal((await closed)[0],1008);
});

test('LAN gateway requires explicit binding and loopback-only upstream', async () => {
  await assert.rejects(createLanViewer(),/explicit local IPv4/);
  await assert.rejects(createLanViewer({host:'0.0.0.0'}),/explicit local IPv4/);
  await assert.rejects(createLanViewer({host:'127.0.0.1',upstream:'http://192.168.0.17:8891'}),/loopback/);
});
