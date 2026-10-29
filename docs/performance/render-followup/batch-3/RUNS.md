# Followup3 runs

Clean measurements:8s warmup/25s sample,3 alternating pairs per workload/API. Same1280x720; no profiling. GPU time unavailable in clean runs.

| Workload/API | Pair | Build | FPS | Frame p95/p99/max ms | Callback p95/p99/max ms | Panel creates / updates |
|---|---:|---|---:|---|---|---|
| V1 arena-active/webgl | 0 | baseline | 60.002 | 16.700/16.800/16.800 | 2.700/3.000/3.300 | unavailable in original |
| V1 arena-active/webgl | 0 | candidate | 60.002 | 16.700/16.800/16.800 | 2.600/2.900/4.000 | 0 / 93 |
| V1 arena-active/webgl | 1 | candidate | 60.002 | 16.700/16.800/16.800 | 2.700/2.900/3.400 | 0 / 92 |
| V1 arena-active/webgl | 1 | baseline | 60.002 | 16.800/16.800/16.800 | 2.600/2.900/3.400 | unavailable in original |
| V1 arena-active/webgl | 2 | baseline | 60.003 | 16.800/16.800/16.800 | 2.700/2.900/3.100 | unavailable in original |
| V1 arena-active/webgl | 2 | candidate | 60.003 | 16.700/16.800/16.800 | 2.700/3.000/3.900 | 0 / 92 |
| V1 arena-active/webgpu | 0 | baseline | 60.003 | 16.700/16.800/16.800 | 1.600/1.800/4.900 | unavailable in original |
| V1 arena-active/webgpu | 0 | candidate | 60.003 | 16.800/16.800/16.800 | 1.800/2.100/2.900 | 0 / 93 |
| V1 arena-active/webgpu | 1 | candidate | 60.002 | 16.700/16.800/16.800 | 1.900/2.400/2.800 | 0 / 92 |
| V1 arena-active/webgpu | 1 | baseline | 60.002 | 16.800/16.800/16.800 | 1.900/2.200/4.400 | unavailable in original |
| V1 arena-active/webgpu | 2 | baseline | 60.002 | 16.800/16.800/16.800 | 1.800/2.100/2.500 | unavailable in original |
| V1 arena-active/webgpu | 2 | candidate | 60.002 | 16.800/16.800/16.800 | 1.900/2.300/3.000 | 0 / 92 |
| V1 panel-events/webgl | 0 | baseline | 60.002 | 16.800/16.800/16.800 | 2.100/2.800/4.100 | 3232 / 1616 |
| V1 panel-events/webgl | 0 | candidate | 60.002 | 16.700/16.800/16.800 | 2.100/2.600/4.000 | 768 / 1600 |
| V1 panel-events/webgl | 1 | candidate | 60.003 | 16.700/16.800/16.800 | 2.100/2.500/3.200 | 768 / 1616 |
| V1 panel-events/webgl | 1 | baseline | 60.003 | 16.700/16.800/16.800 | 2.300/2.900/4.200 | 3232 / 1616 |
| V1 panel-events/webgl | 2 | baseline | 60.002 | 16.800/16.800/16.800 | 2.300/2.700/3.300 | 3200 / 1600 |
| V1 panel-events/webgl | 2 | candidate | 60.003 | 16.700/16.800/16.800 | 2.100/2.700/3.400 | 768 / 1600 |
| V1 panel-events/webgpu | 0 | baseline | 60.002 | 16.700/16.800/16.800 | 1.900/2.500/3.600 | 3200 / 1600 |
| V1 panel-events/webgpu | 0 | candidate | 60.002 | 16.800/16.800/16.800 | 2.100/2.800/3.400 | 768 / 1600 |
| V1 panel-events/webgpu | 1 | candidate | 60.002 | 16.700/16.800/16.800 | 1.800/3.200/4.200 | 768 / 1600 |
| V1 panel-events/webgpu | 1 | baseline | 60.002 | 16.700/16.800/16.800 | 1.900/2.600/3.700 | 3200 / 1600 |
| V1 panel-events/webgpu | 2 | baseline | 60.002 | 16.700/16.800/16.800 | 1.800/2.500/3.100 | 3200 / 1600 |
| V1 panel-events/webgpu | 2 | candidate | 60.002 | 16.700/16.800/16.800 | 2.400/3.600/5.100 | 768 / 1600 |
| final arena-active/webgl | 0 | baseline | 30.001 | 33.400/33.400/33.500 | 3.700/4.000/4.400 | unavailable in original |
| final arena-active/webgl | 0 | candidate | 30.001 | 33.400/33.400/33.500 | 3.700/4.000/9.200 | 0 / 91 |
| final arena-active/webgl | 1 | candidate | 30.001 | 33.400/33.400/33.400 | 3.700/4.100/4.400 | 0 / 92 |
| final arena-active/webgl | 1 | baseline | 30.001 | 33.400/33.400/33.500 | 3.600/4.100/4.600 | unavailable in original |
| final arena-active/webgl | 2 | baseline | 30.001 | 33.400/33.400/33.500 | 4.000/4.300/4.800 | unavailable in original |
| final arena-active/webgl | 2 | candidate | 30.001 | 33.400/33.400/33.500 | 3.900/4.200/4.500 | 0 / 93 |
| final arena-active/webgpu | 0 | baseline | 30.001 | 33.400/33.400/33.500 | 2.300/2.900/9.000 | unavailable in original |
| final arena-active/webgpu | 0 | candidate | 30.001 | 33.400/33.400/33.500 | 2.400/2.800/5.200 | 186 / 93 |
| final arena-active/webgpu | 1 | candidate | 30.001 | 33.400/33.400/33.500 | 2.400/3.000/3.900 | 184 / 92 |
| final arena-active/webgpu | 1 | baseline | 30.001 | 33.400/33.400/33.500 | 2.400/2.700/3.300 | unavailable in original |
| final arena-active/webgpu | 2 | baseline | 30.001 | 33.400/33.400/33.500 | 2.300/2.700/4.500 | unavailable in original |
| final arena-active/webgpu | 2 | candidate | 30.001 | 33.400/33.400/33.500 | 2.400/2.900/4.700 | 184 / 92 |
| final panel-events/webgl | 0 | baseline | 30.001 | 33.400/33.400/33.500 | 2.900/3.600/4.100 | 3200 / 1600 |
| final panel-events/webgl | 0 | candidate | 30.001 | 33.400/33.500/33.500 | 2.700/3.400/3.600 | 768 / 1616 |
| final panel-events/webgl | 1 | candidate | 30.001 | 33.400/33.400/33.500 | 2.700/3.200/4.200 | 768 / 1616 |
| final panel-events/webgl | 1 | baseline | 30.001 | 33.400/33.400/33.500 | 3.100/3.600/4.300 | 3200 / 1600 |
| final panel-events/webgl | 2 | baseline | 30.001 | 33.400/33.400/33.500 | 2.900/3.500/3.900 | 3200 / 1600 |
| final panel-events/webgl | 2 | candidate | 30.001 | 33.400/33.400/33.500 | 3.000/3.600/4.300 | 768 / 1616 |
| final panel-events/webgpu | 0 | baseline | 30.001 | 33.400/33.400/33.500 | 2.700/3.400/3.700 | 3232 / 1616 |
| final panel-events/webgpu | 0 | candidate | 30.001 | 33.400/33.400/33.500 | 3.500/4.100/4.800 | 768 / 1600 |
| final panel-events/webgpu | 1 | candidate | 30.001 | 33.400/33.400/33.500 | 3.500/4.600/5.500 | 768 / 1600 |
| final panel-events/webgpu | 1 | baseline | 30.001 | 33.400/33.400/33.500 | 2.900/3.500/5.200 | 3200 / 1600 |
| final panel-events/webgpu | 2 | baseline | 30.001 | 33.400/33.400/33.500 | 2.800/3.300/3.900 | 3200 / 1600 |
| final panel-events/webgpu | 2 | candidate | 30.001 | 33.400/33.400/33.500 | 3.300/3.800/4.700 | 768 / 1600 |
