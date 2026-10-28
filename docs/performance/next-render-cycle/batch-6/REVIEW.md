# Independent read-only review

One bounded researcher/reviewer (`index_restart_review`, Sol Medium), no edits/builds/GPU runs/delegation. Root owns implementation and runtime verification.

Reviewed pure index preparation, restart boundaries against current official specs and pinned wgpu/GLES source, format propagation Geometry→Draw→PreparedDraw, strip pipeline cache keys, logical draw counts versus aligned storage, shared geometry ownership, live policy guard, original-U32 desktop and actual-eye XR references. No actionable correctness findings in that scope.

The review confirmed pinned wgpu-core requires zero depth bias for point/line pipelines. The bounded repair keeps existing triangle shadow bias and makes other supported topologies valid. Expanded morph/skin fixture was source-reviewed; the suggestion to prove those branches explicitly was incorporated with cpuDeformedVertices and skinPaletteUploadBytes assertions. Browser execution and pixels are root's evidence, not claims from source review.

Limits: no physical Quest, no GPU runtime by the reviewer, no general renderer audit. The inherited B4 crowded-moving-WebGPU exact-image issue is outside this batch and remains open.
