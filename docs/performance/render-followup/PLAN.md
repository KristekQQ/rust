# Render followup — six separately authorized batches

1. Image correctness and verified starting state (diagnosis recorded; A unresolved, not repaired).
2. Overlay selection (completed2026-10-29: diagnosis/diagnostics accepted; automatic query change rejected).
3. Non-glTF allocations in Neon.
4. Camera motion, uploads and worst frames.
5. Independent remaining-bottleneck audit and Three comparison.
6. Final verification and memory/skill update.

Batches1–2 are recorded; batch2 is complete. Batches3–6 are not authorized. Start from verified9986cd4f9/B8 runtime, preserve prior proven improvements. Investigate A (candidate-only six-pixel camera2048 WG), B (historical B4 eight-pixel moving1024 WG), C (5014-pixel culling/shadow control in original and final) separately. Do not assume one cause.

Current batch2 uses isolated codex/render-followup-2 from0b7af5b75; preserve historical followup1. preserve other worktrees, dirty/staged files, servers and processes. No push/main merge/production deployment/shared-service restart. One owner per file; read-only helper for capture review, serial owned GPU/build work.

Verify original archives and actual served sources/WASM/SDK/harness. Reproduce fresh and with prior history, record capture-time inputs and available revisions/resource epochs/commands. Audit capture itself; vary one cache/pool/culling axis at a time. Diagnostic capture/readback is outside performance and production loops. No readPixels/gl.finish/GPU wait in production. Keep tolerances/goldens/inputs exact; preserve failed and adverse attempts. Repair only a proven cause with failing-before/passing-after evidence; if unreproduced or causality unknown, report unresolved. Numerical tolerance changes require a proposal, never silent adoption. B/C remain independent after any A repair.

Use maintained harness and verification gates; checkpoint only owned work, update STATE/EXPERIMENTS, stop after the individually requested batch. Batch2 may implement a bounded measured overlay-selection change; no unrelated refactor or automatic next batch.
