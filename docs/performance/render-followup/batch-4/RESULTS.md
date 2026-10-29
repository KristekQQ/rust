# Three-pair results

All entries are medians of three per-run values, **not pooled percentiles**. Each underlying run and full distribution is retained in `measurements/RUNS.md` and `ANALYSIS.json` in the evidence archive. Original union versus coalesced policy in the same probe runtime. 8s warmup/25s sample; pool60s. Profile off, 1280×720, DPR1. No cross-environment comparison of absolute FPS.

| Workload / backend / campaign | FPS union → coalesced | Callback mean ms | Callback p95 ms | Callback p99 ms | RAF p99 ms | Instance bytes/RAF | Writes/RAF |
|---|---:|---:|---:|---:|---:|---:|---:|
| quiet-rim-camera-stress-webgpu | 45.28 → 46.37 | 21.77 → 21.30 | 35.10 → 35.10 | 36.60 → 37.20 | 50.00 → 50.00 | 1403165.70 → 84714.63 | 0.99 → 25.86 |
| quiet-rim-mixed-webgpu | 36.18 → 36.82 | 27.44 → 26.97 | 42.80 → 42.40 | 45.60 → 43.20 | 50.10 → 50.10 | 1081616.00 → 1072.00 | 1.00 → 67.00 |
| quiet-rim-moving-webgpu | 32.40 → 32.34 | 30.68 → 30.75 | 37.70 → 37.80 | 38.80 → 39.60 | 50.10 → 50.10 | 1066408.19 → 1066396.66 | 1.00 → 1.00 |
| quiet-rim-clustered-webgpu | 25.70 → 25.89 | 38.72 → 38.36 | 48.40 → 47.00 | 49.50 → 48.70 | 50.10 → 50.10 | 833453.36 → 834329.96 | 1.00 → 1.00 |
| quiet-rim-pool-cycle-webgpu | 60.00 → 60.00 | 2.37 → 2.35 | 6.60 → 6.50 | 12.60 → 12.60 | 16.80 → 16.80 | 2427.06 → 2426.39 | 0.02 → 0.02 |
| primary-rim-stress-webgpu | 20.29 → 19.77 | 17.12 → 17.50 | 30.10 → 30.20 | 31.90 → 31.00 | 66.70 → 66.70 | 0.00 → 0.00 | 0.00 → 0.00 |
| primary-rim-static-webgpu | 60.00 → 60.00 | 0.85 → 0.84 | 1.60 → 1.50 | 1.90 → 1.90 | 16.80 → 16.80 | 0.00 → 0.00 | 0.00 → 0.00 |
| primary-rim-active-webgpu | 60.00 → 60.00 | 2.16 → 2.16 | 3.00 → 2.90 | 3.30 → 3.20 | 16.80 → 16.80 | 69022.70 → 34776.41 | 1.00 → 69.18 |
| primary-rim-camera-stress-webgl | 14.24 → 11.75 | 31.81 → 33.07 | 68.80 → 45.30 | 132.80 → 63.50 | 133.40 → 149.90 | 1577653.78 → 221724.19 | 1.00 → 69.22 |
| primary-rim-mixed-webgl | 13.55 → 10.64 | 36.41 → 45.08 | 53.40 → 69.20 | 90.30 → 179.90 | 116.60 → 183.30 | 1081616.00 → 1072.00 | 1.00 → 67.00 |
| primary-rim-moving-webgl | 15.07 → 16.47 | 66.00 → 60.41 | 87.50 → 73.80 | 107.40 → 88.80 | 116.70 → 99.90 | 1066385.77 → 1066426.03 | 1.00 → 1.00 |
| primary-rim-clustered-webgl | 11.21 → 14.59 | 88.74 → 68.25 | 138.90 → 82.10 | 143.40 → 106.00 | 150.10 → 100.10 | 838930.11 → 838662.99 | 1.00 → 1.00 |
| primary-home-rim-pool-cycle-webgl | 60.00 → 60.00 | 2.94 → 2.88 | 6.40 → 6.70 | 13.00 → 12.60 | 16.80 → 16.80 | 2426.39 → 2426.39 | 0.02 → 0.02 |
| primary-home-rim-stress-webgl | 31.06 → 30.94 | 15.63 → 16.45 | 30.00 → 32.00 | 31.10 → 32.50 | 33.40 → 33.50 | 0.00 → 0.00 | 0.00 → 0.00 |
| primary-home-rim-static-webgl | 60.00 → 60.00 | 1.49 → 1.45 | 2.30 → 2.30 | 2.60 → 2.60 | 16.80 → 16.80 | 0.00 → 0.00 | 0.00 → 0.00 |
| primary-home-rim-active-webgl | 60.00 → 60.00 | 3.03 → 2.95 | 4.10 → 4.10 | 4.70 → 4.70 | 16.80 → 16.80 | 69025.78 → 34789.39 | 1.00 → 69.22 |

The pre-closure WebGPU repeats and the interrupted WebGL pool experiment are retained, not silently replaced. The selected quiet/home repetitions were declared after the observed environment change, before their results. Earlier WebGL primary runs used AC after storage processes closed but before transport; home data must not be combined with them as one controlled temperature state.

Sparse2048 has exactly307 glTF draw commands,126823 instances and39067116 triangles/RAF on both policies/backends. Camera/moving workload average triangle counts differ slightly because equal wall-time scenarios contain different RAF sample times; fixed capture states remain exact. No geometry, quality, resolution or update-rate reduction was made. Actual whole-API diagnostic counters and completeness flags are archived separately from these engine glTF counters.
