// Private pairing/session and loopback-only Quest Stream gateway support.
import {randomBytes, randomInt, timingSafeEqual} from 'node:crypto';
import {readFile, open, chmod} from 'node:fs/promises';
import http from 'node:http';

export async function sessionSecret(filename) {
  if (!filename) return randomBytes(32).toString('hex');
  let file;
  try {
    file = await open(filename, 'wx', 0o600);
    await file.writeFile(randomBytes(32).toString('hex') + '\n');
  } catch (error) { if (error.code !== 'EEXIST') throw error; }
  finally { await file?.close(); }
  await chmod(filename, 0o600);
  const secret = (await readFile(filename, 'utf8')).trim();
  if (!/^[a-f0-9]{64}$/.test(secret)) throw Error('Invalid authentication file');
  return secret;
}
// Opt-in persistent short code for a trusted LAN, explicitly selected by the owner.
export async function permanentPairingCode(filename) {
  let file;
  try {
    file = await open(filename, 'wx', 0o600);
    await file.writeFile(String(randomInt(10000)).padStart(4,'0') + '\n');
  } catch (error) { if (error.code !== 'EEXIST') throw error; }
  finally { await file?.close(); }
  await chmod(filename, 0o600);
  const code = (await readFile(filename, 'utf8')).trim();
  if (!/^\d{4}$/.test(code)) throw Error('Invalid pairing code file');
  return code;
}
const style = '<style>body{font:20px system-ui;max-width:640px;margin:12vh auto;padding:24px;background:#10151e;color:#eef4ff}a{color:#8de6cb}input,button{font:inherit;padding:12px;margin:8px 0}iframe{position:fixed;inset:0;width:100%;height:100%;border:0}</style>';
function document(body) { return '<!doctype html><html lang="cs"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">' + style + body + '</html>'; }
const destinations = new Set(['/','/observer','/stream']);
export function pairingLimiter(now = Date.now) {
  let windowStart = now(), globalAttempts = 0;
  const attempts = new Map();
  return ip => {
    const time = now();
    if (time-windowStart >= 15*60*1000) { windowStart=time; globalAttempts=0; attempts.clear(); }
    const count=attempts.get(ip) ?? 0;
    if (count >= 5 || globalAttempts >= 20) return Math.ceil((windowStart+15*60*1000-time)/1000);
    attempts.set(ip,count+1); globalAttempts++;
    return 0;
  };
}
export function shortAccess({pairCode = String(randomInt(10000)).padStart(4,'0'), setCookie, authenticated, streamConfig, streamConfigFile, streamPort = 8088, now = Date.now, permanent = false}) {
  if (!/^\d{4}$/.test(pairCode)) throw Error('Pairing code must have four digits');
  // Hard caps do not depend on attacker-controlled forwarded headers. Global cap also
  // bounds distributed guessing and memory; authenticated requests bypass pairing.
  const expiresAt = permanent ? null : now() + 15*60*1000;
  const expired = () => expiresAt !== null && now() >= expiresAt;
  const expiredMessage = 'Párovací kód vypršel. Pro nový kód restartuj LAN bránu. Již připojené prohlížeče fungují dál.';
  const limit = pairingLimiter(now);
  async function pair(req,res,url) {
    if (expired()) { res.writeHead(410).end(expiredMessage); req.resume(); return; }
    if (req.headers['content-type'] !== 'application/x-www-form-urlencoded') { res.writeHead(415).end(); return; }
    const retryAfter=limit(req.socket.remoteAddress);
    if (retryAfter) { res.writeHead(429,{'Retry-After':String(retryAfter)}).end('Zkus to později.'); req.resume(); return; }
    if (Number(req.headers['content-length']) > 128) { res.writeHead(413).end(); req.resume(); return; }
    let length=0, chunks=[];
    try {
      for await (const chunk of req) { length+=chunk.length; if(length>128) { res.writeHead(413).end(); req.destroy(); return; } chunks.push(chunk); }
      if (expired()) { res.writeHead(410).end(expiredMessage); return; }
      const form=new URLSearchParams(Buffer.concat(chunks).toString());
      const code=form.get('code') ?? '', next=form.get('next') ?? '/';
      if ([...form.keys()].some(key=>!['code','next'].includes(key)) || form.getAll('code').length!==1 || form.getAll('next').length>1 || !destinations.has(next)) { res.writeHead(400).end(); return; }
      if (!/^\d{4}$/.test(code) || !timingSafeEqual(Buffer.from(code),Buffer.from(pairCode))) { res.writeHead(401).end('Nesprávný kód.'); return; }
      setCookie(res); res.writeHead(303,{Location:next}).end();
    } catch { if(!res.headersSent) res.writeHead(400).end(); }
  }
  async function stream(req,res,pathname) {
    const asset = {'/stream':'index.html','/stream/viewer.js':'viewer.js','/stream/playback-policy.js':'playback-policy.js'}[pathname];
    if(asset) {
      let body=await readFile(new URL('../quest-stream/assets/'+asset,import.meta.url),'utf8');
      // Keep standalone Quest Stream working; the LAN player has cookie auth and
      // never receives the device's credential.
      if(asset==='index.html') body=body.replace("s.src='/viewer.js?token='+encodeURIComponent(new URLSearchParams(location.search).get('token')||'')", "s.src='/stream/viewer.js'").replace('Odkaz je soukromý přístupový klíč; přenos', 'Přístup je spárovaný s tímto prohlížečem. Přenos');
      if(asset==='viewer.js') body=body.replace("const token = new URLSearchParams(location.search).get('token') || '';",'').replace("'/playback-policy.js?token=' + encodeURIComponent(token)","'/stream/playback-policy.js'").replace("'/stream?token=' + encodeURIComponent(token)","'/stream/media'");
      res.writeHead(200,{'Content-Type':asset.endsWith('.js')?'text/javascript; charset=utf-8':'text/html; charset=utf-8'}).end(req.method==='HEAD'?undefined:body); return;
    }
    if(!['/stream/media','/stream/status'].includes(pathname)) { res.writeHead(404).end(); return; }
    let target;
    try {
      const config=streamConfigFile ? JSON.parse(await readFile(streamConfigFile,'utf8')) : typeof streamConfig==='function' ? await streamConfig() : streamConfig;
      const url=new URL(config.url);
      if(url.protocol!=='http:' || url.hostname!=='127.0.0.1' || url.port!==String(streamPort) || url.pathname!=='/' || url.username || url.password || url.hash || url.searchParams.getAll('token').length!==1 || [...url.searchParams.keys()].some(key=>key!=='token') || !/^[a-f0-9]{32}$/.test(url.searchParams.get('token'))) throw Error();
      target=new URL(pathname==='/stream/media'?'/stream':'/status',url); target.search=url.search;
    } catch { res.writeHead(503).end('Quest Stream není dostupný.'); return; }
    const proxy=http.get(target,{timeout:10000},reply=>{
      if(reply.statusCode!==200) { reply.resume(); res.writeHead(reply.statusCode===409?409:503).end('Quest Stream není dostupný.'); return; }
      if(pathname==='/stream/status') {
        let size=0; const chunks=[];
        reply.on('data',chunk=>{size+=chunk.length;if(size>65536) reply.destroy();else chunks.push(chunk);});
        reply.on('end',()=>{try { const text=Buffer.concat(chunks).toString(); let value; try { value=JSON.parse(text); } catch { value={streaming:text.startsWith('Přenáším')}; } const clean={}; for(const key of ['streaming','clients','width','height','fps','audio']) if(['boolean','number'].includes(typeof value[key])) clean[key]=value[key]; res.writeHead(200,{'Content-Type':'application/json'}).end(req.method==='HEAD'?undefined:JSON.stringify(clean)); } catch {res.writeHead(502).end();}});
      } else {
        res.writeHead(200,{'Content-Type':'video/mp4'});
        if(req.method==='HEAD') { reply.destroy();res.end(); } else reply.pipe(res);
      }
      res.once('close',()=>reply.destroy());
      reply.on('error',()=>{if(!res.headersSent) res.writeHead(502).end();else res.destroy();});
    });
    proxy.on('timeout',()=>proxy.destroy());
    proxy.on('error',()=>{if(!res.headersSent) res.writeHead(502).end();else res.destroy();});
    res.once('close',()=>proxy.destroy());
  }
  return {pairCode,expiresAt,pair, async route(req,res,url) {
    const short=destinations.has(url.pathname);
    if(!authenticated(req)) {
      if(!short) return false;
      if (expired()) { res.writeHead(410,{'Content-Type':'text/html; charset=utf-8'}).end(req.method==='HEAD'?undefined:document('<title>Kód vypršel</title><h1>Kód vypršel</h1><p>'+expiredMessage+'</p>')); return true; }
      const next=url.pathname;
      // WebKit turns form POST Origin into null under no-referrer. Keep the
      // origin on same-origin pairing submissions without leaking it elsewhere.
      res.setHeader('Referrer-Policy','same-origin');
      res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'}).end(req.method==='HEAD'?undefined:document(`<title>Připojení</title><h1>Připojení</h1><form method="post" action="/pair"><label>Čtyřmístný kód<br><input name="code" type="text" inputmode="numeric" pattern="[0-9]{4}" maxlength="4" autocomplete="one-time-code" required autofocus></label><input type="hidden" name="next" value="${next}"><br><button>Připojit</button></form>`)); return true;
    }
    if(url.pathname==='/') { res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'}).end(req.method==='HEAD'?undefined:document('<title>Quest</title><h1>Quest</h1><p><a href="/observer">Scéna s vlastní kamerou</a></p><p><a href="/stream">Obraz z headsetu</a></p>'));return true; }
    if(url.pathname==='/observer') { res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'}).end(req.method==='HEAD'?undefined:document('<title>Pozorovatel</title><iframe title="Scéna s vlastní kamerou" src="/tools/scene-observer/?source=native&backend=webgl&viewer=1" allow="fullscreen"></iframe>'));return true; }
    if(url.pathname==='/stream' || url.pathname.startsWith('/stream/')) { await stream(req,res,url.pathname);return true; }
    return false;
  }};
}
