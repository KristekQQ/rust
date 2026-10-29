# Followup2 — complete; automatic optimization rejected

2026-10-29. Isolated `codex/render-followup-2`, base `0b7af5b75`. [Report](batch-2/REPORT.md), [commands](batch-2/COMMANDS.md), [manifest](batch-2/MANIFEST.json), [verification](batch-2/VERIFICATION.json). Original B8/f14c4beb preserved.

Proven redundant opaque primitive scans; diagnostic immutable guard removes them (WG2048 130287→0 primitive visits/call). Automatic default rejected on BOTH APIs: lower CPU wall time did not consistently improve whole-frame stress cadence/tails. Large dynamic WG workload improved in these runs, but has a strict image failure. Original selection remains default; `setOverlaySelectionReference(false)` is experimental opt-in only. No production FPS gain claimed.

Final release WASM `d0677791488085cb26b7423a4c062fb4411044e90f55d958193558bd7c5f0798`, source `7cd160e788ece6dfd78b6d913957ba758cb19dc8ad4abae0d52c6114d263ca4f`, bindings `504f0f506ebe4e1839bf3dfc4b169109b35b8ba8c490fdf1c99f5665827080fe`. Current isolated pkg/source/SDK/tests match. One fresh full gate:428native(2ignored),151browser(56WG/95GL),SDK32,XR71,harness14Node+13Python PASS. New exact overlay states21WG/25GL plus per-eye BLEND/asymmetric simulatedXR PASS. Full-cut regressions cover delivered default; no complete worker-cut/fade/dissolve pixel oracle for experimental opt-in.

90clean runs/102137RAF rows, three alternating pairs/config:32exact/1FAIL/12unavailable. Six original→restored-default controls exact, small mixed timing differences. Profiling verified off; clean GPU times unavailable. Nonisolated desktop, no physical Quest. CaseD clustered1024WG:four pixels/max1; both variants also occur in original runtime. Mechanism unresolved, FAIL retained independently of A/B/C. No tolerance/golden change.

Local checkpoint/evidence only. No push/merge/deploy/shared restart. Batch2 stops here; batch3 requires separate authorization. Prior batch1 [report](REPORT.md): A unresolved/not reproduced, B and C separate/open.
