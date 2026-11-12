---
name: quest-hardware-profiling
description: Measure renderer changes directly on physical Quest hardware using the native GPU probe, with reproducible A/B runs and app restoration. Use for Quest performance evidence; distinguish frozen GPU time from live OpenXR FPS and gameplay CPU timing.
---

# Quest hardware profiling

Use the existing [hardware runner](../../../scripts/quest-hardware-probe.py) for
controlled native Neon GPU comparisons. Use [frame-pipeline](../frame-pipeline/SKILL.md)
for timing boundaries and [quest-stream](../quest-stream/SKILL.md) for Wi-Fi/awake
sessions. This workflow temporarily backgrounds Neon and restores it; a request
for hardware testing authorizes that work. A code-only review does not require a
device takeover. Preserve existing task authorization instead of asking again.

## Choose the evidence

- **Frozen physical GPU probe:** Android AArch64 executable runs the actual scene
  and renderer on Quest Vulkan, with fixed cameras and two eyes. GPU timestamps
  measure rendering work. It is not OpenXR, headset FPS, motion latency or a CPU
  physics benchmark. Never present `1000 / gpu_ms` as measured FPS.
- **Live OpenXR:** use the owning checkout's `native/neon/scripts/measure.py`
  recorder/replay when live frame pacing is the question. Match camera/input,
  scene, refresh rate, quality, capture load and installed APK identity. Off-face
  tracking failures invalidate normal-player comparisons.
- **Physics:** isolate simulation/contact/cutting/publication timings. The
  aggregate worker/game duration includes more than physics; do not relabel it.

## Establish the session

Locate the current source checkout and task record first: the project root may
lack the active `native/neon` tree. Do not build an older worktree just because it
has a familiar name. Preserve other source edits, APKs and capture sessions.

Use the SDK `platform-tools/adb` if `adb` is absent from PATH. Check `devices -l`
and the actual model. With USB, read `shell ip -f inet addr show wlan0`, enable
`tcpip 5555` if needed, connect the verified address, then verify the model over
that Wi-Fi serial before telling the user to unplug. An unchanged IP does not
prove TCP debugging survived a headset restart. Carry the selected ADB server
port into every command; never restart unrelated servers or forwards.

Keep one existing awake helper. The explicit `--keep-app ... --follow-app` mode
survives package process restarts with a 30-second grace period. It is a laptop
session, not a permanent device setting. Off-face **Awake** and valid room
tracking are separate. Preserve Quest Stream; record any active capture load.

## Build and run

Build a fresh **Android** release `neon-preview`, not the desktop executable.
Select the installed NDK path; on this macOS workflow the build invocation is:

```sh
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$QUEST_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android29-clang" \
CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384' \
cargo build --manifest-path native/neon/Cargo.toml --locked \
  --target-dir output/native-neon-target --target aarch64-linux-android \
  --release --bin neon-preview

python3 scripts/quest-hardware-probe.py --adb "$QUEST_ADB" \
  --adb-port "$QUEST_ADB_PORT" --serial "$QUEST_WIFI_SERIAL" \
  --binary output/native-neon-target/aarch64-linux-android/release/neon-preview \
  --assets native/neon/assets --out output/measurement/quest-glow-NEW_RUN
```

The runner defaults to three paired runs in AB/BA/AB order, 300 measured frames,
60 warmup frames and 1728×1728 per eye. Baseline explicitly passes
`--no-font-glow-atlas`; the candidate uses the native default. Neither disables
ordinary glow or SSR. The executable, assets and output metadata are validated;
unknown settings, absent timestamps or a candidate that never draws atlas glyphs
must not become a successful benchmark. The explicit `--experiment half-glow`
comparison keeps the glyph atlas enabled in both variants and selects ordinary
halo resolution 1 versus 0.5; it rejects an inactive/fallback half-resolution
path. `--view cut` uses the real cut fixture for paired stereo quality checks.
Include the new depth-reduction and halo passes in whole-pass GPU totals.
The existence of this experiment is not evidence that it should be enabled by
default. Change variants deliberately in code and validation for other effects.

Avoid stale Cargo evidence: preserve content hashes, and refresh mtime when
restoring source copies. A successful build command alone is not proof that an
old copied executable contains current shaders. Keep the tested binary hash.

## Interpret and finish

Sum only `gpu.passesMs`, once per unique sampled frame. Nested sections are
already part of these passes. Compare per-run medians and paired changes; expose
mixed signs and timing spread. Record battery, temperature and GPU-clock samples;
before/after samples do not lock clocks or exclude transient throttling.

Inspect both-eye images and repeated-run determinism. For a visual approximation,
state what changed; for quality-preserving changes require matching pixels.
Keep raw logs, hashes, settings, timing JSON and restoration evidence. On errors,
retain the failed run, terminate only its PID after verifying the exact owned
executable, and restore the app. If process ownership/exit cannot be verified,
report that cleanup and restoration remain unresolved. Do not overwrite
scores, reinstall an APK, stop capture or change power/thermal policies to make a
benchmark pass. If restoration fails, report the actual current device state.
