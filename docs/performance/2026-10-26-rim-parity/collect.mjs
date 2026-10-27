#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import zlib from 'node:zlib';

const usage = 'Usage: node collect.mjs OUTPUT_ROOT REPORT_DIR';
const args = process.argv.slice(2);
if (args.length !== 2) {
  console.error(usage);
  process.exit(2);
}

const outputRoot = path.resolve(args[0]);
const reportDir = path.resolve(args[1]);
const scenarios = [
  { id: 'neutral-final-over', mode: 'Řím při vyšším rozlišení', expectedPairs: 3, active: false },
  { id: 'neutral-final-three', mode: 'Původní Three.js · diagnostické srovnání', expectedPairs: 3, three: true },
  { id: 'neutral-final-active-wg', mode: 'Aktivní scéna · WebGPU', expectedPairs: 3, active: true },
  { id: 'neutral-final-active-gl', mode: 'Aktivní scéna · WebGL', expectedPairs: 3, active: true },
  { id: 'neutral-final-wg', mode: 'Klidová scéna · WebGPU kontrola', expectedPairs: 1, active: false },
  { id: 'neutral-final-gl', mode: 'Klidová scéna · WebGL2 kontrola', expectedPairs: 1, active: false },
];

const inputHashes = [];
const included = [];
const missing = [];
const incomplete = [];
const evidenceLinks = [];
const scenes = [];
const fingerprints = {};

for (const spec of scenarios) {
  const sourceDir = path.join(outputRoot, spec.id);
  const reportPath = path.join(sourceDir, 'report.json');
  if (!fs.existsSync(reportPath)) {
    missing.push(spec.id);
    continue;
  }

  const reportBytes = fs.readFileSync(reportPath);
  const report = JSON.parse(reportBytes.toString('utf8'));
  const changes = report.driverInputsChangedDuringRun;
  const hasInputChanges = Array.isArray(changes) ? changes.length > 0 : changes === true;
  const hasServerErrors = Object.values(report.serverErrors || {}).some((errors) => Array.isArray(errors) ? errors.length > 0 : errors === true || (typeof errors === 'string' && errors.trim().length > 0));
  const hasReportError = typeof report.error === 'string' ? report.error.trim().length > 0 : Boolean(report.error);
  if (hasReportError || hasInputChanges || hasServerErrors) {
    incomplete.push({ id: spec.id, reason: 'Report obsahuje chybu nebo změnu vstupů během běhu.' });
    continue;
  }
  const results = report.results || [];
  const beforeFingerprint = report.baselineBuildFingerprint || null;
  const afterFingerprint = report.candidateBuildFingerprint || null;
  const isThree = Boolean(spec.three || report.threeReference);
  const beforeRows = results.filter((row) => row.label === 'baseline').sort(byPair);
  const afterRows = results.filter((row) => row.label === 'candidate').sort(byPair);
  const expectedPairs = spec.expectedPairs ?? 3;
  if (report.settings?.pairs !== expectedPairs || beforeRows.length !== expectedPairs || afterRows.length !== expectedPairs) {
    incomplete.push({ id: spec.id, reason: `Očekává se ${expectedPairs} kompletních párů.` });
    continue;
  }

  const beforeFps = beforeRows.map(fpsFromMeanRaf);
  const afterFps = afterRows.map(fpsFromMeanRaf);
  const beforeCpu = beforeRows.map((row) => requiredNumber(row.rafCallbackCpu?.median, `${spec.id} baseline CPU median`));
  const afterCpu = afterRows.map((row) => requiredNumber(row.rafCallbackCpu?.median, `${spec.id} candidate CPU median`));
  const comparison = (report.comparisons || []).find((item) => Array.isArray(item.paired));
  const pixelByPair = new Map((comparison?.paired || []).map((pair) => [pair.pair, pair.pixel]));
  const pairs = beforeRows.map((before, index) => {
    const after = afterRows[index];
    if (before.pair !== after.pair) throw new Error(`${spec.id}: baseline/candidate pair order differs.`);
    return {
      pair: before.pair,
      beforeFps: beforeFps[index],
      afterFps: afterFps[index],
      beforeCpu: beforeCpu[index],
      afterCpu: afterCpu[index],
      beforeFrameP95: requiredNumber(before.rafInterval?.p95, `${spec.id} baseline frame P95`),
      afterFrameP95: requiredNumber(after.rafInterval?.p95, `${spec.id} candidate frame P95`),
      beforeFrameP99: requiredNumber(before.rafInterval?.p99, `${spec.id} baseline frame P99`),
      afterFrameP99: requiredNumber(after.rafInterval?.p99, `${spec.id} candidate frame P99`),
    };
  });

  const pixel = summarizePixels(spec, beforeRows, pixelByPair, isThree);
  const beforeLabel = buildLabel(beforeFingerprint, 'Původní checkpoint');
  const afterLabel = isThree ? 'Původní Three.js' : candidateLabel(afterFingerprint);
  const evidenceDir = path.join(reportDir, 'evidence', spec.id);
  fs.mkdirSync(evidenceDir, { recursive: true });
  const reportOut = path.join(evidenceDir, 'report.json.gz');
  fs.writeFileSync(reportOut, zlib.gzipSync(reportBytes, { mtime: 0 }));
  recordInput(reportPath, reportBytes, reportOut, spec.id, 'report');

  const rawPath = path.join(sourceDir, 'raw.json.gz');
  let rawCopied = false;
  if (fs.existsSync(rawPath)) {
    const rawBytes = fs.readFileSync(rawPath);
    const rawOut = path.join(evidenceDir, 'raw.json.gz');
    fs.copyFileSync(rawPath, rawOut);
    recordInput(rawPath, rawBytes, rawOut, spec.id, 'raw');
    rawCopied = true;
  }

  const images = {};
  for (const [side, row] of [['before', beforeRows[0]], ['after', afterRows[0]]]) {
    const file = row.capture?.file;
    if (!file) continue;
    const imagePath = path.join(sourceDir, file);
    if (!fs.existsSync(imagePath)) continue;
    const imageBytes = fs.readFileSync(imagePath);
    const imageOut = path.join(evidenceDir, `${side}-pair0.png`);
    fs.copyFileSync(imagePath, imageOut);
    recordInput(imagePath, imageBytes, imageOut, spec.id, `image-${side}`);
    images[side] = path.relative(reportDir, imageOut).split(path.sep).join('/');
  }

  const gpuBefore = median(beforeRows.map((row) => gpuMs(row, report.settings?.backend)).filter(isNumber));
  const gpuAfter = median(afterRows.map((row) => gpuMs(row, report.settings?.backend)).filter(isNumber));
  const gpuScope = gpuBefore === null && gpuAfter === null
    ? 'Čas GPU není v těchto reportech dostupný.'
    : report.settings?.backend === 'webgpu'
      ? 'GPU timestamp hlavního passu scene.main; samostatný rozsah podle reportu.'
      : 'EXT timer interval RAF; rozsah podle reportu, nejde o celý GPU frame.';

  scenes.push({
    id: spec.id,
    backend: report.settings?.backend === 'webgpu' ? 'WebGPU' : 'WebGL2',
    mode: `${spec.mode} · ${report.viewport?.width || '?'} × ${report.viewport?.height || '?'}`,
    labels: { before: beforeLabel, after: afterLabel },
    builds: {
      before: buildDescription(beforeFingerprint, beforeLabel),
      after: isThree ? describeThreeReference(report.threeReference) : buildDescription(afterFingerprint, afterLabel),
    },
    caption: isThree
      ? 'Diagnostické srovnání s původním Three.js. Rozdílné vyhlazení, shadery a stíny znamenají, že nejde o pixelovou bránu ani důkaz obecné parity.'
      : spec.active
        ? 'Aktivní scéna s pohybující se simulací. Běhy nemají deterministicky shodný stav pro přesnou pixelovou kontrolu.'
        : 'Stejná testovací konfigurace a rozlišení v uvedených bězích. Výsledek platí pro tuto scénu a tuto konfiguraci.',
    cpuBefore: beforeCpu,
    cpuAfter: afterCpu,
    cpuRatio: ratioOfMedians(beforeCpu, afterCpu),
    fpsBefore: beforeFps,
    fpsAfter: afterFps,
    fpsRatio: ratioOfMedians(beforeFps, afterFps),
    gpuBefore,
    gpuAfter,
    gpuScope,
    pairs,
    images,
    pixel,
    caveats: [
      'FPS se počítá pro každý běh jako 1 000 / průměrný RAF interval v ms; limit 60 Hz může výsledek zastropovat.',
      ...(spec.active ? ['Aktivní simulace nemá deterministicky resetovaný capture stav; pixelový výsledek se nehodnotí.'] : []),
      ...(isThree ? ['Původní Three.js má jiné vyhlazení a shoda shaderů/stínů nebyla prokázána.'] : []),
    ],
  });

  fingerprints[spec.id] = { before: beforeFingerprint, after: isThree ? report.threeReference || null : afterFingerprint };
  included.push(spec.id);
  evidenceLinks.push({ label: `${spec.id}: měřicí report`, href: `./evidence/${spec.id}/report.json.gz` });
  if (rawCopied) evidenceLinks.push({ label: `${spec.id}: surová data`, href: `./evidence/${spec.id}/raw.json.gz` });
}

const first = scenes.length ? JSON.parse(fs.readFileSync(path.join(outputRoot, included[0], 'report.json'), 'utf8')) : null;
const baselineBuild = first?.baselineBuildFingerprint || null;
const candidateBuild = first?.candidateBuildFingerprint || baselineBuild;
  const limits = [
  'Měření se vztahuje jen na uvedené scény, buildy a rozlišení; obecnou obrazovou paritu ani výkon všech scén nedokazuje.',
  '60 Hz může omezit naměřené FPS; dosažený limit sám o sobě neměří rezervu výkonu.',
  'Jde o desktopová měření. Fyzický Quest ani jeho výkon zde ověřen nebyl.',
  'Pixelová brána se agreguje přes dostupné páry. Three.js je diagnostické srovnání bez pixelové brány; aktivní scény nemají přesný replay.',
  ...(missing.length ? [`Chybí reporty: ${missing.join(', ')}.`] : []),
  ...(incomplete.length ? [`Nedokončené reporty byly vynechány: ${incomplete.map((item) => item.id).join(', ')}.`] : []),
];
fs.mkdirSync(reportDir, { recursive: true });
const manifest = {
  schema: 1,
  outputRoot: path.relative(reportDir, outputRoot),
  included,
  missing,
  incomplete,
  buildFingerprints: fingerprints,
  inputs: inputHashes,
};
fs.writeFileSync(path.join(reportDir, 'collector-manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
if (!scenes.length) throw new Error('No complete scenario reports found; see collector-manifest.json for missing IDs.');
const viewData = {
  title: 'Řím: pokrok našeho rendereru a srovnání s Three.js',
  subtitle: first ? `${first.browser || 'Prohlížeč neuveden'} · ${first.hardware?.devices?.[0]?.deviceString || 'GPU neuvedeno'} · desktopová měření` : 'Čeká se na měřicí reporty.',
  verdict: (() => {
    const stress = scenes.find(s => s.id === 'neutral-final-over');
    const three = scenes.find(s => s.id === 'neutral-final-three');
    const format = a => median(a).toLocaleString('cs-CZ', { maximumFractionDigits: 2 });
    if (!stress || !three) return 'Výsledky zatím nejsou kompletní; nelze uzavřít srovnání.';
    return `Náš renderer: ${format(stress.fpsBefore)} → ${format(stress.fpsAfter)} FPS ve stejném náročném pohledu. Přímý samostatný test: náš ${format(three.fpsBefore)}, původní Three.js ${format(three.fpsAfter)} FPS. Cíl stejné rychlosti a grafiky dosud splněný není; vyhlazování a detaily stínování obou rendererů nejsou sjednocené. Výkon na Questu nebyl měřen.`;
  })(),
  protocol: 'FPS každého běhu = 1 000 / průměrný RAF interval v ms. Souhrn a poměr používají medián běhů; párová tabulka uvádí FPS, CPU medián a Frame P95/P99.',
  builds: {
    before: baselineBuild ? buildDescription(baselineBuild, buildLabel(baselineBuild, 'Renderer')) : 'Build fingerprint je v jednotlivých scénářích.',
    after: candidateBuild ? buildDescription(candidateBuild, candidateLabel(candidateBuild)) : 'Build fingerprint je v jednotlivých scénářích.',
  },
  scenes,
  limits,
  evidence: evidenceLinks,
};

fs.writeFileSync(path.join(reportDir, 'view-data.json'), `${JSON.stringify(viewData, null, 2)}\n`);
console.log(JSON.stringify({ included, missing, incomplete, inputCount: inputHashes.length }));

function byPair(a, b) { return (a.pair ?? 0) - (b.pair ?? 0); }
function isNumber(value) { return typeof value === 'number' && Number.isFinite(value); }
function requiredNumber(value, label) {
  if (!isNumber(value)) throw new Error(`Missing finite metric: ${label}.`);
  return value;
}
function fpsFromMeanRaf(row) {
  const mean = requiredNumber(row.rafInterval?.mean, `${row.label} RAF mean`);
  if (mean <= 0) throw new Error('RAF mean must be positive.');
  return 1000 / mean;
}
function median(values) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}
function ratioOfMedians(before, after) {
  const a = median(before), b = median(after);
  return a && b !== null ? b / a : null;
}
function gpuMs(row, backend) {
  const sample = backend === 'webgpu' ? row.gpuPassMs?.['scene.main'] : row.glRafGpuMs;
  return isNumber(sample) ? sample : isNumber(sample?.median) ? sample.median : isNumber(sample?.mean) ? sample.mean : null;
}
function buildLabel(fingerprint, fallback) {
  const hash = fingerprint?.wasm || fingerprint?.source;
  if (!hash) return fallback;
  const short = hash.slice(0, 8);
  const checkpoints = { '7b0e83ab': 'Checkpoint 7b0', '292f27c1': 'Checkpoint 292f', 'a9975d26': 'Checkpoint a997', '9223a5e8': 'Checkpoint 9223', '669d72f1': 'Checkpoint 669d' };
  if (checkpoints[short]) return checkpoints[short];
  return `${fallback} · ${short}`;
}
function candidateLabel(fingerprint) {
  const hash = fingerprint?.wasm || fingerprint?.source;
  return hash ? `Finální kandidát a54 · ${hash.slice(0, 8)}` : 'Finální kandidát a54';
}
function buildDescription(fingerprint, label) {
  if (!fingerprint) return `${label}; build fingerprint unavailable in report.`;
  const parts = [fingerprint.wasm && `WASM SHA-256 ${fingerprint.wasm}`, fingerprint.source && `source SHA-256 ${fingerprint.source}`].filter(Boolean);
  return parts.length ? parts.join(' · ') : `${label}; build fingerprint unavailable in report.`;
}
function describeThreeReference(reference) {
  if (!reference) return 'Identita původního Three.js v reportu chybí.';
  if (typeof reference === 'string') return reference;
  return [reference.scope, reference.representation && `reprezentace: ${reference.representation}`, reference.root && `zdroj: ${reference.root}`].filter(Boolean).join(' · ');
}
function summarizePixels(spec, beforeRows, pixelByPair, isThree) {
  const pixels = beforeRows.map((row) => pixelByPair.get(row.pair)).filter(Boolean);
  const maxChannelDelta = pixels.reduce((max, item) => isNumber(item.maxChannelDelta) ? Math.max(max ?? 0, item.maxChannelDelta) : max, null);
  if (spec.active) return { pass: null, exact: null, maxChannelDelta, reason: 'Aktivní simulace nemá deterministicky shodný replay; pixelová brána není vyhodnocená.' };
  const changedPixels = pixels.map((item) => item.changedPixels).filter(isNumber);
  const changedFractions = pixels.map((item) => item.changedFraction).filter(isNumber);
  const framePixels = (pixels[0]?.width || 0) * (pixels[0]?.height || 0);
  const detail = pixelDetail(pixels, framePixels, isThree);
  if (isThree) return { pass: null, exact: pixels.length === beforeRows.length ? (pixels.every((item) => item.exact === true) ? true : pixels.some((item) => item.exact === false) ? false : null) : null, maxChannelDelta, maxChangedPixels: changedPixels.length ? Math.max(...changedPixels) : null, maxChangedFraction: changedFractions.length ? Math.max(...changedFractions) : null, reason: ['Diagnostické srovnání s původním Three.js; AA, shadery a stíny nejsou sjednocené.', detail].filter(Boolean).join(' ') };
  if (pixels.length !== beforeRows.length) return { pass: null, exact: null, maxChannelDelta, reason: 'Pixelová brána není dostupná pro všechny páry.' };
  return {
    pass: pixels.every((item) => item.pass === true) ? true : pixels.some((item) => item.pass === false) ? false : null,
    exact: pixels.every((item) => item.exact === true) ? true : pixels.some((item) => item.exact === false) ? false : null,
    maxChannelDelta,
    maxChangedPixels: changedPixels.length ? Math.max(...changedPixels) : null,
    maxChangedFraction: changedFractions.length ? Math.max(...changedFractions) : null,
    reason: [pixels.find((item) => item.reason)?.reason, detail].filter(Boolean).join(' '),
  };
}
function pixelDetail(pixels, framePixels, diagnostic) {
  if (!pixels.length) return '';
  const maxChannel = Math.max(...pixels.map((item) => item.maxChannelDelta).filter(isNumber));
  const maxChanged = Math.max(...pixels.map((item) => item.changedPixels).filter(isNumber));
  const maxFraction = Math.max(...pixels.map((item) => item.changedFraction).filter(isNumber));
  const details = [];
  if (Number.isFinite(maxChannel)) details.push(`Největší odchylka kanálu ${maxChannel}/255.`);
  if (Number.isFinite(maxChanged) && framePixels) details.push(`Nejvýše ${maxChanged} z ${framePixels} pixelů má odchylku nad tolerancí${Number.isFinite(maxFraction) ? ` (${(maxFraction * 100).toFixed(4)} %)` : ''}.`);
  if (diagnostic || pixels.some((item) => item.gateDisabled === true)) details.push('Pixelová brána je vypnutá pro diagnostické srovnání.');
  else {
    const channelLimit = pixels.find((item) => isNumber(item.channelTolerance))?.channelTolerance;
    const fractionLimit = pixels.find((item) => isNumber(item.pixelFractionTolerance))?.pixelFractionTolerance;
    if (channelLimit !== undefined && fractionLimit !== undefined) details.push(`Mez brány: ${channelLimit}/255 a ${(fractionLimit * 100).toFixed(3)} % pixelů.`);
  }
  return details.join(' ');
}
function recordInput(sourcePath, bytes, targetPath, scenario, kind) {
  inputHashes.push({
    scenario,
    kind,
    source: path.relative(outputRoot, sourcePath).split(path.sep).join('/'),
    copy: path.relative(reportDir, targetPath).split(path.sep).join('/'),
    bytes: bytes.length,
    sha256: crypto.createHash('sha256').update(bytes).digest('hex'),
    copiedBytes: fs.statSync(targetPath).size,
    copiedSha256: crypto.createHash('sha256').update(fs.readFileSync(targetPath)).digest('hex'),
  });
}
