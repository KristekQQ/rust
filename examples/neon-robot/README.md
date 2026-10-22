# Neon Sentinel

An animated, fully three-dimensional mechanical character reinterpreted from the
user's single front reference. Unseen rear surfaces and rig behavior are original
design choices. It is not a photogrammetric reconstruction.

The standalone example loads `assets/models/neon-robot/neon-robot.glb` using the
engine's existing glTF loader. Rust evaluates all animation channels; JS holds
the model handle and sends clip, speed, pause and camera commands. No JS render
loop or external 3D renderer is used.

The desktop preview mirrors X to compensate for this engine orbit view's left-handed front view; the downloaded GLB retains conventional +Z front.

The source is `assets/models/neon-robot/neon-robot.blend`; regenerate with
`build_blender.py` through Blender. The separate studio-stage asset is only for
the example and is not part of the character download.

The GLB contains `Idle`, `Attack` and `Victory` clips, PBR metal/brass and emissive
cyan/magenta surfaces. Studio lights and native bloom provide the
preview lighting. The model downloads as one self-contained GLB file.

Run targeted browser acceptance with the maintained runner's `--page neon-robot
--backend all --capture` options. The check exercises every clip, automatic Rust
playback, both neon colors and actual front/rear geometry pixels.
