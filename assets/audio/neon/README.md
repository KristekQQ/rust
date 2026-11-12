# Neon sound bank

Original deterministic procedural sounds; no third-party recordings or samples.
Regenerate with `python3 assets/audio/neon/generate.py`. Optional `--preview`
creates a concatenated audition and timeline; these preview artifacts are not
required runtime assets and should be moved to output/ after auditioning.

16 mono PCM16 / 48kHz short clips (about 594 KiB total). Rust gameplay chooses
variants, positions and gains. Strong cuts add a low body and granular splash.
Manifest SHA-256 values bind content. Actual subjective quality/Quest audibility
requires headset listening; automated PCM checks do not establish enjoyment.

Runtime currently uses cuts, swings, combo, bank, final countdown and round
start/end. Spawn/miss/UI clips are available for authoring, not emitted by default.
