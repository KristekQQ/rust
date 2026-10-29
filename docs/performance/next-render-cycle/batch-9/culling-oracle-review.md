# Culling-off oracle review

CPU-only PNG decode and targeted source inspection; no browser, GPU, build or source changes. Evidence: `camera-oracle/report.json`, all 18 associated PNGs, and its diagnostic driver.

## Observed distribution

All three sessions repeat the same two exact PNG identities: cached/restored `a5d531295d3ef42f6de3c4383cfd102f165c8b40c12926242f2d5789599b5979`, combined-off `6d115af31c00ff07539ba225e96fdb917717358e39c744a3493308ffa3cd1673`. Consecutive captures in each state are exact. Public camera states match.

Independent CPU decode reproduces 5,014 changed pixels / max channel delta 79. Bounding box is x=7..1248, y=0..590 in 1280×720; no differences occur below row 590. Changed-pixel counts in successive 90-row bands are 399, 664, 855, 971, 1123, 785, 217, 0. Of changed pixels, 575 exceed delta 8 and 44 exceed delta 32. Among 5,006 changed pixels away from the image boundary, 3,843 have an adjacent cached-image RGB channel gradient above 20; differences therefore frequently lie near image edges, but this does not establish rasterization as the cause. Off-minus-cached RGB sums are positive at 2,437 pixels and negative at 2,551; the effect is not uniform brightening/darkening.

All six coordinates of the prior intermittent camera-stress pair-1 failure are unchanged between these cached/off states. The pixel sets are disjoint. A shared source cause remains possible but is not evidenced by location.

## Exact flag semantics

- `src/web.rs:637–641` sets one global renderer `frustum_culling` boolean. It does not change visibility/render options, camera, transforms or authored fixture content.
- `src/visibility.rs:302–332` selects eligible roots by pass-frustum spatial query when enabled; disabled selects every pass-eligible object. The enabled query sorts IDs (`:247`); disabled traverses scene objects stored in `BTreeMap` (`src/scene.rs:311`). Imported draw IDs likewise traverse `BTreeMap` (`src/visibility.rs:274,384–388`). No simple enabled/disabled reversal of common root ordering was found.
- The flag is also part of static/dynamic shadow-map keys (`src/render/effects.rs:350–356`). Toggling it forces shadow-map regeneration and changes light-frustum caster selection through `prepare_shadow_pass`, not just the main camera set. Shadow glTF preparation receives the flag too (`src/render/renderer.rs:1639`). `casts_shadow`/`is_static` eligibility still applies (`src/visibility.rs:266–267`); stress drones explicitly do not cast shadows (`examples/rim/stress.js:78`).
- Disabling the flag also bypasses primitive-frustum tests (`src/render/gltf_gpu.rs:2646–2667`) and changes eligible membership/group formation. It can therefore alter batching boundaries even while camera and visible authored scene remain fixed.

The larger submitted set is intentional. Correct conservative culling plus hardware clipping should preserve visible geometry; the flag does not deliberately reveal hidden fixture content. Nevertheless, the observed difference cannot yet be attributed to a wrong bound, shadow selection, regrouping/representation, cache or another cause. Shadow caster selection legitimately changes as an optimization mechanism; a visible output change is not thereby excused.

## Oracle interpretation and next step

The driver correctly establishes an uncached/unculled reference (`fineCacheBytes===0`, cache disabled), fixed camera, stable state and exact comparisons. This is an independent **combined equivalence** test and it fails materially. It is not an isolated prepared-cache test because global main/primitive/shadow culling changes simultaneously. The fixture's multiple passes make that distinction consequential.

The smallest next action is the already planned serial isolation: final cached, cache-only off, cull-only off, both off, restored; plus original B1 cull-only off, at the same deterministic path, appearance and viewport. Save consecutive pixels and available draw/cull/plan counters per axis. This distinguishes cache dependence from global culling and tests whether the latter predates B2–B8. If culling alone differs on both builds, a later bounded diagnostic must hold shadow culling/maps fixed while changing only main/primitive culling (or vice versa); do not infer that result now. Keep exact tolerance and retain both the new 5,014-pixel failure and original six-pixel failure as unresolved acceptance evidence.
