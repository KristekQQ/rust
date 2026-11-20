import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import {once} from 'node:events';
import {WebSocket, WebSocketServer} from 'ws';
import {createLanViewer} from '../tools/scene-observer/lan-viewer.mjs';

async function fixture(t, options={}) {
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
    if(options.largePacket)ws.send(options.largePacket);
    ws.on('message',(data,binary) => ws.send(data,{binary}));
  });
  upstream.listen(0,'127.0.0.1'); await once(upstream,'listening');
  const gateway = await createLanViewer({host:'127.0.0.1',port:0,upstream:`http://127.0.0.1:${upstream.address().port}`, ...options});
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
  for (const route of ['/tools/scene-observer/','/tools/scene-observer/?source=native',
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
  for (const route of ['/tools/scene-observer/viewer.js','/js/engine.js','/js/audio.js','/tools/scene-observer/scene-inbox.mjs','/pkg/webgpu_wasm_bg.wasm?v=123',
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

test('LAN cut bursts negotiate lossless compression and retain exact binary data',async t=>{
  const payload=Buffer.alloc(200000);for(let i=0;i<payload.length;i++)payload[i]=i%137;
  const f=await fixture(t,{largePacket:payload});
  const ws=new WebSocket(f.base.replace('http:','ws:')+'/observer?role=viewer',{headers:{Cookie:f.cookie,Origin:f.base}});
  const binary=new Promise((resolve,reject)=>{ws.on('error',reject);ws.on('message',(data,isBinary)=>{if(isBinary)resolve(data);});});
  try{await once(ws,'open');assert.match(ws.extensions,/permessage-deflate/);assert.deepEqual(await binary,payload);}finally{ws.terminate();}
});

test('LAN gateway requires explicit binding and loopback-only upstream', async () => {
  await assert.rejects(createLanViewer(),/explicit local IPv4/);
  await assert.rejects(createLanViewer({host:'0.0.0.0'}),/explicit local IPv4/);
  await assert.rejects(createLanViewer({host:'127.0.0.1',upstream:'http://192.168.0.17:8891'}),/loopback/);
});

const pair = (f,code,next='/observer',extra={}) => fetch(f.base+'/pair',{method:'POST',redirect:'manual',headers:{Origin:f.base,'Content-Type':'application/x-www-form-urlencoded',...extra},body:new URLSearchParams({code,next})});
test('short routes pair with four digits, retain destinations and hide credentials',async t=>{
  const f=await fixture(t,{pairCode:'0042'});
  for(const route of ['/','/observer','/stream']) {
    const page=await fetch(f.base+route); assert.equal(page.status,200);
    assert.equal(page.headers.get('referrer-policy'),'same-origin','Pairing form must retain its same-origin POST Origin');
    const body=await page.text(); assert.match(body,/action="\/pair"/); assert.ok(!body.includes(f.gateway.token));assert.ok(!body.includes('0042'));
  }
  assert.equal((await pair(f,'0000')).status,401);
  assert.equal((await pair(f,'0042','https://evil.invalid')).status,400);
  assert.equal((await pair(f,'0042','/stream',{Origin:'http://evil.invalid'})).status,403);
  assert.equal((await pair(f,'0042','/stream',{Origin:''})).status,403);
  assert.equal((await pair(f,'0042','/stream',{Origin:'null'})).status,403);
  const paired=await pair(f,'0042','/stream'); assert.equal(paired.status,303);assert.equal(paired.headers.get('location'),'/stream');
  assert.match(paired.headers.get('set-cookie'),/HttpOnly; SameSite=Strict; Max-Age=2592000/);
  const cookie=paired.headers.get('set-cookie').split(';')[0];
  for(const route of ['/','/observer','/stream','/stream/viewer.js','/stream/playback-policy.js']) {
    const response=await fetch(f.base+route,{headers:{cookie}});assert.equal(response.status,200);
    assert.equal(response.headers.get('referrer-policy'),'no-referrer');
    const body=await response.text(); assert.ok(!body.includes(f.gateway.token));assert.ok(!body.includes('?token='));
    if(route==='/observer') assert.match(body,/iframe.*source=native/);
    if(route==='/stream/viewer.js') assert.match(body,/fetch\('\/stream\/media'/);
  }
});
test('pairing enforces per-IP cap, bounded bodies and exact destination allowlist',async t=>{
  const f=await fixture(t,{pairCode:'0042'});
  const huge=await fetch(f.base+'/pair',{method:'POST',headers:{Origin:f.base,'Content-Type':'application/x-www-form-urlencoded'},body:'code='+ '0'.repeat(130)});assert.equal(huge.status,413);
  for(let i=0;i<4;i++) assert.equal((await pair(f,'0000')).status,401);
  const blocked=await pair(f,'0042');assert.equal(blocked.status,429);assert.ok(Number(blocked.headers.get('retry-after'))>0);
  assert.equal((await fetch(f.base+'/api/native',{headers:{cookie:f.cookie}})).status,200,'Existing authenticated viewers survive pairing lockout');
});
test('authentication file persists independent secret across new pairing codes',async t=>{
  const {mkdtemp,readFile,stat,rm}=await import('node:fs/promises');const {tmpdir}=await import('node:os'); const path=await import('node:path');
  const dir=await mkdtemp(path.join(tmpdir(),'viewer-auth-'));t.after(()=>rm(dir,{recursive:true,force:true}));const authFile=path.join(dir,'auth');
  const f=await fixture(t,{authFile,pairCode:'0001'});assert.equal((await stat(authFile)).mode & 0o777,0o600);
  assert.equal((await readFile(authFile,'utf8')).trim(),f.gateway.token);
  const g=await fixture(t,{authFile,pairCode:'0002'});assert.equal(g.gateway.token,f.gateway.token);
  assert.equal((await fetch(g.base+'/api/native',{headers:{cookie:f.cookie}})).status,200);
  assert.equal((await fetch(g.gateway.url,{redirect:'manual'})).status,303);
});
test('stream proxy refreshes private token, strips headers, sanitizes status and cleans up disconnect',async t=>{
  let token='1'.repeat(32), closedResolve;const closed=new Promise(resolve=>closedResolve=resolve);const requests=[];
  const upstream=http.createServer((req,res)=>{
    requests.push({url:req.url,headers:req.headers});
    if(req.url.startsWith('/status')) {res.end(JSON.stringify({streaming:true,clients:1,token,connectionUrl:'private'}));return;}
    res.writeHead(200,{'Content-Type':'video/mp4'});res.write(Buffer.from([0,1,2,3]));
    req.on('close',closedResolve);
  });
  upstream.listen(0,'127.0.0.1');await once(upstream,'listening');t.after(()=>new Promise(resolve=>upstream.close(resolve)));
  const port=upstream.address().port;
  const f=await fixture(t,{streamPort:port,streamConfig:()=>({url:`http://127.0.0.1:${port}/?token=${token}`})});
  const headers={cookie:f.cookie,Authorization:'Bearer private',Origin:f.base};
  const status=await fetch(f.base+'/stream/status',{headers});assert.deepEqual(await status.json(),{streaming:true,clients:1});
  token='2'.repeat(32);
  const controller=new AbortController();const media=await fetch(f.base+'/stream/media',{headers,signal:controller.signal});const reader=media.body.getReader();
  assert.deepEqual([...((await reader.read()).value)],[0,1,2,3]);controller.abort();await reader.cancel().catch(()=>{});
  await Promise.race([closed,new Promise((_,reject)=>{const timer=setTimeout(()=>reject(Error('Proxy did not disconnect')),2000);timer.unref();})]);
  assert.equal(requests[0].url,'/status?token='+ '1'.repeat(32));assert.equal(requests[1].url,'/stream?token='+token);
  for(const req of requests) {assert.equal(req.headers.cookie,undefined);assert.equal(req.headers.authorization,undefined);assert.equal(req.headers.origin,undefined);}
  assert.equal((await fetch(f.base+'/stream/private',{headers})).status,404);
});
test('stream rejects unsafe targets and never returns upstream error secrets',async t=>{
  for(const url of ['http://evil.invalid:8088/?token='+ '1'.repeat(32),'http://127.0.0.1:8088/other?token='+ '1'.repeat(32),'http://127.0.0.1:8088/?token=bad']) {
    const f=await fixture(t,{streamConfig:{url}});const response=await fetch(f.base+'/stream/media',{headers:{cookie:f.cookie}});assert.equal(response.status,503);assert.ok(!(await response.text()).includes(url));
  }
});
test('pairing limiter bounds distributed guesses and resets only after the window',async ()=>{
  const {pairingLimiter}=await import('../tools/scene-observer/lan-access.mjs');let now=1000;const limit=pairingLimiter(()=>now);
  for(let i=0;i<20;i++) assert.equal(limit('ip-'+i),0);
  assert.equal(limit('new-ip'),900);now+=899000;assert.equal(limit('new-ip'),1);now+=1000;assert.equal(limit('new-ip'),0);
});
test('stream config file is re-read and real device text status is sanitized',async t=>{
  const {mkdtemp,writeFile,rm}=await import('node:fs/promises'); const {tmpdir}=await import('node:os');const path=await import('node:path');
  const dir=await mkdtemp(path.join(tmpdir(),'viewer-stream-'));t.after(()=>rm(dir,{recursive:true,force:true}));const configFile=path.join(dir,'stream.json');
  const seen=[];const upstream=http.createServer((req,res)=>{seen.push(req.url);res.end('Přenáším 1920×1080 · private-secret');});
  upstream.listen(0,'127.0.0.1');await once(upstream,'listening');t.after(()=>new Promise(resolve=>upstream.close(resolve)));const port=upstream.address().port;
  const f=await fixture(t,{streamPort:port,streamConfigFile:configFile});
  for(const token of ['a'.repeat(32),'b'.repeat(32)]) {await writeFile(configFile,JSON.stringify({url:`http://127.0.0.1:${port}/?token=${token}`}));const response=await fetch(f.base+'/stream/status',{headers:{cookie:f.cookie}});assert.deepEqual(await response.json(),{streaming:true});assert.equal(seen.at(-1),'/status?token='+token);}
});
test('pairing code expires after fifteen minutes without renewal; authenticated sessions survive',async t=>{
  let now=1000;
  const f=await fixture(t,{pairCode:'0042',now:()=>now});
  assert.equal(f.gateway.expiresAt,901000);
  now=f.gateway.expiresAt-1;
  assert.equal((await pair(f,'0042')).status,303);
  now=f.gateway.expiresAt;
  for(const route of ['/','/observer','/stream']) {
    const response=await fetch(f.base+route);assert.equal(response.status,410);const body=await response.text();
    assert.match(body,/Párovací kód vypršel/);assert.ok(!body.includes('0042'));assert.ok(!body.includes('<form'));
    assert.equal((await fetch(f.base+route,{headers:{cookie:f.cookie}})).status,200);
  }
  for(const time of [now, now+900000, now+24*60*60*1000]) {
    now=time;const response=await pair(f,'0042');assert.equal(response.status,410);assert.ok(!(await response.text()).includes('0042'));assert.equal(response.headers.get('set-cookie'),null);
  }
  assert.equal((await fetch(f.base+'/api/native',{headers:{cookie:f.cookie}})).status,200);
  assert.equal((await fetch(f.gateway.url,{redirect:'manual'})).status,303,'Legacy high-entropy bootstrap stays compatible');
});


test('explicit permanent pairing file survives elapsed time and gateway restart',async t=>{
  const {mkdtemp,writeFile,readFile,stat,rm}=await import('node:fs/promises');
  const {tmpdir}=await import('node:os');const path=await import('node:path');
  const dir=await mkdtemp(path.join(tmpdir(),'viewer-permanent-'));t.after(()=>rm(dir,{recursive:true,force:true}));
  const pairCodeFile=path.join(dir,'pair-code.txt');await writeFile(pairCodeFile,'0042\n');
  let now=1000;const f=await fixture(t,{pairCodeFile,now:()=>now});
  assert.equal(f.gateway.pairCode,'0042');assert.equal(f.gateway.expiresAt,null);
  assert.equal((await stat(pairCodeFile)).mode & 0o777,0o600);
  now+=365*24*60*60*1000;
  assert.equal((await fetch(f.base+'/stream')).status,200);
  assert.equal((await pair(f,'0042')).status,303);
  for(let i=0;i<4;i++)assert.equal((await pair(f,'0000')).status,401);
  assert.equal((await pair(f,'0042')).status,429,'Permanent mode retains attempt limits');
  const g=await fixture(t,{pairCodeFile,now:()=>now});assert.equal(g.gateway.pairCode,'0042');
  assert.equal((await pair(g,'0042')).status,303);
  assert.equal(await readFile(pairCodeFile,'utf8'),'0042\n');
});
