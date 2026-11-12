import http from 'node:http';
import { readFile, realpath, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocketServer, WebSocket } from 'ws';
import { WireCache, decodePacket, encodePacket, encodeRecording, RECORDING_LIMIT, CACHE_LIMIT } from './protocol.mjs';
import {createNativePoseBridge} from './native-poses.mjs';
import {createNativeSceneBridge} from './native-scene.mjs';
import {encodeByteDelta} from './byte-delta.mjs';
const loopback = host => ['127.0.0.1', 'localhost', '::1', '[::1]'].includes(host);
const mime = { '.html':'text/html', '.js':'text/javascript', '.mjs':'text/javascript', '.json':'application/json', '.wasm':'application/wasm', '.css':'text/css', '.svg':'image/svg+xml', '.png':'image/png', '.jpg':'image/jpeg' };
export async function createObserverServer({ root, host = '127.0.0.1', port = 8791, recordingLimit = RECORDING_LIMIT,nativePoses=false,nativeSnapshot=null } = {}) {
  if (!Number.isSafeInteger(recordingLimit) || recordingLimit < 4 || recordingLimit > RECORDING_LIMIT) throw new Error('Invalid recording limit');
  if (!loopback(host)) throw new Error('Observer server must bind loopback');
  const base = await realpath(root ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
  let cache = new WireCache(), publisher = null, session = null, latest = null;
  let tape = [], tapeBytes = 4, recordingFull = false;
  const viewers = new Map();
  const poseViewers=new Map();let nativeBridge=null,nativeScene=null;
  const egress={packets:0,rawBytes:0,sentBytes:0,patchFrames:0};
  const sendJSON = (ws, data) => { if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(data)); };
  const status = () => ({ session, publisher: publisher?.readyState === WebSocket.OPEN, viewers: viewers.size, sequence: cache.sequence.toString(), cacheBytes: cache.byteLength, recording: { packets: tape.length, bytes: tapeBytes, limit: recordingLimit, full: recordingFull }, latest });
  function pump(ws, peer) {
    if (!peer.ready || peer.inFlight || !latest || peer.sentSource === cache.sequence || ws.readyState !== WebSocket.OPEN) return;
    const wire = cache.packetFor(peer);
    peer.inFlight = true; peer.sentSource = cache.sequence;
    ws.send(encodePacket(latest, wire));
  }
  function sceneMeta(){
    const frame=nativeScene?.latest;if(!frame)return null;
    if(nativeScene.normalizedSnapshot)return {...frame,scene:true};
    const time=nativeBridge?.normalizeTimestamp(frame.sampledAt);if(!time)return null;
    const pose=nativeBridge.latest?.state;
    return {...frame,...time,session:'scene-'+frame.session,state:{...frame.state,head:pose?.head??null,swords:pose?.swords??0},scene:true};
  }
  function pumpPose(ws,peer){
    const scene=sceneMeta();
    if(scene){
      if(peer.session!==scene.session){
        peer.session=scene.session;peer.ready=false;peer.inFlight=false;peer.sentSource=null;peer.sequence=0;peer.known=[new Map(),new Map(),new Map()];peer.lastPacket=null;
        sendJSON(ws,{type:'session',session:scene.session,native:true,scene:true});
      }
      if(!peer.ready||peer.inFlight||peer.sentSource===nativeScene.cache.sequence||ws.readyState!==WebSocket.OPEN)return;
      const wire=nativeScene.cache.packetFor(peer);peer.inFlight=true;peer.sentSource=nativeScene.cache.sequence;
      const raw=encodePacket(scene,wire);egress.packets++;egress.rawBytes+=raw.length;
      if(peer.delta){const encoded=encodeByteDelta(peer.lastPacket,raw);peer.lastPacket={sequence:encoded.sequence,bytes:raw};egress.sentBytes+=encoded.payload.length;if(encoded.patch)egress.patchFrames++;ws.send(encoded.payload);}
      else{egress.sentBytes+=raw.length;ws.send(raw);}return;
    }
    if(nativeScene?.hasSnapshot)return;
    const frame=nativeBridge?.latest;
    if(!frame||!peer.ready||peer.inFlight||peer.sentSource===frame.sequence||ws.readyState!==WebSocket.OPEN)return;
    peer.inFlight=true;peer.sentSource=frame.sequence;
    sendJSON(ws,{type:'pose-frame',meta:frame});
  }
  if(nativePoses)nativeBridge=await createNativePoseBridge({...nativePoses,onFrame:frame=>{
    for(const [ws,peer] of poseViewers){
      if(nativeScene?.hasSnapshot){pumpPose(ws,peer);continue;}
      if(peer.session!==frame.session){peer.session=frame.session;peer.ready=false;peer.inFlight=false;peer.sentSource=null;sendJSON(ws,{type:'session',session:frame.session,native:true});}
      pumpPose(ws,peer);
    }
  }});
  if(nativeBridge)nativeScene=await createNativeSceneBridge({port:nativePoses.scenePort??nativeBridge.port+1,token:nativeBridge.token,initialSnapshot:nativeSnapshot,onFrame:()=>{for(const [ws,peer] of poseViewers)pumpPose(ws,peer);}});
  function sameOrigin(req) {
    if (!req.headers.origin) return true; // Native/local clients do not send Origin.
    try { const origin = new URL(req.headers.origin); return ['http:', 'https:'].includes(origin.protocol) && loopback(origin.hostname) && origin.host === req.headers.host; } catch { return false; }
  }
  const server = http.createServer(async (req, res) => {
    try {
      if (!loopback(new URL(`http://${req.headers.host}`).hostname) || !sameOrigin(req)) { res.writeHead(403).end('Forbidden origin'); return; }
      const url = new URL(req.url, `http://${req.headers.host}`);
      if (url.pathname === '/favicon.ico') { res.writeHead(204).end(); return; }
      if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405).end(); return; }
      if (url.pathname === '/api/scene') { res.writeHead(200, {'Content-Type':'application/json','Cache-Control':'no-store'}).end(JSON.stringify(nativeScene?{...nativeScene.status(),egress}:{enabled:false})); return; }
      if (url.pathname === '/api/status') { res.writeHead(200, {'Content-Type':'application/json', 'Cache-Control':'no-store'}).end(JSON.stringify(status())); return; }
      if(url.pathname==='/api/native'){
        res.writeHead(200,{'Content-Type':'application/json','Cache-Control':'no-store'}).end(JSON.stringify(nativeBridge?{...nativeBridge.status(),token:nativeBridge.token,scene:nativeScene?.status()}: {enabled:false}));return;
      }
      if (url.pathname === '/api/recording') {
        const recording = encodeRecording(tape);
        res.writeHead(200, {'Content-Type':'application/octet-stream', 'Content-Disposition':'attachment; filename="scene.nsr"', 'Content-Length':recording.length, 'Cache-Control':'no-store', 'X-Recording-Full':String(recordingFull)}).end(recording); return;
      }
      if(url.pathname==='/'){res.writeHead(302,{Location:'/tools/scene-observer/'+url.search}).end();return;}
      let candidate = path.resolve(base, '.' + decodeURIComponent(url.pathname === '/' ? '/tools/scene-observer/index.html' : url.pathname));
      if (candidate !== base && !candidate.startsWith(base + path.sep)) { res.writeHead(403).end(); return; }
      if ((await stat(candidate)).isDirectory()) candidate = path.join(candidate, 'index.html');
      candidate = await realpath(candidate);
      if (!candidate.startsWith(base + path.sep)) { res.writeHead(403).end(); return; }
      const data = await readFile(candidate);
      res.writeHead(200, {'Content-Type':mime[path.extname(candidate)] ?? 'application/octet-stream', 'Content-Length':data.length, 'Cache-Control':'no-store'}).end(req.method === 'HEAD' ? undefined : data);
    } catch (error) { res.writeHead(error.code === 'ENOENT' ? 404 : 400).end('Unavailable'); }
  });
  const wss = new WebSocketServer({ noServer:true, maxPayload:CACHE_LIMIT + 65540, perMessageDeflate:false });
  server.on('upgrade', (req, socket, head) => {
    let url;
    try { url = new URL(req.url, `http://${req.headers.host}`); } catch { socket.destroy(); return; }
    const role = url.searchParams.get('role');
    if (!loopback(url.hostname) || !sameOrigin(req) || url.pathname !== '/observer' || !['publisher','viewer'].includes(role) || (role === 'publisher' && publisher)) { socket.end('HTTP/1.1 403 Forbidden\r\n\r\n'); return; }
    wss.handleUpgrade(req, socket, head, ws => {
      if(role==='viewer'&&url.searchParams.get('source')==='native'){
        const scene=sceneMeta();
        const peer={session:scene?.session??nativeBridge?.latest?.session??null,ready:false,inFlight:false,sentSource:null,delta:url.searchParams.get('encoding')==='ndp1',sequence:0,known:[new Map(),new Map(),new Map()]};poseViewers.set(ws,peer);
        if(peer.session)sendJSON(ws,{type:'session',session:peer.session,native:true,scene:!!scene});
        ws.on('error',()=>{});ws.on('close',()=>poseViewers.delete(ws));
        ws.on('message',(data,binary)=>{
          try{
            if(binary||data.length>4096)throw Error('Invalid native viewer control');const message=JSON.parse(data.toString());
            if(message.type==='clock'){sendJSON(ws,{type:'clock',sentAt:message.sentAt,serverAt:Date.now()});return;}
            if(message.type==='ready'){peer.ready=true;pumpPose(ws,peer);}
            else if(message.type==='ack'){peer.inFlight=false;pumpPose(ws,peer);}
          }catch{ws.close(1008);}
        });return;
      }
      let peer;
      if (role === 'publisher') publisher = ws;
      else {
        peer = { sequence:0, known:[new Map(),new Map(),new Map()], ready:false, inFlight:false, sentSource:0n };
        viewers.set(ws, peer); if (session) sendJSON(ws, {type:'session',session});
      }
      ws.on('error', () => {});
      ws.on('close', () => { if (publisher === ws) publisher = null; viewers.delete(ws); });
      ws.on('message', (data, binary) => {
        try {
          if (!binary) {
            if (data.length > 4096) throw new Error('Control message too large');
            const message = JSON.parse(data.toString());
            if (message.type === 'clock' && Number.isFinite(message.sentAt)) { sendJSON(ws, {type:'clock',sentAt:message.sentAt,serverAt:Date.now()}); return; }
            if (role !== 'viewer') throw new Error('Unsupported publisher control');
            if (message.type === 'ready') { if (session && !peer.inFlight) { peer.ready = true; pump(ws, peer); } }
            else if (message.type === 'ack') { if (peer.ready && peer.inFlight) { peer.inFlight = false; pump(ws, peer); } }
            else throw new Error('Unsupported viewer control');
            return;
          }
          if (role !== 'publisher') throw new Error('Viewer cannot publish');
          const packet = new Uint8Array(data.buffer, data.byteOffset, data.byteLength), {meta,wire} = decodePacket(packet);
          if (meta.session !== session) {
            // Validate the first complete session frame before replacing any live state.
            const next = new WireCache(); next.ingest(wire); cache = next; session = meta.session;
            tape = []; tapeBytes = 4; recordingFull = false;
            for (const [viewer, p] of viewers) {
              p.sequence = 0; p.known = [new Map(),new Map(),new Map()]; p.ready = false; p.inFlight = false; p.sentSource = 0n;
              sendJSON(viewer, {type:'session',session});
            }
          } else cache.ingest(wire);
          latest = meta;
          if (!recordingFull) {
            if (tapeBytes + 4 + packet.length <= recordingLimit) { tape.push(packet.slice()); tapeBytes += packet.length + 4; }
            else { recordingFull = true; for (const viewer of viewers.keys()) sendJSON(viewer, {type:'recording',full:true,bytes:tapeBytes,limit:recordingLimit}); }
          }
          sendJSON(ws, {type:'ack', recordingFull});
          for (const [viewer, p] of viewers) pump(viewer, p);
        } catch (error) { sendJSON(ws, {type:'error',message:error.message,error:error.message}); ws.close(1008, 'Invalid observer message'); }
      });
    });
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(port, host, resolve); });
  const address = server.address(), url = `http://${host.includes(':') ? `[${host}]` : host}:${address.port}`;
  return { server, url,nativeBridge, close:async () => { for (const ws of wss.clients) ws.terminate(); await new Promise(resolve => wss.close(resolve));if(nativeScene)await nativeScene.close();if(nativeBridge)await nativeBridge.close(); await new Promise((resolve,reject) => server.close(error => error ? reject(error) : resolve())); } };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const nativeSnapshot=process.env.OBSERVER_SNAPSHOT?decodePacket(new Uint8Array(await readFile(process.env.OBSERVER_SNAPSHOT))):null;
  const observer = await createObserverServer({root:process.argv[2],nativeSnapshot, port:Number(process.env.PORT ?? 8791),nativePoses:{port:Number(process.env.OBSERVER_UDP_PORT??8792),token:process.env.OBSERVER_TOKEN}});
  console.log(`Scene observer: ${observer.url}`);
  for (const signal of ['SIGINT','SIGTERM']) process.once(signal, async () => { await observer.close(); process.exit(0); });
}
