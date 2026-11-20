"""Original 120 BPM / E minor industrial cyberpunk score, revision 2. No external samples/dependencies.

Called by generate.py. Also runnable alone to replace only music manifest entries.
Attack-aligned 2-second bars share Em9 harmony and one fixed mastering gain.
"""
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
FRAMES = 2 * SR
TAU = math.tau


def hz(midi):
    return 440 * 2 ** ((midi - 69) / 12)


def synth_bar(level):
    out = [0.] * FRAMES
    rng = random.Random(0x43594252 + level)

    def hit(start, duration, sound):
        offset = round(start * SR)
        for i in range(min(round(duration * SR), FRAMES - offset)):
            out[offset + i] += sound(i / SR)

    # Dry industrial four-on-the-floor: clear low punch plus a clipped machine tick.
    for beat in range(4):
        hit(beat * .5, .31, lambda t: .46 * math.sin(TAU *
            (46 * t + 4.9 * (1 - math.exp(-t * 42)))) * math.exp(-t * 17)
            + .048 * math.tanh(3 * math.sin(TAU * 1900 * t)) * math.exp(-t * 190))
    for start in (.5, 1.5):
        hit(start, .17, lambda t: (.067 + level * .013) * min(1, t / .0004) * (
            .85 * rng.uniform(-1, 1) * math.exp(-t * 29)
            + .65 * math.sin(TAU * 172 * t + 1.8 * math.sin(TAU * 287 * t)) * math.exp(-t * 24)
            + .32 * math.sin(TAU * 1471 * t) * math.sin(TAU * 2137 * t) * math.exp(-t * 42)))
    # Inharmonic ring-modulated clockwork, deliberately short to leave room for cuts.
    division = (8, 16, 16)[level]
    for step in range(division):
        gain = (.020 + level * .005) * (1 if step % 2 else .48)
        hit(step * 2 / division, .048, lambda t, g=gain: g * min(1, t / .0003) * (
            math.sin(TAU * 6143 * t) * math.sin(TAU * 8731 * t)
            + .35 * rng.uniform(-1, 1)) * math.exp(-t * 110))
    # Detuned, softly driven reese. Finite harmonic bank avoids aliased saw edges.
    # The syncopation leaves the kick exposed; every section shares the E-minor bass.
    patterns = (
        [(0,40,.22),(.375,40,.10),(.75,38,.18),(1,40,.25),(1.625,35,.24)],
        [(0,40,.18),(.375,40,.10),(.625,40,.20),(.875,38,.10),
         (1,40,.19),(1.375,47,.10),(1.625,38,.12),(1.875,35,.11)],
        [(0,40,.18),(.25,40,.09),(.375,47,.10),(.625,40,.20),(.875,38,.10),
         (1,40,.18),(1.25,40,.09),(1.375,47,.10),(1.625,38,.12),(1.875,35,.11)])
    for start, note, length in patterns[level]:
        f = hz(note)
        def reese(t, f=f, length=length, start=start):
            a = TAU * f * t
            harmonics = sum(math.exp(-k * .25) / k * (
                math.sin(k * a * .991) + math.sin(k * a * 1.009)) for k in range(1, 10))
            env = min(1, t / .002) * min(1, (length - t) / .018)
            duck = .24 + .76 * (1 - math.exp(-((t + start) % .5) * 27))
            return (.117 + level * .014) * env * duck * math.tanh(harmonics * 1.8)
        hit(start, length, reese)
    # Cold drone rather than a warm chord wash: E/B with a faint beating ninth.
    for i in range(FRAMES):
        t = i / SR
        env = min(1, t / .025, (2 - 1 / SR - t) / .045)
        duck = .25 + .75 * (1 - math.exp(-(t % .5) * 20))
        out[i] += .020 * env * duck * (math.sin(TAU * hz(52) * t)
            + .45 * math.sin(TAU * hz(59) * t + .4 * math.sin(TAU * 1.7 * t))
            + .23 * math.sin(TAU * hz(66) * t))
    # FM transmission call/answer: descending E-D-B-G, then a lower B-A-F#-E reply.
    # Intro leaves space; drive/final expose the complete original dystopian motif.
    phrases = (
        [(0,76),(.375,71),(1,67),(1.375,64)],
        [(0,76),(.25,74),(.375,71),(.625,67),(1,71),(1.25,69),(1.375,66),(1.625,64)],
        [(0,76),(.25,74),(.375,71),(.625,67),(.875,66),
         (1,71),(1.25,69),(1.375,66),(1.625,64),(1.875,59)])
    lead = [0.] * FRAMES
    for start, note in phrases[level]:
        offset = round(start * SR)
        f = hz(note)
        length = .19
        for i in range(min(round(length * SR), FRAMES - offset)):
            t = i / SR
            env = min(1, t / .0015) * math.exp(-t * 19) * min(1, (length - t) / .015)
            # Noninteger modulator creates an unmistakable sci-fi bell/radio edge.
            mod = (2.7 + level * .4) * math.exp(-t * 21) * math.sin(TAU * f * 2.013 * t)
            lead[offset + i] += (.052 + level * .009) * env * math.sin(TAU * f * t + mod)
    for i in range(FRAMES):
        out[i] += lead[i] + .20 * lead[(i - 18000) % FRAMES] + .07 * lead[(i - 36000) % FRAMES]
        out[i] *= .88 * min(1, i / 24, (FRAMES - 1 - i) / 144)
    return out

def synth_break():
    """Sparse radio interruption, half-time weight, then a sixteenth-note pickup."""
    out = [0.] * FRAMES
    rng = random.Random(0x425245414B)

    def hit(start, length, sound):
        offset = round(start * SR)
        for i in range(min(round(length * SR), FRAMES - offset)):
            out[offset + i] += sound(i / SR)

    for start in (0, 1):
        hit(start, .30, lambda t: .43 * math.sin(TAU *
            (46 * t + 4.9 * (1 - math.exp(-t * 42)))) * math.exp(-t * 17))
    # Half-time snare followed by a tightening industrial pickup to the next bar.
    for start, gain in ((1,.09), (1.5,.045), (1.75,.065), (1.875,.085)):
        hit(start, .115, lambda t, g=gain: g * min(1, t / .0004) *
            (rng.uniform(-1, 1) + .65 * math.sin(TAU * 172 * t +
             2 * math.sin(TAU * 287 * t))) * math.exp(-t * 35))
    # Chopped narrow-band radio voice, an original FM phrase with silent answers.
    for start, note in ((.125,76),(.375,74),(.625,71),(1.25,67),(1.625,66)):
        f = hz(note)
        hit(start, .16, lambda t, f=f: .063 * min(1, t / .002) *
            min(1, (.16 - t) / .015) * math.exp(-t * 13) *
            (.65 + .35 * math.sin(TAU * 37 * t)) *
            math.sin(TAU * f * t + 3.5 * math.exp(-t * 18) * math.sin(TAU * f * 2.013 * t)))
    # A low E engine pulse and short descending scanner identify this as a break.
    hit(0, .45, lambda t: .08 * min(1, t / .003) * math.exp(-t * 9) *
        math.tanh(2 * math.sin(TAU * hz(40) * t + .5 * math.sin(TAU * hz(40) * 1.013 * t))))
    hit(.78, .14, lambda t: .018 * math.sin(TAU * (2700 * t - 4500 * t * t)) *
        math.sin(math.pi * t / .14) ** 2)
    for i in range(FRAMES):
        out[i] *= .88 * min(1, i / 24, (FRAMES - 1 - i) / 144)
    return out


def generate(save, clips):
    entries = []
    for level, (section, label) in enumerate((('intro', 'Neon: úvod'),
            ('drive', 'Neon: rozjezd'), ('final', 'Neon: finále'), ('break', 'Neon: rádiový zlom'))):
        name = 'neon_bar_' + section
        samples = synth_break() if section == 'break' else synth_bar(level)
        assert len(samples) == FRAMES and max(map(abs, samples)) < .85
        save(name, samples)
        clips[name] = samples
        entries.append(dict(id=name, label=label, file=name + '.wav', durationSeconds=2.0,
            spatial=False, loop=False, bus='music', suggestedGain=.65, variantsGroup='neon_bar',
            bpm=120, beatsPerBar=4, key='E minor', musicRevision=2, style='industrial cyberpunk',
            peakDbFS=round(20 * math.log10(max(map(abs, samples))), 2),
            sha256=hashlib.sha256((ROOT / (name + '.wav')).read_bytes()).hexdigest()))
    return entries


if __name__ == '__main__':
    def save(name, samples):
        pcm = array.array('h', (round(v * 32767) for v in samples))
        if sys.byteorder != 'little':
            pcm.byteswap()
        with wave.open(str(ROOT / (name + '.wav')), 'wb') as f:
            f.setparams((1, 2, SR, 0, 'NONE', 'not compressed'))
            f.writeframes(pcm.tobytes())
    clips = {}
    entries = generate(save, clips)
    manifest_path = ROOT / 'manifest.json'
    manifest = json.loads(manifest_path.read_text())
    manifest['sounds'] = [s for s in manifest['sounds'] if not s['id'].startswith('neon_bar_')] + entries
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    if '--preview' in sys.argv:
        # 8s per section plus contrasting 2s pickups; runtime bank stays uncluttered.
        order = [name for section in ('intro', 'drive', 'final')
                 for name in [section] * 4 + ['break']]
        preview = [v for section in order for v in clips['neon_bar_' + section]]
        destination = ROOT.parents[2] / 'output' / 'neon-music-preview.wav'
        destination.parent.mkdir(parents=True, exist_ok=True)
        pcm = array.array('h', (round(v * 32767) for v in preview))
        if sys.byteorder != 'little':
            pcm.byteswap()
        with wave.open(str(destination), 'wb') as f:
            f.setparams((1, 2, SR, 0, 'NONE', 'not compressed'))
            f.writeframes(pcm.tobytes())
        print(destination)
    print(json.dumps({'musicBars': len(entries), 'bpm': 120, 'secondsPerBar': 2}))
