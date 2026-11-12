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
    const {playbackDecision} = await import('/playback-policy.js?token=' + encodeURIComponent(token));
    if (signal.aborted) throw new DOMException('Odpojeno', 'AbortError');
    status.textContent = 'Připojuji obraz z Questu…';
    const media = new MediaSource();
    if (objectUrl) URL.revokeObjectURL(objectUrl);
    objectUrl = URL.createObjectURL(media);
    const opened = event(media, 'sourceopen', signal); video.src = objectUrl; await opened;
    const response = await fetch('/stream?token=' + encodeURIComponent(token), {signal, cache:'no-store'});
    if (!response.ok) throw new Error(response.status === 409 ? 'Přenos už sledují dvě okna. Zavři jedno z nich a znovu klikni Sledovat.' : response.status === 403 ? 'Tento odkaz už neplatí. Otevři aktuální odkaz z Quest Stream.' : 'Sdílení není dostupné (' + response.status + '). Zkontroluj Quest Stream v brýlích.');
    reader = response.body.getReader();
    let pending = new Uint8Array(), init = new Uint8Array(), source, fragment;
    let lastPruned = 0, firstFrameAt, playbackStarted = false;
    const append = async data => {
      const done = event(source, 'updateend', signal); source.appendBuffer(data); await done;
      if (video.buffered.length) {
        const ranges = Array.from({length: video.buffered.length}, (_, i) => ({start:video.buffered.start(i), end:video.buffered.end(i)}));
        if (!playbackStarted) {
          firstFrameAt ??= performance.now();
          const first = ranges[0];
          // Start with reserve; a sparse/static capture must still be allowed to start.
          if (first.end - first.start < 0.3 && performance.now() - firstFrameAt < 600) return;
          video.currentTime = first.start;
          playbackStarted = true;
        }
        const next = playbackDecision(video.currentTime, ranges);
        if (next.seek !== null) video.currentTime = next.seek;
        if (video.playbackRate !== next.rate) video.playbackRate = next.rate;
        if (video.paused) video.play().catch(() => { status.textContent = 'Přehrávání se nepodařilo spustit. Povol přehrávání videa v prohlížeči.'; });
        if (video.currentTime > 3 && video.currentTime - lastPruned >= 1 && source.buffered.length && source.buffered.start(0) < video.currentTime - 2) {
          const removed = event(source, 'updateend', signal); source.remove(0, video.currentTime - 2); await removed;
          lastPruned = video.currentTime;
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
        } else if (type === 'moof') {
          if (fragment) throw new Error('Neúplný video snímek.');
          fragment = box;
        } else if (type === 'mdat') {
          if (!fragment) throw new Error('Chybí hlavička video snímku.');
          // One complete frame per MSE append instead of two separate updates.
          await append(join(fragment, box)); fragment = null;
          status.textContent = 'Živě · H.264 · bez zvuku';
        } else { throw new Error('Neočekávaný video segment.'); }
      }
    }
    status.textContent = 'Spojení se přerušilo. Zkus znovu Sledovat; pokud sdílení skončilo i v brýlích, povol nový přenos.';
  } catch (e) {
    status.textContent = signal.aborted ? 'Odpojeno. Pro další sledování klikni Sledovat.' : e.message;
  } finally {
    controller.abort();
    if (reader) try { await reader.cancel(); } catch {}
    video.pause(); video.playbackRate = 1; video.removeAttribute('src'); video.load();
    if (objectUrl) { URL.revokeObjectURL(objectUrl); objectUrl = null; }
    start.disabled = false; stop.disabled = true;
  }
};
stop.onclick = () => controller?.abort();
document.querySelector('#full').onclick = () => video.requestFullscreen?.().catch(e => { status.textContent = e.message; });
window.addEventListener('pagehide', () => controller?.abort());
