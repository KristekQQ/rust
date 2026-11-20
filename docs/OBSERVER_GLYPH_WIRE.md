# Observer glyph transport

The v4 publisher omits unused fallback panel triangles/glow when `font3d_bound` is true. The existing shared font variant assets and authoritative glyph child instances remain unchanged; appearance and font selection are still represented by those assets/instances. Bound panel metadata retains dimensions, bounds and binding state. Its fallback text revision is normalized, so a changed score does not invalidate an unused mesh. Unbound panels retain their geometry path. This is wire-compatible with the existing receiver and observer WASM.

## Cut reconstruction: proposed next step, not implemented

Keep the headset authoritative for gameplay, movement and score. Known models should use immutable asset IDs/content hashes and instance transforms. A cut recipe must identify the parent asset/version, event/child IDs, finite blade sweep or ribbon in parent-local coordinates, cap/material parameters and pivot rebasing. A plane alone is insufficient for the current finite-stroke cutter. Reuse the Rust CPU cutter, not a separate JS approximation.

Validate native/WASM reconstructed geometry on exact inputs before enabling recipes. The current wire has parent/child lineage and birth transforms for interpolation, but does not retain the full cut recipe or guarantee an identical canonical CPU source on the receiver. Add bounded event retention/ACKs so coalesced frames, late joins and repeated cuts cannot lose parent dependencies. Verify a geometry fingerprint; request a one-time mesh snapshot on missing assets or mismatches. Do not resend the full mesh for ordinary motion, or silently show an invented split.

## Evidence

Baseline physical Quest scripted run: 55 seconds, 57 cuts. Decoded observer capture (including setup) attributed 80.4% of bytes to panel assets. This is decoded payload attribution, not a compressed-link throughput comparison. Candidate evidence lives in ignored `output/observer-glyph/`.

Candidate0.13.1 installed and hash-verified on Quest3. All13 wire tests passed. Physical55s scripted run completed64cuts (baseline57); normal input restored and original scores unchanged. During remaining50..10seconds, observer delivery improved1.51->15.39updates/s; decoded panel assets125.59MB->0.536MB. Cuts differ, so this is workload-matched operational evidence, not bit-identical A/B or claimed FPS gain. Browser visibly retained timer, score, fruit and cut fragments. No main integration/push.

## Lossless cut-burst compression (0.13.2)

After glyph changes, real cuts still produced bursts up to1MB. The native worker now optionally wraps complete NDP1 packets with NZP1 (u32 decoded length + level1 standard zlib), only for>=4096B packets saving>=32B including envelope. Delta ACK bases remain uncompressed, and all geometry remains exact. Updated receiver validates size/checksum/trailing bytes before delta application; deploy receiver before APK. LAN gateway negotiates no-context permessage-deflate; browser byte labels describe post-decompression data.

Physical55s baseline and two55s candidate runs: native ingress898->203/208kBps; receipt-agep95144->92/99ms. Actual cuts61/60/63, so these are matched scripted workloads, not identical events. Maximum receipt age374->703/265ms: occasional spikes remain. No measured gameplay FPSgain (about83.6FPS in allruns). Normal input and scores restored; Stream preserved. Sources span native checkout and existing quest-short-links checkout, recorded in local coordination.
