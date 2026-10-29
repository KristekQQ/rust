# Executed commands and reproduction

Actual working directory: `/private/tmp/rust-render-followup-2`. Node24.19.0 was prepended from `/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin`. Commands ran serially on AC; host observations and exact argv are retained in `measurements/commands.jsonl` inside the archive. Failed tooling attempts remain in the evidence; no measurement is silently replaced.

Restore the archive to a new empty directory with its verified `RESTORE.py`. `measurements/baseline` and `measurements/final` are complete measured source/SDK/assets/release packages (historical docs omitted). From the isolated repo, map the recorded initial root to restored `measurements/baseline`, final root to `measurements/final`, and configs to their restored paths; choose a NEW `--out` for every rerun. Use current maintained scripts at runtime checkpoint08c2d212a. Do not launch against a shared server.

## Actual serial ledger

### cal-rim-camera-stress-webgpu

UTC 2026-10-29T21:29:30.377186+00:00; exit0; 20.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgpu --family rim-camera-stress --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-camera-stress-webgpu
```

### pilot-rim-camera-stress-webgpu

UTC 2026-10-29T21:29:50.516279+00:00; exit0; 30.2s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-camera-stress-webgpu/config.json --backend webgpu --family rim-camera-stress --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 5000 --pairs 1 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/pilot-rim-camera-stress-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-camera-stress-webgl

UTC 2026-10-29T21:30:20.751853+00:00; exit0; 10.0s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgl --family rim-camera-stress --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-camera-stress-webgl
```

### pilot-rim-camera-stress-webgl

UTC 2026-10-29T21:30:30.811264+00:00; exit0; 30.2s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-camera-stress-webgl/config.json --backend webgl --family rim-camera-stress --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 5000 --pairs 1 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/pilot-rim-camera-stress-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-stress-webgpu

UTC 2026-10-29T21:31:11.624543+00:00; exit0; 20.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgpu --family rim-stress --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-stress-webgpu
```

### primary-rim-stress-webgpu

UTC 2026-10-29T21:31:31.760887+00:00; exit0; 241.3s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-stress-webgpu/config.json --backend webgpu --family rim-stress --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-stress-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### primary-rim-camera-stress-webgpu

UTC 2026-10-29T21:35:33.128428+00:00; exit0; 241.2s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-camera-stress-webgpu/config.json --backend webgpu --family rim-camera-stress --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-camera-stress-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-clustered-webgpu

UTC 2026-10-29T21:39:34.341914+00:00; exit0; 20.0s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgpu --family rim-clustered --load 1024 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-clustered-webgpu
```

### primary-rim-clustered-webgpu

UTC 2026-10-29T21:39:54.343801+00:00; exit0; 231.4s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-clustered-webgpu/config.json --backend webgpu --family rim-clustered --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-clustered-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-active-webgpu

UTC 2026-10-29T21:43:45.750649+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgpu --family rim-active --load 1 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-active-webgpu
```

### primary-rim-active-webgpu

UTC 2026-10-29T21:43:55.841184+00:00; exit0; 211.2s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-active-webgpu/config.json --backend webgpu --family rim-active --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-active-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### cal-arena-active-webgpu

UTC 2026-10-29T21:47:27.102644+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgpu --family arena-active --load 1 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-arena-active-webgpu
```

### primary-arena-active-webgpu

UTC 2026-10-29T21:47:37.196229+00:00; exit0; 211.4s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-arena-active-webgpu/config.json --backend webgpu --family arena-active --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-arena-active-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-pool-cycle-webgpu

UTC 2026-10-29T21:51:08.588475+00:00; exit0; 10.0s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgpu --family rim-pool-cycle --load 1024 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-pool-cycle-webgpu
```

### primary-rim-pool-cycle-webgpu

UTC 2026-10-29T21:51:18.675678+00:00; exit0; 432.4s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-pool-cycle-webgpu/config.json --backend webgpu --family rim-pool-cycle --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 60000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-pool-cycle-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-stress-webgl

UTC 2026-10-29T21:58:31.086818+00:00; exit0; 10.0s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgl --family rim-stress --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-stress-webgl
```

### primary-rim-stress-webgl

UTC 2026-10-29T21:58:41.138857+00:00; exit0; 231.7s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-stress-webgl/config.json --backend webgl --family rim-stress --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-stress-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### primary-rim-camera-stress-webgl

UTC 2026-10-29T22:02:32.869652+00:00; exit0; 231.4s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-camera-stress-webgl/config.json --backend webgl --family rim-camera-stress --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-camera-stress-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-clustered-webgl

UTC 2026-10-29T22:06:24.277508+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgl --family rim-clustered --load 1024 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-clustered-webgl
```

### primary-rim-clustered-webgl

UTC 2026-10-29T22:06:34.366865+00:00; exit0; 221.7s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-clustered-webgl/config.json --backend webgl --family rim-clustered --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-clustered-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-active-webgl

UTC 2026-10-29T22:10:16.115557+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgl --family rim-active --load 1 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-active-webgl
```

### primary-rim-active-webgl

UTC 2026-10-29T22:10:26.204019+00:00; exit0; 211.0s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-active-webgl/config.json --backend webgl --family rim-active --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-active-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### cal-arena-active-webgl

UTC 2026-10-29T22:13:57.231339+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgl --family arena-active --load 1 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-arena-active-webgl
```

### primary-arena-active-webgl

UTC 2026-10-29T22:14:07.314554+00:00; exit0; 211.1s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-arena-active-webgl/config.json --backend webgl --family arena-active --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-arena-active-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### cal-rim-pool-cycle-webgl

UTC 2026-10-29T22:17:38.466918+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgl --family rim-pool-cycle --load 1024 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-pool-cycle-webgl
```

### primary-rim-pool-cycle-webgl

UTC 2026-10-29T22:17:48.552385+00:00; exit0; 432.3s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-pool-cycle-webgl/config.json --backend webgl --family rim-pool-cycle --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 60000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-rim-pool-cycle-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### diagnostic-rim-camera-stress-webgpu

UTC 2026-10-29T22:25:45.120398+00:00; exit0; 70.3s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-camera-stress-webgpu/config.json --backend webgpu --family rim-camera-stress --max-load 2048 --width 1280 --height 720 --warmup 2000 --sample 8000 --pairs 1 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/diagnostic-rim-camera-stress-webgpu --candidate-root output/followup-5/probe2 --profile --frame-context 1 --diagnostics --diagnostic-sample 5000
```

### diagnostic-arena-active-webgpu

UTC 2026-10-29T22:26:55.469894+00:00; exit0; 50.3s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-arena-active-webgpu/config.json --backend webgpu --family arena-active --max-load 2048 --width 1280 --height 720 --warmup 2000 --sample 8000 --pairs 1 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/diagnostic-arena-active-webgpu --candidate-root output/followup-5/probe2 --profile --frame-context 1 --diagnostics --diagnostic-sample 5000
```

### diagnostic-rim-camera-stress-webgl

UTC 2026-10-29T22:27:45.781546+00:00; exit0; 60.5s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-rim-camera-stress-webgl/config.json --backend webgl --family rim-camera-stress --max-load 2048 --width 1280 --height 720 --warmup 2000 --sample 8000 --pairs 1 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/diagnostic-rim-camera-stress-webgl --candidate-root output/followup-5/probe2 --profile --frame-context 1 --diagnostics --diagnostic-sample 5000
```

### diagnostic-arena-active-webgl

UTC 2026-10-29T22:28:46.293888+00:00; exit0; 50.3s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-arena-active-webgl/config.json --backend webgl --family arena-active --max-load 2048 --width 1280 --height 720 --warmup 2000 --sample 8000 --pairs 1 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/diagnostic-arena-active-webgl --candidate-root output/followup-5/probe2 --profile --frame-context 1 --diagnostics --diagnostic-sample 5000
```

### f6-worker-webgpu-initial-pair0

UTC 2026-10-29T22:29:36.669356+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgpu-initial-pair0 output/followup-2/original/pkg webgpu 64 1
```

### f6-worker-webgpu-final-pair0

UTC 2026-10-29T22:29:46.763246+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgpu-final-pair0 output/followup-5/probe2/pkg webgpu 64 1
```

### f6-worker-webgpu-final-pair1

UTC 2026-10-29T22:29:56.858191+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgpu-final-pair1 output/followup-5/probe2/pkg webgpu 64 1
```

### f6-worker-webgpu-initial-pair1

UTC 2026-10-29T22:30:06.959798+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgpu-initial-pair1 output/followup-2/original/pkg webgpu 64 1
```

### f6-worker-webgpu-initial-pair2

UTC 2026-10-29T22:30:17.057990+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgpu-initial-pair2 output/followup-2/original/pkg webgpu 64 1
```

### f6-worker-webgpu-final-pair2

UTC 2026-10-29T22:30:27.153858+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgpu-final-pair2 output/followup-5/probe2/pkg webgpu 64 1
```

### soak-initial-webgpu

UTC 2026-10-29T22:30:37.243166+00:00; exit0; 141.4s; power changed:False.

```sh
node scripts/render-performance.mjs soak --baseline-root output/followup-2/original --config output/followup-6/cal-rim-pool-cycle-webgpu/config.json --backend webgpu --family rim-pool-cycle --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 120000 --no-profile --startup-timeout 90000 --max-time 600 --out output/followup-6/soak-initial-webgpu
```

### soak-final-webgpu

UTC 2026-10-29T22:32:58.658514+00:00; exit0; 140.9s; power changed:False.

```sh
node scripts/render-performance.mjs soak --baseline-root output/followup-5/probe2 --config output/followup-6/cal-rim-pool-cycle-webgpu/config.json --backend webgpu --family rim-pool-cycle --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 120000 --no-profile --startup-timeout 90000 --max-time 600 --out output/followup-6/soak-final-webgpu
```

### f6-worker-webgl-initial-pair0

UTC 2026-10-29T22:35:19.593794+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgl-initial-pair0 output/followup-2/original/pkg webgl 64 1
```

### f6-worker-webgl-final-pair0

UTC 2026-10-29T22:35:29.688464+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgl-final-pair0 output/followup-5/probe2/pkg webgl 64 1
```

### f6-worker-webgl-final-pair1

UTC 2026-10-29T22:35:39.772019+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgl-final-pair1 output/followup-5/probe2/pkg webgl 64 1
```

### f6-worker-webgl-initial-pair1

UTC 2026-10-29T22:35:49.854264+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgl-initial-pair1 output/followup-2/original/pkg webgl 64 1
```

### f6-worker-webgl-initial-pair2

UTC 2026-10-29T22:35:59.947707+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgl-initial-pair2 output/followup-2/original/pkg webgl 64 1
```

### f6-worker-webgl-final-pair2

UTC 2026-10-29T22:36:10.037692+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/benchmark-rim-cuts.mjs f6-worker-webgl-final-pair2 output/followup-5/probe2/pkg webgl 64 1
```

### soak-initial-webgl

UTC 2026-10-29T22:36:20.129962+00:00; exit0; 141.0s; power changed:False.

```sh
node scripts/render-performance.mjs soak --baseline-root output/followup-2/original --config output/followup-6/cal-rim-pool-cycle-webgl/config.json --backend webgl --family rim-pool-cycle --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 120000 --no-profile --startup-timeout 90000 --max-time 600 --out output/followup-6/soak-initial-webgl
```

### soak-final-webgl

UTC 2026-10-29T22:38:41.150253+00:00; exit0; 141.1s; power changed:False.

```sh
node scripts/render-performance.mjs soak --baseline-root output/followup-5/probe2 --config output/followup-6/cal-rim-pool-cycle-webgl/config.json --backend webgl --family rim-pool-cycle --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 120000 --no-profile --startup-timeout 90000 --max-time 600 --out output/followup-6/soak-final-webgl
```

### cold

UTC 2026-10-29T22:41:02.297640+00:00; exit0; 70.4s; power changed:False.

```sh
node scripts/benchmark-cold-variants.mjs --baseline-root output/followup-2/original --candidate-root output/followup-5/probe2 --out output/followup-6/cold
```

### cal-historical-pool-webgpu

UTC 2026-10-29T22:42:12.772967+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgpu --family rim-pool-cycle --load 256 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-historical-pool-webgpu
```

### historical-pool-webgpu

UTC 2026-10-29T22:42:22.852708+00:00; exit0; 422.1s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-historical-pool-webgpu/config.json --backend webgpu --family rim-pool-cycle --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 60000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/historical-pool-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### cal-historical-pool-webgl

UTC 2026-10-29T22:49:24.981406+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgl --family rim-pool-cycle --load 256 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-historical-pool-webgl
```

### historical-pool-webgl

UTC 2026-10-29T22:49:35.065439+00:00; exit0; 422.6s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-historical-pool-webgl/config.json --backend webgl --family rim-pool-cycle --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 60000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/historical-pool-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### storage-cleanup

UTC 2026-10-29T22:56:37.818718+00:00; exit0; 10.1s; power changed:False.

```sh
python3 output/followup-6/cleanup_owned_restore.py
```

### cal-rim-common-webgpu

UTC 2026-10-29T22:56:47.902582+00:00; exit0; 20.2s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-5/probe2 --backend webgpu --family rim-common --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-common-webgpu
```

### three-common-webgpu

UTC 2026-10-29T22:57:08.138944+00:00; exit0; 221.1s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-5/probe2 --config output/followup-6/cal-rim-common-webgpu/config.json --backend webgpu --family rim-common --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/three-common-webgpu --three-root output/followup-5/probe2 --no-profile
```

### cal-rim-common-webgl

UTC 2026-10-29T23:00:49.299916+00:00; exit0; 10.0s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-5/probe2 --backend webgl --family rim-common --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-rim-common-webgl
```

### three-common-webgl

UTC 2026-10-29T23:00:59.358816+00:00; exit0; 222.3s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-5/probe2 --config output/followup-6/cal-rim-common-webgl/config.json --backend webgl --family rim-common --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/three-common-webgl --three-root output/followup-5/probe2 --no-profile
```

### cal-final-production-webgl

UTC 2026-10-29T23:04:41.714344+00:00; exit0; 20.2s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-5/probe2 --backend webgl --family rim-stress --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-final-production-webgl
```

### three-production-webgl

UTC 2026-10-29T23:05:01.973042+00:00; exit0; 222.1s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-5/probe2 --config output/followup-6/cal-final-production-webgl/config.json --backend webgl --family rim-stress --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/three-production-webgl --three-root output/followup-5/probe2 --no-profile
```

### cal-dynamic-webgpu

UTC 2026-10-29T23:08:44.196218+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgpu --family dynamic --load 64 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-dynamic-webgpu
```

### primary-dynamic-webgpu

UTC 2026-10-29T23:08:54.286516+00:00; exit0; 422.4s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-dynamic-webgpu/config.json --backend webgpu --family dynamic --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 60000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-dynamic-webgpu --candidate-root output/followup-5/probe2 --no-profile
```

### cal-dynamic-webgl

UTC 2026-10-29T23:15:56.723669+00:00; exit0; 10.1s; power changed:False.

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/followup-2/original --backend webgl --family dynamic --load 64 --max-load 2048 --width 1280 --height 720 --warmup 1000 --sample 3000 --no-profile --startup-timeout 90000 --out output/followup-6/cal-dynamic-webgl
```

### primary-dynamic-webgl

UTC 2026-10-29T23:16:06.808894+00:00; exit0; 422.3s; power changed:False.

```sh
node scripts/render-performance.mjs compare --baseline-root output/followup-2/original --config output/followup-6/cal-dynamic-webgl/config.json --backend webgl --family dynamic --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 60000 --pairs 3 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 --max-time 2400 --out output/followup-6/primary-dynamic-webgl --candidate-root output/followup-5/probe2 --no-profile
```

### final-gate

UTC 2026-10-29T23:23:09.167936+00:00; exit0; 694.2s; power changed:False.

```sh
env VERIFY_FORCE=1 VERIFY_WASM_PROFILE=release npm run verify:push
```

## Executed offline checks

```sh
python3 output/followup-6/analyze.py
/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 output/followup-6/validate_evidence.py
python3 output/followup-6/analyze_worker_soak.py
python3 output/followup-6/analyze_diagnostics.py
python3 output/followup-6/analyze_events.py
python3 output/followup-6/summarize_extra.py
python3 output/followup-6/check_final_identity.py
PYTHONPATH=output/followup6-validator-deps python3 /Users/Honza/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/wasm-render-performance
```

The first skill-validator invocation lacked PyYAML; installed only into ignored `output/followup6-validator-deps`, then reran the existing validator. No production dependency was changed. Analysis scripts, driver, exact input configurations and raw captures/time series are archived. `write_results.py` generated the initial result section; final report also receives manual evidence/acceptance edits.

## Archive and post-gate checks

See `VERIFICATION.json`, `FINAL_IDENTITY.json` and `ARCHIVE_VERIFICATION.json` for observed results and exact gate/restore identities. Archive creation and restore commands are below; neither publishes or deploys.

```sh
python3 output/followup-6/archive.py
python3 output/followup-6/restore-evidence.py docs/performance/render-followup/batch-6/EVIDENCE.tar.gz /private/tmp/rust-followup6-evidence-restore-final
python3 output/followup-6/verify_restore.py
git diff --check
```

Receipt-only command actually executed: `VERIFY_WASM_PROFILE=release npm exec -- node scripts/verification-cache.mjs check all`. Release identity: `VERIFY_WASM_PROFILE=release node --input-type=module -e 'import {assertFreshBuild} from "./scripts/build-fingerprint.mjs"; console.log(JSON.stringify(await assertFreshBuild()));'`. These run with the bundled Node PATH above.

Full gate is forced once using `VERIFY_FORCE=1 VERIFY_WASM_PROFILE=release npm run verify:push`; after it passes, a receipt-only check confirms unchanged inputs without rerunning all tests. Physical Quest is not included.
