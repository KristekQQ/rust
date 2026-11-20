# Neon sound bank

Original deterministic procedural sounds; no third-party recordings or samples.
Regenerate with `python3 assets/audio/neon/generate.py`. Optional `--preview`
creates a concatenated audition and timeline; these preview artifacts are not
required runtime assets and should be moved to output/ after auditioning.

20 mono PCM16 / 48kHz clips (about 1.3 MiB total). Rust gameplay chooses
variants, positions and gains. Strong cuts add a low body and granular splash.
Manifest SHA-256 values bind content. Actual subjective quality/Quest audibility
requires headset listening; automated PCM checks do not establish enjoyment.

Runtime currently uses cuts, swings, combo, bank, final countdown and round
start/end. Spawn/miss/UI clips are available for authoring, not emitted by default.

## Original music

`neon_bar_intro`, `neon_bar_drive`, `neon_bar_final`, `neon_bar_break` are exact 96,000-sample
(2.000s) 120 BPM 4/4 bars in E minor/Em9. A downbeat kick marks beat zero;
kicks repeat each quarter, snares on beats 2/4. Revision 2 uses a detuned, driven reese bass with syncopated pulses, dry
metallic snares and ring-modulated machine hats. A cold FM call-and-answer
descends E-D-B-G / B-A-F#-E; the intro leaves space and the final adds bass
subdivisions and FM density. Each music manifest entry carries `musicRevision: 2`. All share compatible harmony, tempo and sample endpoints.
The contrasting break leaves half-time space for a chopped FM radio phrase, then
adds a tightening metallic snare pickup into the next section. All four music
entries explicitly route through `bus: "music"`.
The original additive synthesis and seeded percussion use no external samples.

`python3 assets/audio/neon/generate_music.py --preview` regenerates only music
and its manifest entries, plus `output/neon-music-preview.wav`: a continuous
30-second audition (8 seconds per section, each followed by a 2-second break). The main `generate.py` also includes
music, so rebuilding effects preserves these registrations. No Python packages
are required. Music uses fixed mastering gain, preserving section dynamics.

Play each bar once at pitch 1 on the authoritative two-second musical grid;
there is no leading count-in or looping baked into the files. Non-spatial music
should use the music bus. `suggestedGain` is authoring metadata; event gain must
be supplied by the caller. Native embeds the same WAV bytes; browser loads the
manifest. Current native WAV limits are mono PCM16/48kHz, ten seconds per clip,
24 simultaneous voices; the bars fit these limits. This bank does not implement
sample-accurate scheduling or prove headset listening quality.
