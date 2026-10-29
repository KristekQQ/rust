# Render followup — six separately authorized batches

1. Image correctness and verified starting state (diagnosis recorded; A unresolved, not repaired).
2. Overlay selection (completed2026-10-29: diagnosis/diagnostics accepted; automatic query change rejected).
3. Non-glTF allocations in Neon (completed2026-10-29: GL panel reuse accepted; WG automatic reuse rejected).
4. Camera motion, uploads and worst frames (completed; union defaults retained).
5. Independent remaining-bottleneck audit and Three comparison (completed; bounded mode3 opt-in retained, default unchanged).
6. Final verification and memory/skill update (completed2026-10-30; bounded verdict, image release blockers retained).

Batches1–6 are recorded and this series is complete. See FINAL_REPORT.md; completion is not unconditional image/performance/release acceptance. No additional optimization cycle is authorized. Start from verified9986cd4f9/B8 runtime, preserve prior proven improvements. Investigate A (candidate-only six-pixel camera2048 WG), B (historical B4 eight-pixel moving1024 WG), C (5014-pixel culling/shadow control in original and final) separately. Do not assume one cause.

Historical batch3 reused isolated `/private/tmp/rust-render-followup-2` on `codex/render-followup-3` from `bcd774eb5761c201777c9b8892017e1be494b9af`; preserve prior checkpoints. preserve other worktrees, dirty/staged files, servers and processes. No push/main merge/production deployment/shared-service restart. One owner per file; read-only helper for capture review, serial owned GPU/build work.

Verify original archives and actual served sources/WASM/SDK/harness. Reproduce fresh and with prior history, record capture-time inputs and available revisions/resource epochs/commands. Audit capture itself; vary one cache/pool/culling axis at a time. Diagnostic capture/readback is outside performance and production loops. No readPixels/gl.finish/GPU wait in production. Keep tolerances/goldens/inputs exact; preserve failed and adverse attempts. Repair only a proven cause with failing-before/passing-after evidence; if unreproduced or causality unknown, report unresolved. Numerical tolerance changes require a proposal, never silent adoption. B/C remain independent after any A repair.

Use maintained harness and verification gates; checkpoint only owned work, update STATE/EXPERIMENTS, stop after the individually requested batch. Batch3 implemented the recorded bounded non-glTF allocation experiment; it does not authorize unrelated refactors or automatic next batches. See batch-3/PROTOCOL.md.

Followup4 uses codex/render-followup-4 in the same exclusively owned isolated checkout, base237279b6a. Only dynamic-frame/upload-policy work; see batch-4/PROTOCOL.md.
