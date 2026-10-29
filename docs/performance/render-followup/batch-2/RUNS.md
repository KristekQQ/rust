# Followup2 — full clean-run results

All rows are retained, including adverse measurements. B/C labels mean baseline/candidate within the named dataset, not necessarily original/final. `paired-*` compares diagnostic reference to experimental query; `final-*` compares original f14 runtime to intermediate 882 runtime (optimized WG, original GL); `restored-*` compares original to delivered d067 runtime (original selector both). Clean profiling disabled. Frames are RAF intervals; callback time is wall time. No available clean GPU timing.

| Dataset | Run | FPS | Frame mean / p95 / p99 / max ms | Callback mean / p95 / p99 / max ms | Actual draw commands snapshot |
|---|---|---:|---|---|---:|
| final-arena-active-1-webgl | baseline-arena-active-1-pair0 | 60.003 | 16.666 / 16.700 / 16.800 / 16.800 | 1.897 / 2.600 / 3.000 / 7.500 | 48 |
| final-arena-active-1-webgl | candidate-arena-active-1-pair0 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 1.843 / 2.600 / 3.100 / 4.400 | 48 |
| final-arena-active-1-webgl | candidate-arena-active-1-pair1 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.790 / 2.700 / 3.400 / 6.800 | 48 |
| final-arena-active-1-webgl | baseline-arena-active-1-pair1 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.908 / 2.700 / 3.500 / 5.100 | 48 |
| final-arena-active-1-webgl | baseline-arena-active-1-pair2 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.866 / 2.600 / 2.800 / 4.400 | 48 |
| final-arena-active-1-webgl | candidate-arena-active-1-pair2 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.975 / 2.700 / 2.900 / 4.400 | 48 |
| final-arena-active-1-webgpu | baseline-arena-active-1-pair0 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.082 / 1.800 / 2.300 / 4.300 | 48 |
| final-arena-active-1-webgpu | candidate-arena-active-1-pair0 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.044 / 1.800 / 2.200 / 4.000 | 48 |
| final-arena-active-1-webgpu | candidate-arena-active-1-pair1 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.054 / 1.700 / 2.000 / 2.400 | 48 |
| final-arena-active-1-webgpu | baseline-arena-active-1-pair1 | 60.003 | 16.666 / 16.800 / 16.800 / 16.800 | 1.043 / 1.700 / 2.000 / 3.100 | 48 |
| final-arena-active-1-webgpu | baseline-arena-active-1-pair2 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 1.107 / 1.800 / 2.100 / 2.400 | 48 |
| final-arena-active-1-webgpu | candidate-arena-active-1-pair2 | 60.003 | 16.666 / 16.700 / 16.800 / 16.800 | 1.103 / 1.800 / 2.100 / 2.800 | 48 |
| final-rim-active-1-webgl | baseline-rim-active-1-pair0 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 2.827 / 3.800 / 4.200 / 4.900 | 203 |
| final-rim-active-1-webgl | candidate-rim-active-1-pair0 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 2.658 / 3.800 / 4.300 / 4.900 | 202 |
| final-rim-active-1-webgl | candidate-rim-active-1-pair1 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 2.774 / 3.900 / 4.300 / 5.400 | 203 |
| final-rim-active-1-webgl | baseline-rim-active-1-pair1 | 60.003 | 16.666 / 16.700 / 16.800 / 16.800 | 2.777 / 3.800 / 4.300 / 4.800 | 203 |
| final-rim-active-1-webgl | baseline-rim-active-1-pair2 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 2.828 / 3.800 / 4.300 / 4.800 | 202 |
| final-rim-active-1-webgl | candidate-rim-active-1-pair2 | 60.003 | 16.666 / 16.800 / 16.800 / 16.800 | 2.839 / 3.800 / 4.400 / 5.000 | 202 |
| final-rim-active-1-webgpu | baseline-rim-active-1-pair0 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 2.077 / 2.900 / 3.200 / 5.300 | 202 |
| final-rim-active-1-webgpu | candidate-rim-active-1-pair0 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 1.965 / 2.800 / 3.100 / 3.700 | 202 |
| final-rim-active-1-webgpu | candidate-rim-active-1-pair1 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.918 / 2.800 / 3.000 / 4.100 | 202 |
| final-rim-active-1-webgpu | baseline-rim-active-1-pair1 | 60.003 | 16.666 / 16.800 / 16.800 / 16.800 | 2.147 / 2.900 / 3.200 / 5.300 | 203 |
| final-rim-active-1-webgpu | baseline-rim-active-1-pair2 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 2.127 / 2.900 / 3.200 / 4.000 | 202 |
| final-rim-active-1-webgpu | candidate-rim-active-1-pair2 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 2.081 / 2.900 / 3.300 / 7.600 | 202 |
| final-rim-clustered-1024-webgl | baseline-rim-clustered-1024-pair0 | 25.058 | 39.908 / 50.100 / 50.100 / 66.700 | 39.719 / 49.000 / 53.500 / 63.500 | 241 |
| final-rim-clustered-1024-webgl | candidate-rim-clustered-1024-pair0 | 25.235 | 39.628 / 50.100 / 50.100 / 83.300 | 39.472 / 48.300 / 57.500 / 69.700 | 241 |
| final-rim-clustered-1024-webgl | candidate-rim-clustered-1024-pair1 | 25.081 | 39.871 / 50.100 / 50.100 / 66.700 | 39.681 / 48.700 / 51.300 / 65.100 | 241 |
| final-rim-clustered-1024-webgl | baseline-rim-clustered-1024-pair1 | 24.898 | 40.165 / 50.100 / 66.600 / 66.700 | 39.992 / 48.900 / 56.800 / 63.300 | 241 |
| final-rim-clustered-1024-webgl | baseline-rim-clustered-1024-pair2 | 24.961 | 40.062 / 50.100 / 50.100 / 66.700 | 39.869 / 48.800 / 50.900 / 64.300 | 241 |
| final-rim-clustered-1024-webgl | candidate-rim-clustered-1024-pair2 | 24.297 | 41.157 / 50.100 / 66.600 / 66.700 | 40.959 / 50.900 / 55.600 / 62.000 | 241 |
| final-rim-clustered-1024-webgpu | baseline-rim-clustered-1024-pair0 | 25.475 | 39.254 / 50.100 / 50.100 / 66.700 | 39.064 / 48.200 / 53.600 / 64.200 | 241 |
| final-rim-clustered-1024-webgpu | candidate-rim-clustered-1024-pair0 | 27.023 | 37.005 / 50.000 / 50.100 / 66.700 | 36.786 / 44.800 / 50.700 / 58.700 | 241 |
| final-rim-clustered-1024-webgpu | candidate-rim-clustered-1024-pair1 | 27.281 | 36.655 / 50.000 / 50.100 / 66.700 | 36.443 / 44.800 / 48.900 / 55.900 | 241 |
| final-rim-clustered-1024-webgpu | baseline-rim-clustered-1024-pair1 | 25.321 | 39.493 / 50.100 / 50.100 / 66.700 | 39.259 / 48.800 / 54.400 / 58.000 | 241 |
| final-rim-clustered-1024-webgpu | baseline-rim-clustered-1024-pair2 | 25.578 | 39.096 / 50.100 / 50.100 / 50.100 | 38.878 / 47.800 / 49.500 / 57.300 | 241 |
| final-rim-clustered-1024-webgpu | candidate-rim-clustered-1024-pair2 | 27.059 | 36.956 / 50.000 / 50.100 / 66.700 | 36.734 / 45.100 / 49.000 / 62.200 | 241 |
| final-rim-static-1-webgl | baseline-rim-static-1-pair0 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 1.234 / 2.000 / 2.300 / 3.200 | 40 |
| final-rim-static-1-webgl | candidate-rim-static-1-pair0 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 1.046 / 1.800 / 2.200 / 4.200 | 40 |
| final-rim-static-1-webgl | candidate-rim-static-1-pair1 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.242 / 2.000 / 2.500 / 2.800 | 40 |
| final-rim-static-1-webgl | baseline-rim-static-1-pair1 | 60.003 | 16.666 / 16.700 / 16.800 / 16.800 | 1.153 / 1.800 / 2.200 / 2.900 | 40 |
| final-rim-static-1-webgl | baseline-rim-static-1-pair2 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.275 / 1.900 / 2.400 / 2.900 | 40 |
| final-rim-static-1-webgl | candidate-rim-static-1-pair2 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 1.259 / 2.000 / 2.500 / 4.600 | 40 |
| final-rim-static-1-webgpu | baseline-rim-static-1-pair0 | 60.003 | 16.666 / 16.700 / 16.800 / 16.800 | 0.818 / 1.400 / 1.700 / 3.300 | 40 |
| final-rim-static-1-webgpu | candidate-rim-static-1-pair0 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 0.723 / 1.400 / 1.700 / 2.600 | 40 |
| final-rim-static-1-webgpu | candidate-rim-static-1-pair1 | 60.003 | 16.666 / 16.700 / 16.800 / 16.800 | 0.824 / 1.400 / 1.700 / 2.400 | 40 |
| final-rim-static-1-webgpu | baseline-rim-static-1-pair1 | 60.002 | 16.666 / 16.800 / 16.800 / 16.800 | 0.730 / 1.400 / 1.600 / 2.100 | 40 |
| final-rim-static-1-webgpu | baseline-rim-static-1-pair2 | 60.002 | 16.666 / 16.700 / 16.800 / 16.800 | 0.851 / 1.500 / 1.700 / 2.700 | 40 |
| final-rim-static-1-webgpu | candidate-rim-static-1-pair2 | 60.003 | 16.666 / 16.700 / 16.800 / 16.800 | 0.784 / 1.400 / 1.800 / 4.900 | 40 |
| final-rim-stress-1024-webgl | baseline-rim-stress-1024-pair0 | 42.533 | 23.511 / 33.400 / 33.400 / 33.500 | 8.284 / 15.800 / 17.700 / 19.500 | 241 |
| final-rim-stress-1024-webgl | candidate-rim-stress-1024-pair0 | 42.613 | 23.467 / 33.400 / 33.400 / 33.500 | 8.664 / 17.600 / 19.000 / 22.000 | 241 |
| final-rim-stress-1024-webgl | candidate-rim-stress-1024-pair1 | 42.442 | 23.562 / 33.400 / 33.400 / 33.500 | 8.396 / 15.900 / 18.500 / 26.500 | 241 |
| final-rim-stress-1024-webgl | baseline-rim-stress-1024-pair1 | 42.722 | 23.407 / 33.400 / 33.400 / 33.500 | 8.227 / 15.600 / 16.800 / 21.300 | 241 |
| final-rim-stress-1024-webgl | baseline-rim-stress-1024-pair2 | 42.242 | 23.673 / 33.400 / 33.400 / 33.400 | 8.434 / 16.500 / 18.700 / 20.800 | 241 |
| final-rim-stress-1024-webgl | candidate-rim-stress-1024-pair2 | 42.602 | 23.473 / 33.400 / 33.400 / 33.500 | 8.458 / 16.500 / 18.300 / 22.000 | 241 |
| final-rim-stress-2048-webgl | baseline-rim-stress-2048-pair0 | 28.921 | 34.577 / 50.000 / 50.100 / 50.100 | 17.472 / 30.700 / 32.100 / 37.900 | 308 |
| final-rim-stress-2048-webgl | candidate-rim-stress-2048-pair0 | 28.862 | 34.648 / 50.000 / 50.100 / 50.100 | 17.808 / 31.400 / 36.600 / 42.000 | 308 |
| final-rim-stress-2048-webgl | candidate-rim-stress-2048-pair1 | 28.841 | 34.673 / 50.000 / 50.100 / 50.100 | 17.679 / 31.000 / 33.400 / 41.300 | 308 |
| final-rim-stress-2048-webgl | baseline-rim-stress-2048-pair1 | 28.961 | 34.529 / 50.000 / 50.100 / 50.100 | 17.577 / 30.900 / 32.700 / 39.400 | 308 |
| final-rim-stress-2048-webgl | baseline-rim-stress-2048-pair2 | 28.961 | 34.529 / 50.000 / 50.100 / 50.100 | 17.692 / 31.300 / 33.200 / 37.900 | 308 |
| final-rim-stress-2048-webgl | candidate-rim-stress-2048-pair2 | 28.881 | 34.625 / 50.000 / 50.100 / 66.700 | 17.871 / 31.400 / 39.000 / 64.700 | 308 |
| final-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair0 | 37.497 | 26.669 / 33.400 / 33.400 / 50.000 | 15.073 / 31.000 / 34.200 / 37.800 | 308 |
| final-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair0 | 37.014 | 27.017 / 33.400 / 33.400 / 116.600 | 7.775 / 26.500 / 28.200 / 35.400 | 308 |
| final-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair1 | 37.292 | 26.815 / 33.400 / 33.400 / 116.700 | 7.899 / 27.000 / 28.900 / 74.800 | 308 |
| final-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair1 | 37.632 | 26.573 / 33.400 / 33.400 / 50.000 | 14.887 / 30.500 / 32.100 / 41.600 | 308 |
| final-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair2 | 37.736 | 26.500 / 33.400 / 33.400 / 66.700 | 14.797 / 30.400 / 31.200 / 41.100 | 308 |
| final-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair2 | 37.452 | 26.701 / 33.400 / 33.400 / 100.000 | 7.890 / 27.500 / 28.600 / 32.200 | 308 |
| paired-rim-stress-1024-webgl | baseline-rim-stress-1024-pair0 | 42.562 | 23.495 / 33.400 / 33.400 / 33.400 | 8.333 / 15.900 / 18.000 / 20.500 | 241 |
| paired-rim-stress-1024-webgl | candidate-rim-stress-1024-pair0 | 42.345 | 23.615 / 33.400 / 33.400 / 33.500 | 5.810 / 19.900 / 20.900 / 22.800 | 241 |
| paired-rim-stress-1024-webgl | candidate-rim-stress-1024-pair1 | 42.174 | 23.711 / 33.400 / 33.400 / 33.500 | 5.812 / 20.100 / 21.400 / 22.900 | 241 |
| paired-rim-stress-1024-webgl | baseline-rim-stress-1024-pair1 | 42.550 | 23.502 / 33.400 / 33.400 / 33.500 | 8.595 / 16.800 / 19.000 / 20.500 | 241 |
| paired-rim-stress-1024-webgl | baseline-rim-stress-1024-pair2 | 42.682 | 23.429 / 33.400 / 33.400 / 33.500 | 8.266 / 15.600 / 17.800 / 21.100 | 241 |
| paired-rim-stress-1024-webgl | candidate-rim-stress-1024-pair2 | 42.373 | 23.600 / 33.400 / 33.400 / 33.500 | 6.097 / 20.300 / 21.200 / 24.900 | 241 |
| paired-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair0 | 37.471 | 26.687 / 33.400 / 33.400 / 50.000 | 15.042 / 30.900 / 34.200 / 44.000 | 308 |
| paired-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair0 | 37.397 | 26.740 / 33.400 / 33.400 / 83.300 | 7.786 / 27.200 / 28.100 / 33.500 | 308 |
| paired-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair1 | 37.516 | 26.655 / 33.400 / 33.400 / 100.000 | 7.938 / 27.600 / 28.700 / 31.700 | 308 |
| paired-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair1 | 37.376 | 26.755 / 33.400 / 33.400 / 50.000 | 15.120 / 31.200 / 34.600 / 38.400 | 308 |
| paired-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair2 | 37.616 | 26.584 / 33.400 / 33.400 / 49.900 | 14.867 / 30.400 / 31.900 / 36.900 | 308 |
| paired-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair2 | 37.591 | 26.602 / 33.400 / 33.400 / 50.000 | 7.894 / 27.200 / 28.300 / 31.200 | 308 |
| restored-rim-stress-1024-webgl | baseline-rim-stress-1024-pair0 | 42.454 | 23.555 / 33.400 / 33.400 / 33.500 | 8.412 / 16.300 / 17.900 / 23.300 | 241 |
| restored-rim-stress-1024-webgl | candidate-rim-stress-1024-pair0 | 42.374 | 23.600 / 33.400 / 33.400 / 33.500 | 8.372 / 16.200 / 18.000 / 22.000 | 241 |
| restored-rim-stress-1024-webgl | candidate-rim-stress-1024-pair1 | 42.493 | 23.533 / 33.400 / 33.400 / 33.500 | 8.308 / 15.700 / 17.600 / 26.500 | 241 |
| restored-rim-stress-1024-webgl | baseline-rim-stress-1024-pair1 | 42.414 | 23.577 / 33.400 / 33.400 / 33.500 | 8.325 / 15.700 / 17.500 / 21.400 | 241 |
| restored-rim-stress-1024-webgl | baseline-rim-stress-1024-pair2 | 42.442 | 23.562 / 33.400 / 33.400 / 33.500 | 8.357 / 15.900 / 18.100 / 23.400 | 241 |
| restored-rim-stress-1024-webgl | candidate-rim-stress-1024-pair2 | 42.465 | 23.549 / 33.400 / 33.400 / 33.500 | 8.273 / 15.800 / 17.300 / 21.200 | 241 |
| restored-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair0 | 37.551 | 26.630 / 33.400 / 33.400 / 50.000 | 14.987 / 30.800 / 33.000 / 41.400 | 308 |
| restored-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair0 | 37.497 | 26.669 / 33.400 / 33.400 / 66.600 | 15.105 / 31.300 / 33.400 / 39.000 | 308 |
| restored-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair1 | 37.551 | 26.630 / 33.400 / 33.400 / 66.700 | 15.017 / 30.900 / 33.800 / 46.000 | 308 |
| restored-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair1 | 37.642 | 26.566 / 33.400 / 33.400 / 33.500 | 14.997 / 30.800 / 33.900 / 41.500 | 308 |
| restored-rim-stress-2048-webgpu | baseline-rim-stress-2048-pair2 | 37.337 | 26.783 / 33.400 / 33.400 / 83.400 | 15.253 / 31.600 / 35.500 / 40.500 | 308 |
| restored-rim-stress-2048-webgpu | candidate-rim-stress-2048-pair2 | 37.671 | 26.545 / 33.400 / 33.400 / 66.600 | 14.877 / 30.600 / 31.700 / 37.000 | 308 |

## Paired observations

Percentages describe these runs; nonisolated host does not establish a small causal gain/regression. `unavailable` is not a pixel PASS.

| Dataset | Pair | FPS baseline → candidate | Observed delta | Pixel verdict |
|---|---:|---|---:|---|
| final-arena-active-1-webgl | 0 | 60.003 → 60.002 | -0.00% | unavailable |
| final-arena-active-1-webgl | 1 | 60.002 → 60.002 | +0.00% | unavailable |
| final-arena-active-1-webgl | 2 | 60.002 → 60.002 | +0.00% | unavailable |
| final-arena-active-1-webgpu | 0 | 60.002 → 60.002 | +0.00% | unavailable |
| final-arena-active-1-webgpu | 1 | 60.003 → 60.002 | -0.00% | unavailable |
| final-arena-active-1-webgpu | 2 | 60.002 → 60.003 | +0.00% | unavailable |
| final-rim-active-1-webgl | 0 | 60.002 → 60.002 | +0.00% | unavailable |
| final-rim-active-1-webgl | 1 | 60.003 → 60.002 | -0.00% | unavailable |
| final-rim-active-1-webgl | 2 | 60.002 → 60.003 | +0.00% | unavailable |
| final-rim-active-1-webgpu | 0 | 60.002 → 60.002 | +0.00% | unavailable |
| final-rim-active-1-webgpu | 1 | 60.003 → 60.002 | -0.00% | unavailable |
| final-rim-active-1-webgpu | 2 | 60.002 → 60.002 | -0.00% | unavailable |
| final-rim-clustered-1024-webgl | 0 | 25.058 → 25.235 | +0.71% | exact |
| final-rim-clustered-1024-webgl | 1 | 24.898 → 25.081 | +0.74% | exact |
| final-rim-clustered-1024-webgl | 2 | 24.961 → 24.297 | -2.66% | exact |
| final-rim-clustered-1024-webgpu | 0 | 25.475 → 27.023 | +6.08% | FAIL |
| final-rim-clustered-1024-webgpu | 1 | 25.321 → 27.281 | +7.74% | exact |
| final-rim-clustered-1024-webgpu | 2 | 25.578 → 27.059 | +5.79% | exact |
| final-rim-static-1-webgl | 0 | 60.002 → 60.002 | +0.00% | exact |
| final-rim-static-1-webgl | 1 | 60.003 → 60.002 | -0.00% | exact |
| final-rim-static-1-webgl | 2 | 60.002 → 60.002 | +0.00% | exact |
| final-rim-static-1-webgpu | 0 | 60.003 → 60.002 | -0.00% | exact |
| final-rim-static-1-webgpu | 1 | 60.002 → 60.003 | +0.00% | exact |
| final-rim-static-1-webgpu | 2 | 60.002 → 60.003 | +0.00% | exact |
| final-rim-stress-1024-webgl | 0 | 42.533 → 42.613 | +0.19% | exact |
| final-rim-stress-1024-webgl | 1 | 42.722 → 42.442 | -0.65% | exact |
| final-rim-stress-1024-webgl | 2 | 42.242 → 42.602 | +0.85% | exact |
| final-rim-stress-2048-webgl | 0 | 28.921 → 28.862 | -0.20% | exact |
| final-rim-stress-2048-webgl | 1 | 28.961 → 28.841 | -0.41% | exact |
| final-rim-stress-2048-webgl | 2 | 28.961 → 28.881 | -0.28% | exact |
| final-rim-stress-2048-webgpu | 0 | 37.497 → 37.014 | -1.29% | exact |
| final-rim-stress-2048-webgpu | 1 | 37.632 → 37.292 | -0.90% | exact |
| final-rim-stress-2048-webgpu | 2 | 37.736 → 37.452 | -0.75% | exact |
| paired-rim-stress-1024-webgl | 0 | 42.562 → 42.345 | -0.51% | exact |
| paired-rim-stress-1024-webgl | 1 | 42.550 → 42.174 | -0.88% | exact |
| paired-rim-stress-1024-webgl | 2 | 42.682 → 42.373 | -0.72% | exact |
| paired-rim-stress-2048-webgpu | 0 | 37.471 → 37.397 | -0.20% | exact |
| paired-rim-stress-2048-webgpu | 1 | 37.376 → 37.516 | +0.37% | exact |
| paired-rim-stress-2048-webgpu | 2 | 37.616 → 37.591 | -0.07% | exact |
| restored-rim-stress-1024-webgl | 0 | 42.454 → 42.374 | -0.19% | exact |
| restored-rim-stress-1024-webgl | 1 | 42.414 → 42.493 | +0.19% | exact |
| restored-rim-stress-1024-webgl | 2 | 42.442 → 42.465 | +0.05% | exact |
| restored-rim-stress-2048-webgpu | 0 | 37.551 → 37.497 | -0.15% | exact |
| restored-rim-stress-2048-webgpu | 1 | 37.642 → 37.551 | -0.24% | exact |
| restored-rim-stress-2048-webgpu | 2 | 37.337 → 37.671 | +0.90% | exact |

Instance/triangle/upload/allocation deltas and actual quality states are in RESULTS.json; complete raw frame sequences, boundary states, commands, captures and configurations are in EVIDENCE.tar.gz. `rendererStatsDrawCallsNotCommands` deliberately preserves logical primitive statistics under a name that prevents confusing them with actual commands. Dynamic native snapshot counts can differ because sampled simulation phases differ. No per-frame native command histogram is claimed.
