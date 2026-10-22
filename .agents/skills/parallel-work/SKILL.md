---
name: parallel-work
description: Coordinate concurrent chats or agents in this repository using worktree isolation, file ownership, integration handoffs and reproducible Quest releases. Use when concurrent work is known or before integrating another task; ordinary isolated edits need no registry ceremony.
---

# Concurrent repository work

Follow [AGENTS.md](../../../AGENTS.md). This skill grants no extra permission to
push, deploy, message other chats or move their active work.

## Detect changes economically

Check at task start, before long verification/builds and before integration/push;
recheck affected paths after surprises. Do not poll the repository or read full
chat histories after every edit.

Inspect branch/HEAD, `git status --short`, `git worktree list` and relevant task
records. Distinguish a **recorded claim**, **changed content**, and **unknown
authorship**. Dirty status, mtime, process names or `/root` do not identify a chat.
An unchanged dirty-file list can hide further edits to already dirty files.

After intended edits, freeze verification inputs: HEAD/index and relevant content
hashes, including new files and generated artifacts used by the check. Compare
afterwards. Own expected edits are not foreign interference. Before/after hashes
cannot detect a temporary edit restored between snapshots; isolated stable inputs
are stronger evidence.

Reuse existing fingerprints. Browser verification covers Rust/WASM and
SDK/tests/examples; production validation checks deployed bytes. Compare initial
and final build identity: final `assertFreshBuild()` alone can pass if another
complete build replaces sources, WASM and its receipt. Rust's fingerprint excludes
JS/HTML/assets. Do not rerun GPU tests merely to identify changed files.

| Observation | Action |
|---|---|
| Disjoint files change in another worktree | Continue; this checkout's inputs are unaffected. |
| Another task claims a needed file/output in this checkout | Sequence ownership or isolate. |
| Relevant bytes change with no known owner | Preserve evidence/versions; report unknown origin. Do not guess authorship or revert automatically. |
| Inputs change during verification | Invalidate affected evidence, stabilize inputs, repeat affected checks. |

## Record ownership and isolate work

The shared local registry is `codex-coordination/` beneath the absolute directory
returned by `git rev-parse --path-format=absolute --git-common-dir`. Linked
worktrees share it; separate clones require a handoff. It is untracked and
advisory, not an atomic lock or scheduler. Missing/stale records prove neither
inactivity nor ownership; age alone never revokes a claim.

Each task writes only its own uniquely named Markdown record. Keep one current
status/checkout; mark prior locations historical. Include unique chat/task ID
(not just `/root`), checkout, branch/base SHA, owned paths/hunks, status, update
time and integration owner/remote target if assigned. Update on ownership or
checkout changes and handoff, not every tool call. Handoffs add exact commits,
focused evidence, build identity and limits. Keep live status here, not MEMORY.md.

One writer per file/output per checkout. Independent substantial tasks use
separate worktrees and `codex/` branches; small disjoint docs edits can stay in
place. Reuse suitable attached checkouts. Choose the intended base explicitly:
remote main, local HEAD and dirty work may differ; worktrees do not copy dirty
changes. Isolation does not resolve overlapping interfaces or semantic conflicts.

Preserve unrelated edits/staging; never stash/reset/clean or commit them for
another task. Already interleaved work needs owned-hunk extraction. Resolve only
the contested file/output; continue independent work. Do not migrate active chats,
stop their processes or archive their worktrees without authorization.

## Integrate and publish

One recorded integrator owns a shared remote release target such as `origin/main`,
even when workers have different local branches. Resolve conflicting release
claims before publication. Workers may push explicitly authorized isolated
branches; that does not authorize publishing other tasks' dirty work.

Integrate only agreed changes, review conflicts, run the required final gate once
on the final inputs and follow content-bound receipt rules. No hook bypass or
force-push. Serialize `pkg/`, `output/production/`, `all_sources.txt` and shared
verification reports within a checkout. Do not replace another task's testing or
serving package; separate worktrees keep build outputs independent.

Freeze benchmark source/package/scene. Bind Quest runs to served source/WASM
fingerprints, backend and settings, not just a scene label or Git SHA. Use
[frame-pipeline](../frame-pipeline/SKILL.md) for API timing boundaries.
Report edits, commit, push, CI and physical headset acceptance separately.

Existing chats may need to reread AGENTS.md; do not claim automatic adoption.
