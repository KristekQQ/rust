"""Original deterministic procedural Neon Arena effects. Python standard library only."""
import array
import hashlib
import json
import math
from pathlib import Path
import random
import sys
import wave

ROOT = Path(__file__).resolve().parent
SR = 48000
TAU = 2 * math.pi
manifest = []
clips = {}


def render(name, label, duration, kind, seed=1, strength=1, notes=(), spatial=True, gain=.65):
    rng = random.Random(seed)
    samples = []
    low = 0.0
    phase = 0.0
    for i in range(round(SR * duration)):
        t = i / SR
        n = rng.uniform(-1, 1)
        low += .13 * (n - low)
        bright = n - low
        if kind == 'cut':
            phase += TAU * (160 + 1850 * math.exp(-t * 43)) / SR
            x = (.52 * bright * math.exp(-t * 26)
                 + .38 * math.sin(phase) * math.exp(-t * 32)
                 + .38 * low * math.exp(-t * 12)
                 + .09 * math.sin(TAU * 2900 * t) * math.exp(-t * 22))
            # Strong cuts have a lower body and a short granular splash, not just more gain.
            if strength > 1:
                x += .30 * math.sin(TAU * (105*t + 1.8*(1-math.exp(-t*24)))) * math.exp(-t*19)
                x += .20 * low * (0.55 + 0.45*math.sin(TAU*77*t)) * math.exp(-t*16)
            x *= strength
        elif kind == 'swing':
            envelope = math.sin(math.pi * t / duration) ** 2
            x = (low * 1.8 + bright * .12) * envelope
        elif kind == 'spawn':
            phase += TAU * (300 + 1500 * t / duration) / SR
            x = .28 * math.sin(phase) * math.sin(math.pi * t / duration) ** 2 + .10 * low
        elif kind == 'miss':
            phase += TAU * (260 - 160 * t / duration) / SR
            x = .38 * math.sin(phase) * math.exp(-t * 11) + .18 * low * math.exp(-t * 16)
        else:
            x = 0
            for start, frequency, amplitude in notes:
                age = t - start
                if age >= 0:
                    env = min(1, age / .006) * math.exp(-age * 8)
                    x += amplitude * env * (math.sin(TAU * frequency * age)
                         + .18 * math.sin(TAU * frequency * 2 * age))
        fade = min(1, t / .002) * min(1, max(0, (duration - 1 / SR - t) / .025))
        samples.append(x * fade)
    peak = max(abs(v) for v in samples)
    scale = min(1, .66 / max(peak, 1e-9))
    samples = [v * scale for v in samples]
    save(name, samples)
    clips[name] = samples
    manifest.append(dict(id=name, label=label, file=name + '.wav', durationSeconds=duration,
        spatial=spatial, loop=False, suggestedGain=gain, variantsGroup=name.rsplit('_', 1)[0]
        if name[-1:].isdigit() else name, peakDbFS=round(20 * math.log10(max(abs(v) for v in samples)), 2),
        sha256=hashlib.sha256((ROOT / (name + '.wav')).read_bytes()).hexdigest()))


def save(name, samples):
    pcm = array.array('h', (round(max(-1, min(1, v)) * 32767) for v in samples))
    if sys.byteorder != 'little':
        pcm.byteswap()
    with wave.open(str(ROOT / (name + '.wav')), 'wb') as f:
        f.setparams((1, 2, SR, 0, 'NONE', 'not compressed'))
        f.writeframes(pcm.tobytes())


for k in range(1, 4):
    render(f'fruit_cut_soft_{k}', f'Jemný řez {k}', .24, 'cut', 100+k, .65, gain=.7)
    render(f'fruit_cut_hard_{k}', f'Silný řez {k}', .32, 'cut', 200+k, 1.25, gain=.7)
for k in range(1, 3):
    render(f'blade_swing_{k}', f'Švih {k}', .22 + k * .035, 'swing', 300+k, gain=.4)
render('fruit_spawn', 'Objevení ovoce', .18, 'spawn', gain=.25)
render('fruit_miss', 'Minuté ovoce', .26, 'miss', gain=.35, spatial=False)
render('combo_triple', 'Dokončené kombo', .70, 'tone', notes=((0,880,.24),(.08,1108.73,.22),(.16,1318.51,.26)), spatial=False)
render('chain_bank', 'Potvrzení bonusu', .65, 'tone', notes=((0,659.25,.22),(.07,987.77,.24),(.14,1318.51,.22)), spatial=False)
render('countdown_tick', 'Odpočet', .16, 'tone', notes=((0,660,.27),), spatial=False)
render('round_start', 'Začátek kola', .85, 'tone', notes=((0,440,.23),(.10,659.25,.24),(.20,880,.26)), spatial=False)
render('round_end', 'Konec kola', 1.1, 'tone', notes=((0,880,.21),(.14,659.25,.22),(.28,440,.26)), spatial=False)
render('ui_confirm', 'Potvrzení v menu', .20, 'tone', notes=((0,880,.20),(.045,1320,.16)), spatial=False)

# A spaced playlist, then a short illustrative gameplay mix. No runtime dependencies.
demo = []
timeline = []
for item in manifest:
    timeline.append({'seconds': round(len(demo)/SR, 2), 'id': item['id'], 'label': item['label']})
    demo.extend(clips[item['id']])
    demo.extend([0.] * round(.32 * SR))
start = len(demo)/SR
mix = [0.] * (7 * SR)
for when, name, gain in [(0,'countdown_tick',.6),(.5,'countdown_tick',.6),(1,'round_start',.65),
    (1.6,'blade_swing_1',.5),(1.72,'fruit_cut_soft_1',.8),(2.2,'blade_swing_2',.5),
    (2.32,'fruit_cut_hard_1',.8),(2.8,'fruit_cut_hard_2',.8),(2.83,'combo_triple',.55),
    (3.4,'fruit_cut_soft_2',.8),(3.85,'fruit_cut_hard_3',.8),(4.3,'chain_bank',.5),
    (5.3,'round_end',.65)]:
    offset = round(when*SR)
    for j, value in enumerate(clips[name]):
        mix[offset+j] += value*gain
assert max(map(abs, mix)) < .95
demo.extend(mix)
timeline.append({'seconds':round(start,2), 'id':'gameplay_demo', 'label':'Ukázková sekvence hry'})
if '--preview' in sys.argv:
    save('preview', demo)
(ROOT/'manifest.json').write_text(json.dumps({'version':1,'sampleRate':SR,'channels':1,'encoding':'PCM_S16LE',
    'provenance':'Original procedural synthesis; no third-party recordings or samples.',
    'sounds':manifest},ensure_ascii=False,indent=2)+'\n')
if '--preview' in sys.argv:
    (ROOT/'preview-timeline.json').write_text(json.dumps(timeline,ensure_ascii=False,indent=2)+'\n')

# Verify exported bytes, including silence at both edges and peak headroom.
for path in ROOT.glob('*.wav'):
    with wave.open(str(path),'rb') as f:
        assert (f.getnchannels(),f.getsampwidth(),f.getframerate()) == (1,2,SR)
        data = array.array('h', f.readframes(f.getnframes()))
        if sys.byteorder != 'little': data.byteswap()
        assert data[0] == 0 and data[-1] == 0, path.name
        assert 0 < max(map(abs,data)) < 31129, path.name
print(json.dumps({'effects':len(manifest),'effectBytes':sum((ROOT/i['file']).stat().st_size for i in manifest),
    'previewSeconds':round(len(demo)/SR,2),'verification':'PCM headers, non-silence, peak headroom, zero endpoints PASS'}))
