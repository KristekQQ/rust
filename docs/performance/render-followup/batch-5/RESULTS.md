# Actual run results

All individual runs are retained in RUNS.json/CSV and the raw archive. FPS is1000/mean RAF interval; CPU is callback wall time, including browser/API stalls and sampler. Three production/common comparisons are not exact graphic equivalence. Clean GPU time is unavailable; diagnostic queries are separate. No max-only performance verdict.

| Campaign | Pair | Baseline→candidate FPS | Callback mean ms | Frame p95/p99 ms | Callback p99 ms |
|---|---:|---:|---:|---:|---:|
|primary-arena-active-webgl|0|60.00→60.00|1.86→1.86|16.70/16.80→16.70/16.80|2.90→3.00|
|primary-arena-active-webgl|1|60.00→60.00|2.04→2.02|16.70/16.80→16.70/16.80|3.00→3.00|
|primary-arena-active-webgl|2|60.00→60.00|1.94→2.04|16.70/16.80→16.70/16.80|2.90→2.90|
|primary-arena-active-webgpu|0|60.00→60.00|1.00→0.94|16.70/16.80→16.70/16.80|1.80→1.80|
|primary-arena-active-webgpu|1|60.00→60.00|0.94→1.00|16.80/16.80→16.70/16.80|1.80→1.90|
|primary-arena-active-webgpu|2|60.00→60.00|1.01→0.98|16.70/16.80→16.70/16.80|1.90→1.80|
|primary-rim-active-webgl|0|60.00→60.00|2.80→2.54|16.70/16.80→16.70/16.80|4.30→4.20|
|primary-rim-active-webgl|1|60.00→60.00|2.86→2.94|16.80/16.80→16.70/16.80|4.30→4.30|
|primary-rim-active-webgl|2|60.00→60.00|2.93→2.95|16.70/16.80→16.80/16.80|4.30→4.40|
|primary-rim-active-webgpu|0|60.00→60.00|2.23→2.21|16.70/16.80→16.80/16.80|3.50→3.40|
|primary-rim-active-webgpu|1|60.00→60.00|2.36→2.25|16.70/16.80→16.70/16.80|3.50→3.40|
|primary-rim-active-webgpu|2|60.00→60.00|2.19→2.26|16.70/16.80→16.80/16.80|3.50→3.50|
|primary-rim-camera-stress-webgl|0|36.80→36.88|22.33→21.35|33.40/33.40→33.40/33.40|36.80→36.00|
|primary-rim-camera-stress-webgl|1|34.70→36.90|25.82→21.48|50.00/50.10→33.40/33.50|50.10→35.70|
|primary-rim-camera-stress-webgl|2|34.16→35.48|26.98→21.73|50.00/50.10→33.40/50.00|51.40→39.20|
|primary-rim-camera-stress-webgpu|0|49.69→51.22|19.86→19.11|33.40/33.40→33.40/33.40|34.70→34.80|
|primary-rim-camera-stress-webgpu|1|50.77→52.64|19.41→18.71|33.40/33.40→33.30/33.40|33.70→33.20|
|primary-rim-camera-stress-webgpu|2|49.81→53.12|19.79→18.50|33.40/33.40→33.40/33.40|36.00→32.90|
|primary-rim-clustered-webgl|0|25.14→25.14|39.61→39.61|50.10/50.10→50.10/50.10|49.40→50.30|
|primary-rim-clustered-webgl|1|25.34→25.46|39.28→39.12|50.10/50.10→50.10/50.10|50.70→48.80|
|primary-rim-clustered-webgl|2|25.60→25.82|38.90→38.53|50.00/50.10→50.10/50.10|47.60→48.30|
|primary-rim-clustered-webgpu|0|26.30→26.38|37.83→37.68|50.00/50.10→50.00/50.10|47.30→47.20|
|primary-rim-clustered-webgpu|1|26.36→26.46|37.74→37.56|50.10/50.10→50.00/50.10|46.70→46.90|
|primary-rim-clustered-webgpu|2|26.22→26.42|37.92→37.64|50.00/50.10→50.00/50.10|47.40→46.70|
|primary-rim-mixed-webgl|0|30.00→29.98|28.26→27.48|50.00/50.10→50.00/50.10|44.50→46.20|
|primary-rim-mixed-webgl|1|30.00→30.00|28.43→27.09|50.00/50.10→50.00/50.10|45.70→42.70|
|primary-rim-mixed-webgl|2|29.96→30.00|28.63→27.18|50.00/50.10→50.00/50.10|45.70→43.00|
|primary-rim-mixed-webgpu|0|34.99→35.10|28.33→28.16|50.00/50.10→50.00/66.60|55.40→56.80|
|primary-rim-mixed-webgpu|1|36.98→38.55|26.85→25.73|50.00/50.10→50.00/50.10|43.50→41.20|
|primary-rim-mixed-webgpu|2|36.88→38.58|26.92→25.73|50.00/50.10→50.00/50.10|44.70→42.00|
|primary-rim-stress-webgl|0|31.00→31.00|15.61→15.62|33.40/33.50→33.40/50.00|31.00→30.90|
|primary-rim-stress-webgl|1|31.00→30.96|15.52→15.68|33.40/50.00→33.40/50.00|30.20→31.10|
|primary-rim-stress-webgl|2|31.04→31.02|15.59→15.53|33.40/50.00→33.40/50.00|30.40→30.30|
|primary-rim-stress-webgpu|0|39.47→39.30|14.93→15.24|33.40/33.40→33.40/33.40|31.20→33.10|
|primary-rim-stress-webgpu|1|39.46→39.42|14.91→15.00|33.40/33.40→33.40/33.40|31.20→32.00|
|primary-rim-stress-webgpu|2|39.27→39.42|15.16→15.01|33.40/33.40→33.40/33.40|32.70→31.50|
|three-common-webgl|0|40.31→38.56|7.40→0.73|33.40/33.40→33.40/33.40|28.40→1.00|
|three-common-webgl|1|40.39→38.50|7.46→0.73|33.40/33.40→33.40/33.40|28.60→1.00|
|three-common-webgl|2|39.74→37.28|7.70→0.83|33.40/33.40→33.40/33.40|28.90→1.10|
|three-common-webgpu|0|44.67→38.55|6.72→0.70|33.40/33.40→33.40/33.40|29.10→1.00|
|three-common-webgpu|1|44.74→38.56|6.70→0.78|33.40/33.40→33.40/33.40|29.00→1.00|
|three-common-webgpu|2|44.62→38.55|6.76→0.75|33.40/33.40→33.40/33.40|29.00→1.00|
|three-production-webgl|0|30.00→29.96|16.66→3.17|33.40/33.40→33.40/50.00|32.60→4.80|
|three-production-webgl|1|30.00→30.02|16.77→3.76|33.40/33.40→33.40/50.00|32.00→4.70|
|three-production-webgl|2|30.00→29.92|17.13→3.73|33.40/33.40→50.00/50.10|33.60→4.80|
