#!/usr/bin/env bash
set -euo pipefail
cd /Users/Honza/.codex/worktrees/rim-publish/rust
node scripts/render-performance.mjs calibrate --baseline-root output/render-performance-current-vs-three/native --family rim-static --backend webgpu --width 4096 --height 2730 --max-load 1 --no-profile --warmup 1000 --sample 2500 --out output/render-performance-current-vs-three/cal-wg > /tmp/rim-current-cal-wg.log
for trial in 0 1 2; do
 node scripts/render-performance.mjs soak --baseline-root output/render-performance-current-vs-three/native --config output/render-performance-current-vs-three/cal-wg/config.json --max-load 1 --no-profile --warmup 8000 --sample 25000 --out "output/render-performance-current-vs-three/webgpu-$trial" > "/tmp/rim-current-webgpu-$trial.log"
done
node scripts/render-performance.mjs calibrate --baseline-root output/render-performance-current-vs-three/native --family rim-static --backend webgl --width 2880 --height 1920 --max-load 1 --no-profile --warmup 1000 --sample 2500 --out output/render-performance-current-vs-three/cal-normal-gl > /tmp/rim-current-cal-normal-gl.log
node scripts/render-performance.mjs compare --baseline-root output/render-performance-current-vs-three/native --three-root output/render-performance-rim-parity/three-original --config output/render-performance-current-vs-three/cal-normal-gl/config.json --max-load 1 --pairs 1 --no-profile --warmup 3000 --sample 8000 --out output/render-performance-current-vs-three/normal-gl-three > /tmp/rim-current-normal-gl-three.log
node scripts/render-performance.mjs smoke --baseline-root output/render-performance-current-vs-three/native --family rim-static --backend webgpu --width 2880 --height 1920 --max-load 1 --no-profile --warmup 3000 --sample 8000 --out output/render-performance-current-vs-three/normal-webgpu > /tmp/rim-current-normal-webgpu.log
