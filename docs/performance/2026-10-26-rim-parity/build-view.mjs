#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';

const usage = 'Usage: node build-view.mjs view-data.json index.html';
const args = process.argv.slice(2);
if (args.length !== 2) {
  console.error(usage);
  process.exit(2);
}

const inputPath = path.resolve(args[0]);
const outputPath = path.resolve(args[1]);
const data = JSON.parse(fs.readFileSync(inputPath, 'utf8'));
validate(data);

// Image paths in the input belong to that JSON file, even when the report is
// written to another directory. Store portable relative paths in the report.
for (const scene of data.scenes) {
  for (const key of ['before', 'after']) {
    const value = scene.images?.[key];
    if (typeof value === 'string' && value) {
      const absolute = path.resolve(path.dirname(inputPath), value);
      scene.images[key] = path.relative(path.dirname(outputPath), absolute).split(path.sep).join('/');
    }
  }
}

const embedded = JSON.stringify(data).replace(/[<>&\u2028\u2029]/g, (char) => ({
  '<': '\\u003c', '>': '\\u003e', '&': '\\u0026', '\u2028': '\\u2028', '\u2029': '\\u2029',
}[char]));
const html = `<!doctype html>
<html lang="cs">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>${escapeHtml(data.title)}</title>
<style>
:root{font:16px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif;color-scheme:light dark;--bg:#f3f5f8;--panel:#fff;--ink:#182230;--muted:#586575;--line:#d7dee7;--blue:#2563eb;--orange:#c2410c;--soft:#e9eef6}
@media(prefers-color-scheme:dark){:root{--bg:#111821;--panel:#1b2633;--ink:#edf3fa;--muted:#b0bdcc;--line:#364454;--blue:#74a5ff;--orange:#ff9a68;--soft:#263446}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink)}main{max-width:1180px;margin:auto;padding:clamp(16px,4vw,42px)}h1,h2,h3,p{margin-top:0}h1{font-size:clamp(1.7rem,4vw,2.5rem);line-height:1.15;margin-bottom:.5rem}h2{font-size:1.3rem;margin-bottom:.8rem}h3{font-size:1rem;margin-bottom:.5rem}.muted,.note{color:var(--muted)}.panel{background:var(--panel);border:1px solid var(--line);border-radius:16px;padding:clamp(16px,3vw,24px);margin:18px 0;box-shadow:0 5px 18px #14243a0b}.hero{padding:14px 0 4px}.notice{border-left:4px solid #d97706;background:var(--soft);padding:14px 18px;border-radius:8px;margin:20px 0}.controls{display:flex;flex-wrap:wrap;gap:16px;align-items:end}.controls label{display:grid;gap:5px;font-weight:650;flex:1 1 220px;min-width:0;max-width:100%}.controls select{width:100%;max-width:100%;min-width:0}select,input{font:inherit;color:var(--ink);background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:8px 10px}.builds,.stats,.work-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(210px,1fr));gap:12px}.build-card,.stat,.work-item{background:var(--soft);border-radius:10px;padding:13px;min-width:0}.build-card span{display:block;overflow-wrap:anywhere;word-break:break-word}.build-card strong,.stat strong{display:block;font-size:1.08rem}.bar-row{display:grid;grid-template-columns:minmax(90px,130px) 1fr auto;gap:10px;align-items:center;margin:11px 0}.bar-track{height:15px;border-radius:9px;background:var(--soft);overflow:hidden}.bar{height:100%;min-width:2px;border-radius:9px}.bar.before{background:var(--blue)}.bar.after{background:var(--orange)}.legend{display:flex;gap:18px;flex-wrap:wrap;color:var(--muted);font-size:.92rem}.dot{display:inline-block;width:11px;height:11px;border-radius:50%;margin-right:5px}.samples{font-variant-numeric:tabular-nums}.image-wrap{position:relative;max-width:100%;overflow:hidden;background:#0b1017;border-radius:10px;min-height:180px;display:grid;place-items:center}.image-wrap img{display:block;width:100%;max-width:100%;height:auto}.image-wrap img.overlay{position:absolute;inset:0;width:100%;height:100%;object-fit:contain;clip-path:inset(0 0 0 var(--split,50%))}.image-label{position:absolute;top:8px;background:#101820d9;color:white;padding:4px 8px;border-radius:5px;font-size:.85rem}.image-label.left{left:8px}.image-label.right{right:8px}.slider{width:100%;margin:10px 0}.image-caption{font-size:.9rem;color:var(--muted)}.image-pair{display:grid;grid-template-columns:1fr 1fr;gap:12px}.image-pair figure{margin:0}.image-pair img{display:block;width:100%;height:auto;background:#0b1017;border-radius:8px}.table-wrap{overflow-x:auto}table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums}th,td{text-align:right;padding:9px 8px;border-bottom:1px solid var(--line);white-space:nowrap}th:first-child,td:first-child{text-align:left}summary{cursor:pointer;font-weight:700}details{margin-top:12px}a{color:var(--blue)}.status{padding:10px 12px;background:var(--soft);border-radius:8px}.empty{color:var(--muted);padding:14px 0}footer{font-size:.9rem;color:var(--muted);padding:12px 0 30px}
@media(max-width:580px){.bar-row{grid-template-columns:80px 1fr auto;gap:7px;font-size:.9rem}.image-pair{grid-template-columns:1fr}.panel{border-radius:12px}}
 .bar-track{position:relative}.sample-point{position:absolute;top:1px;width:13px;height:13px;border:2px solid var(--panel);border-radius:50%;transform:translateX(-50%);z-index:1}.sample-point.before{background:var(--blue)}.sample-point.after{background:var(--orange)}
</style>
</head>
<body><main>
<header class="hero"><h1 id="title"></h1><p id="subtitle" class="muted"></p></header>
<section class="panel"><h2>Výsledek podle dat</h2><p id="verdict"></p></section>
<section class="notice" aria-label="Důležitý kontext"><strong>Jak číst výsledek:</strong> limit 60 Hz se během srovnání nezměnil; stejný počet snímků za sekundu proto sám o sobě nedokazuje pokrok. Výsledky platí pro uvedené scény a měření, nikoli obecně.</section>
<section class="panel" aria-labelledby="pick-title"><h2 id="pick-title">Vyberte měření</h2><div class="controls"><label>Backend<select id="backend"></select></label><label>Režim<select id="mode"></select></label></div><p id="caption" class="muted" style="margin:12px 0 0"></p></section>
<section class="panel"><details><summary>Sestavení a kontrolní otisky</summary><div id="builds" class="builds"></div></details></section>
<section class="panel"><h2>FPS podle jednotlivých měření</h2><p class="note">Každý bod je jeden běh; FPS v něm vychází z průměrné doby snímku: 1 000 ÷ průměrné RAF v ms. Souhrn ukazuje medián z dostupných běhů. Limit 60 Hz může FPS zastropovat.</p><div class="legend"><span><i class="dot" style="background:var(--blue)"></i><span id="beforeLegend"></span></span><span><i class="dot" style="background:var(--orange)"></i><span id="afterLegend"></span></span></div><div id="fps"></div><p id="ratio" class="status"></p></section>
<section class="panel"><h2>CPU práce snímku</h2><p class="note">CPU čas zahrnuje práci aplikace, simulaci a přípravu vykreslení. Je oddělený od času GPU, který je uveden níže.</p><div id="cpu"></div><p class="note">CPU je zde doplňkový údaj. Mediány jsou v tabulce jednotlivých párů.</p></section>
<section class="panel"><h2>Skutečné snímky scény</h2><div id="images"></div></section>
<section class="panel"><h2>Výsledky jednotlivých párů</h2><div class="table-wrap"><table><thead><tr><th>Pár</th><th>FPS · <span id="pairBeforeFps"></span></th><th>FPS · <span id="pairAfterFps"></span></th><th>CPU medián · <span id="pairBeforeCpu"></span> (ms)</th><th>CPU medián · <span id="pairAfterCpu"></span> (ms)</th><th>Frame P95 · <span id="pairBeforeP95"></span> (ms)</th><th>Frame P95 · <span id="pairAfterP95"></span> (ms)</th><th>Frame P99 · <span id="pairBeforeP99"></span> (ms)</th><th>Frame P99 · <span id="pairAfterP99"></span> (ms)</th></tr></thead><tbody id="pairs"></tbody></table></div></section>
<section class="panel"><h2>GPU, práce a limity</h2><p id="gpu"></p><div id="work" class="work-grid"></div><details><summary>Metodika a omezení</summary><div id="protocol"></div><ul id="limits"></ul><ul id="caveats"></ul><div id="evidence"></div></details></section>
<footer>Vygenerováno z přiložených měřicích dat. Obrázky jsou původní snímky ze stejné scény.</footer>
</main>
<script type="application/json" id="data">${embedded}</script>
<script>
(()=>{'use strict';const d=JSON.parse(document.getElementById('data').textContent),$=id=>document.getElementById(id),el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n},fmt=n=>Number.isFinite(n)?n.toFixed(2):'—',median=a=>{const sorted=[...a].sort((x,y)=>x-y),middle=Math.floor(sorted.length/2);return sorted.length?(sorted.length%2?sorted[middle]:(sorted[middle-1]+sorted[middle])/2):NaN},medianLabel=n=>n===1?'Hodnota z jednoho běhu':n===3?'Medián ze tří běhů':'Medián z '+n+' běhů',label=(s,side)=>s.labels?.[side]||(side==='before'?'Původní':'Současné');
document.title=d.title;$('title').textContent=d.title;$('subtitle').textContent=d.subtitle||'';$('verdict').textContent=d.verdict||'Verdikt nebyl uveden.';$('protocol').textContent=d.protocol||'Metodika nebyla uvedena.';
const backend=$('backend'),mode=$('mode'),unique=a=>[...new Set(a)];for(const v of unique(d.scenes.map(s=>s.backend)))backend.add(new Option(v,v));backend.value=d.scenes[0].backend;
function fillModes(){const prev=mode.value;mode.replaceChildren();for(const v of unique(d.scenes.filter(s=>s.backend===backend.value).map(s=>s.mode)))mode.add(new Option(v,v));if([...mode.options].some(o=>o.value===prev))mode.value=prev;render()}
backend.addEventListener('change',fillModes);mode.addEventListener('change',render);fillModes();
function render(){const s=d.scenes.find(x=>x.backend===backend.value&&x.mode===mode.value);if(!s)return;$('caption').textContent=s.caption||'';
const labels={before:label(s,'before'),after:label(s,'after')};$('beforeLegend').textContent=labels.before;$('afterLegend').textContent=labels.after;for(const id of ['pairBeforeFps','pairBeforeCpu','pairBeforeP95','pairBeforeP99'])$(id).textContent=labels.before;for(const id of ['pairAfterFps','pairAfterCpu','pairAfterP95','pairAfterP99'])$(id).textContent=labels.after;const builds=$('builds');builds.replaceChildren();for(const side of ['before','after']){const b=s.builds?.[side]??d.builds?.[side];const card=el('div','build-card');card.append(el('strong','',labels[side]));card.append(el('span','',typeof b==='string'?b:b==null?'—':JSON.stringify(b)));builds.append(card)}
const fps=$('fps'),fpsBefore=s.fpsBefore||[],fpsAfter=s.fpsAfter||[],maxFps=Math.max(...fpsBefore,...fpsAfter,1);fps.replaceChildren();for(const [side,values,kind] of [['before',fpsBefore,'before'],['after',fpsAfter,'after']]){const row=el('div','bar-row');row.append(el('span','',labels[side]));const track=el('div','bar-track'),bar=el('div','bar '+kind),value=median(values);bar.style.width=Math.max(0,value/maxFps*100)+'%';track.append(bar);for(const [i,n] of values.entries()){const point=el('i','sample-point '+kind);point.style.left=Math.max(1,Math.min(99,n/maxFps*100))+'%';point.title='Běh '+(i+1)+': '+fmt(n)+' FPS';track.append(point)}row.append(track);row.append(el('strong','',medianLabel(values.length)+': '+fmt(value)+' FPS'));fps.append(row);fps.append(el('p','samples muted','Jednotlivé běhy: '+values.map((n,i)=>'#'+(i+1)+' '+fmt(n)+' FPS').join(' · ')))}
const cpuMax=Math.max(...s.cpuBefore,...s.cpuAfter,0.01),cpu=$('cpu');cpu.replaceChildren();for(const [side,values,kind] of [['before',s.cpuBefore,'before'],['after',s.cpuAfter,'after']]){const value=median(values),row=el('div','bar-row');row.append(el('span','',labels[side]));const track=el('div','bar-track'),bar=el('div','bar '+kind);bar.style.width=(value/cpuMax*100)+'%';track.append(bar);row.append(track);row.append(el('strong','',fmt(value)+' ms'));cpu.append(row);cpu.append(el('p','samples muted',medianLabel(values.length)+': '+fmt(value)+' ms'))}
const n=(s.pairs||[]).length,pairWord=n===1?'pár':n>=2&&n<=4?'páry':'párů';$('ratio').textContent='Poměr mediánu FPS po / před: '+fmt(s.fpsRatio)+'×. Opakování: '+n+' '+pairWord+'.';
const images=$('images');images.replaceChildren();if(s.images?.before&&s.images?.after){const wrap=el('div','image-wrap');const before=el('img',''),after=el('img','overlay');before.src=s.images.before;before.alt=labels.before+' vykreslení';after.src=s.images.after;after.alt=labels.after+' vykreslení';after.addEventListener('load',()=>{wrap.style.aspectRatio=(after.naturalWidth||16)+' / '+(after.naturalHeight||9)});wrap.append(before,after);wrap.style.setProperty('--split','50%');wrap.append(el('span','image-label left',labels.before),el('span','image-label right',labels.after));const slider=document.createElement('input');slider.className='slider';slider.type='range';slider.min='0';slider.max='100';slider.value='50';slider.setAttribute('aria-label','Posuvník porovnání snímků');slider.addEventListener('input',()=>wrap.style.setProperty('--split',(100-slider.value)+'%'));images.append(wrap,slider,el('p','image-caption','Posuňte jezdec a porovnejte snímky. Pixelová kontrola uvádí výsledek pro všechna měření.'))}else images.append(el('p','empty','Pro toto měření nejsou k dispozici oba snímky.'));
const pixel=s.pixel;if(pixel){const passed=pixel.pass===true,failed=pixel.pass===false;const p=el('p','status',passed?'Pixelová kontrola prošla.':failed?'Pixelová kontrola neprošla.':'Pixelová shoda není vyhodnocená.');if(passed||failed)p.append(' Přesné pixely: '+(pixel.exact===true?'ano':pixel.exact===false?'ne':'—')+'; max. odchylka kanálu: '+(pixel.maxChannelDelta??'—'));if(pixel.reason)p.append(' · '+pixel.reason);images.append(p)}else images.append(el('p','status','Pixelová shoda není vyhodnocená.'));
const pairs=$('pairs');pairs.replaceChildren();for(const p of s.pairs||[]){const tr=document.createElement('tr');for(const v of [p.pair,p.beforeFps,p.afterFps,p.beforeCpu,p.afterCpu,p.beforeFrameP95,p.afterFrameP95,p.beforeFrameP99,p.afterFrameP99])tr.append(el('td','',v==null?'—':typeof v==='number'?fmt(v):v));pairs.append(tr)}
$('gpu').textContent='GPU čas: '+(s.gpuBefore==null?'nezměřen':fmt(s.gpuBefore)+' ms původní')+' · '+(s.gpuAfter==null?'nezměřen':fmt(s.gpuAfter)+' ms současné')+'. Rozsah: '+(s.gpuScope||'neuveden')+'.';const work=$('work');work.replaceChildren();for(const w of s.work||[]){const box=el('div','work-item');box.append(el('strong','',w.label));box.append(el('div','',fmt(w.before)+' → '+fmt(w.after)));work.append(box)}
const limits=$('limits');limits.replaceChildren();for(const x of d.limits||[])limits.append(el('li','',x));const caveats=$('caveats');caveats.replaceChildren();for(const x of s.caveats||[])caveats.append(el('li','',x));const evidence=$('evidence');evidence.replaceChildren();for(const item of d.evidence||[]){const href=item.href||'';if(!(href.startsWith('https:')||href.startsWith('http:')||href.startsWith('./')||href.startsWith('../')||href.startsWith('#')))continue;const a=el('a','',item.label);a.href=href;a.rel='noopener';a.target='_blank';evidence.append(a,document.createTextNode(' '))}}
})();
</script></body></html>`;

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, html);
console.log(`Wrote ${outputPath}`);

function validate(value) {
  if (!value || typeof value !== 'object' || !Array.isArray(value.scenes) || !value.scenes.length) throw new Error('Input must include a non-empty scenes array.');
  for (const name of ['before', 'after']) {
    const build = value.builds?.[name];
    if (!(typeof build === 'string' ? build.trim().length > 0 : build && typeof build === 'object' && Object.keys(build).length > 0)) throw new Error(`builds.${name} must be a non-empty string or object.`);
  }
  for (const [index, scene] of value.scenes.entries()) {
    if (!scene || typeof scene.backend !== 'string' || typeof scene.mode !== 'string' || !Array.isArray(scene.cpuBefore) || !Array.isArray(scene.cpuAfter)) throw new Error(`Scene ${index} needs backend, mode, cpuBefore and cpuAfter.`);
    if (!scene.cpuBefore.length || scene.cpuBefore.length !== scene.cpuAfter.length || [...scene.cpuBefore, ...scene.cpuAfter].some(n => typeof n !== 'number' || !Number.isFinite(n) || n < 0)) throw new Error(`Scene ${index} CPU arrays must be non-empty, same-length arrays of finite non-negative numbers.`);
    if (!Array.isArray(scene.fpsBefore) || !Array.isArray(scene.fpsAfter) || !scene.fpsBefore.length || scene.fpsBefore.length !== scene.fpsAfter.length || scene.fpsBefore.length !== scene.cpuBefore.length || scene.fpsAfter.length !== scene.cpuAfter.length || [...scene.fpsBefore, ...scene.fpsAfter].some(n => typeof n !== 'number' || !Number.isFinite(n) || n < 0)) throw new Error(`Scene ${index} FPS arrays must be non-empty, match their CPU arrays, and contain finite non-negative FPS values.`);
  }
}
function escapeHtml(value) { return String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c])); }
