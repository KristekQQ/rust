'use strict';
const video = document.querySelector('#video');
const start = document.querySelector('#start');
const stop = document.querySelector('#stop');
const status = document.querySelector('#status');
let controller, objectUrl;
const token = new URLSearchParams(location.search).get('token') || '';
const event = (target, name, signal) => new Promise((resolve, reject) => {
  const abort = () => { cleanup(); reject(new DOMException('Odpojeno', 'AbortError')); };
  const done = e => { cleanup(); resolve(e); };
  const fail = () => { cleanup(); reject(new Error('Prohlížeč odmítl video. Zkus aktuální Chrome nebo Edge.')); };
  const cleanup = () => { target.removeEventListener(name, done); target.removeEventListener('error', fail); signal?.removeEventListener('abort', abort); };
  if (signal?.aborted) { abort(); return; }
  signal?.addEventListener('abort', abort, {once:true});
  target.addEventListener(name, done, {once:true}); target.addEventListener('error', fail, {once:true});
});
function join(a, b) { const c = new Uint8Array(a.length + b.length); c.set(a); c.set(b, a.length); return c; }
function codecFromInit(data) {
  for (let i = 4; i + 8 < data.length; i++) {
    if (data[i] === 97 && data[i+1] === 118 && data[i+2] === 99 && data[i+3] === 67 && data[i+4] === 1)
      return 'avc1.' + Array.from(data.subarray(i+5,i+8), x => x.toString(16).padStart(2,'0')).join('');
  }
  throw new Error('Chybí konfigurace H.264.');
}
start.onclick = async () => {
  if (!window.MediaSource) { status.textContent = 'Tento prohlížeč nepodporuje MediaSource. Použij aktuální Chrome nebo Edge.'; return; }
  start.disabled = true; stop.disabled = false;
  controller = new AbortController();
  const signal = controller.signal;
  let reader;
  try {
    status.textContent = 'Připojuji obraz z Questu…';
    const media = new MediaSource();
    if (objectUrl) URL.revokeObjectURL(objectUrl);
    objectUrl = URL.createObjectURL(media);
    const opened = event(media, 'sourceopen', signal); video.src = objectUrl; await opened;
    const response = await fetch('/stream?token=' + encodeURIComponent(token), {signal, cache:'no-store'});
    if (!response.ok) throw new Error(response.status === 409 ? 'Přenos už sledují dvě okna. Zavři jedno z nich a znovu klikni Sledovat.' : response.status === 403 ? 'Tento odkaz už neplatí. Otevři aktuální odkaz z Quest Stream.' : 'Sdílení není dostupné (' + response.status + '). Zkontroluj Quest Stream v brýlích.');
    reader = response.body.getReader();
    let pending = new Uint8Array(), init = new Uint8Array(), source;
    const append = async data => {
      const done = event(source, 'updateend', signal); source.appendBuffer(data); await done;
      if (video.buffered.length) {
        const end = video.buffered.end(video.buffered.length - 1);
        if (end - video.currentTime > 0.45) video.currentTime = Math.max(video.buffered.start(0), end - 0.12);
        if (video.paused) video.play().catch(() => { status.textContent = 'Přehrávání se nepodařilo spustit. Povol přehrávání videa v prohlížeči.'; });
        if (video.currentTime > 3 && source.buffered.length && source.buffered.start(0) < video.currentTime - 2) {
          const removed = event(source, 'updateend', signal); source.remove(0, video.currentTime - 2); await removed;
        }
      }
    };
    while (!signal.aborted) {
      const {value, done} = await reader.read(); if (done) break;
      pending = join(pending, value);
      while (pending.length >= 8) {
        const size = new DataView(pending.buffer, pending.byteOffset, pending.byteLength).getUint32(0);
        if (size < 8 || size > 8 * 1024 * 1024) throw new Error('Neplatný video segment.');
        if (pending.length < size) break;
        const box = pending.slice(0, size); pending = pending.slice(size);
        const type = String.fromCharCode(...box.subarray(4,8));
        if (!source) {
          init = join(init, box);
          if (init.length > 1024 * 1024) throw new Error('Příliš velká konfigurace videa.');
          if (type === 'moov') {
            const mime = 'video/mp4; codecs="' + codecFromInit(init) + '"';
            if (!MediaSource.isTypeSupported(mime)) throw new Error('Prohlížeč nepodporuje tento H.264 profil.');
            source = media.addSourceBuffer(mime); await append(init); init = null;
          }
        } else { await append(box); if (type === 'mdat') status.textContent = 'Živě · H.264 · bez zvuku'; }
      }
    }
    status.textContent = 'Spojení se přerušilo. Zkus znovu Sledovat; pokud sdílení skončilo i v brýlích, povol nový přenos.';
  } catch (e) {
    status.textContent = signal.aborted ? 'Odpojeno. Pro další sledování klikni Sledovat.' : e.message;
  } finally {
    controller.abort();
    if (reader) try { await reader.cancel(); } catch {}
    video.pause(); video.removeAttribute('src'); video.load();
    if (objectUrl) { URL.revokeObjectURL(objectUrl); objectUrl = null; }
    start.disabled = false; stop.disabled = true;
  }
};
stop.onclick = () => controller?.abort();
document.querySelector('#full').onclick = () => video.requestFullscreen?.().catch(e => { status.textContent = e.message; });
window.addEventListener('pagehide', () => controller?.abort());
