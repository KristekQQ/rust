# Parallel-work guidance verification — 2026-10-22

Scope: the new `.agents/skills/parallel-work/SKILL.md` and its routing/decision
entries in `AGENTS.md` and `MEMORY.md`. No rendering, gameplay, physics, SDK or
build-script changes are included. The primary checkout's unrelated edits and
staging are preserved. Publication uses `codex/parallel-work-guidance`, based on
`7c6ccfd51d618fc27e97af85d7be587f50469d84`; integration into `origin/main` is separate.

## Proportionate checks

- Skill Creator `quick_validate.py`: PASS.
- Relative Markdown navigation links in the affected guidance: PASS.
- Scoped `git diff --check`: PASS.
- Independent read-only behavioral review: no material findings for (1) two
  worktrees targeting the same remote release branch, (2) changed dirty-file bytes
  and a replaced fresh WASM package during verification, (3) unregistered changes
  in a separate worktree plus an ambiguous old owner record.

These checks validate documentation structure and intended decisions. They do
not demonstrate enforcement against non-cooperating agents or actual headset
performance. No watcher, lock service or runtime behavior was added.

## Required publication gate

The repository requires `npm run verify:push` before publication, even though a
docs-only change does not itself require a GPU rebuild. A matching content-bound
full PASS may be reused; otherwise the gate runs the native, WASM, SDK, harness,
types and registered WebGPU/WebGL browser checks and regenerates `all_sources.txt`.
This worktree has independent build/output paths. The installed pre-push hook
must reuse matching evidence rather than running the suite again.

First run: all 283 native tests (2 ignored) and all registered browser scenarios
passed, but final receipt creation rejected new `scripts/__pycache__` files
created by the Python harness. This was a generated artifact change, not evidence
of another agent editing source. The task-owned bytecode directory was removed;
retry uses `PYTHONDONTWRITEBYTECODE=1`, with no test or hook bypass.

Second run: the generated-artifact issue was absent; 47/48 browser scenarios
passed, while the existing `neon-arena` WebGPU cut-face pixel probe reported
`96/169`. It passed in the first full run on identical runtime source. No runtime
or assertion change was made to suppress the failure. A focused rerun of that
scenario precedes the final required gate.

Final result: **PASS**, `VERIFY_WASM_PROFILE=release PYTHONDONTWRITEBYTECODE=1
npm run verify:push`, native ARM64 Node. The focused cut-face rerun also passed.

- Rust formatting, 283 native tests (2 ignored), native Clippy: PASS.
- SDK, harness, XR platform, types: PASS.
- wasm32 check/Clippy and matching release WASM: PASS (build reused on retry).
- Browser: 48/48 PASS; 21 WebGPU and 27 WebGL scenarios, including simulated XR.
- Full content-bound PASS receipt recorded; source dump regenerated.
- Tested Rust source fingerprint: `085d268a855d5634206b9dc9a0c9ed2666b9bf34b925bb2dae13ec1f1c8a3ae7`.
- Tested WASM SHA256: `37bd0db2f708d761a6a2c0fc51a73516dd9b917651ffcfc11f9279bfe3fb141c`.

Actual Quest execution and performance were not tested by this documentation
change. The intermittent cut-face probe remains a known verification limitation;
passing the final run is not evidence that its instability has been fixed.
