---
name: zabavnost
description: Design and tune VR gameplay decisions, score mastery, pacing and natural hand movements. Use for repetitive or boring play, wave design and level authoring; use the relevant project skills for renderer performance and XR APIs.
---

# Fun and player skill

Build a minute the player wants to replay: readable opportunities, satisfying
physical actions, a little spectacle and room to improve. Keep this skill and
research notes in English; give Honza concise Czech summaries. Do not claim
personal headset sensations or assume a technically correct game is enjoyable.

## Decisions that matter

- Imagine a player chasing a better score. Specify which target to choose or
  leave, when to act, and what a mistake really costs. More objects, phase names
  or slightly different arcs do not by themselves create a new challenge.
- Mix priority, precision, delayed completion, timing and fluent hand transitions.
  Contrast the opening throws immediately. Allow success and recovery before
  combining learned actions in the finale; the round stays 60 seconds.
- Honza found scale .06 frustrating to hit. Standard playable fruit is now .09;
  precision targets are .075, not .045. Keep targets comfortably hittable: mastery
  should come from choices, timing and execution, not forced miniature targets.
- Design the hand's start, strike and end position. Placement suggests a hand or
  direction; it does not enforce either unless native rules actually do.
- Side controls must work from the same play position: turn, see and tap within
  comfortable sword/arm reach. Face the panel toward the player at chest height;
  do not solve accidental presses by making the player walk. Verify a real blade
  tap with a stationary head position and nearby grip, not a teleported ray.
- At round end, place START in front of the current head direction within
  stationary hand reach. Restore its rear play-time position when the next round
  starts; verify both-eye visibility and actual activation.
- Favor readable arcade timing over realism. Honza rejected the .42-second
  realistic apex as too fast. The current .84-second apex follows the same path
  at half speed and doubles the strike window. Do not speed it up merely to
  match real gravity. Rust has gravity/collisions, not atmospheric drag.
- Use anticipation and earned spectacle: brief launch glints, actual cut feedback
  and a clear reward. Effects must preserve silhouettes and frame time. Decorative
  launch objects must not change fruit collisions.
- Honza values discovery and earned competitive mastery: "I figured something
  out, and can use it to beat my record or other players." Hide advanced
  combinations, not basic controls; let consistent score and visual feedback
  reveal them. Better choices and execution should create a learnable advantage
  across replays. Do not explain the secret bonus in player-facing tutorials.

## Current neon arena rules

The arena opts into two independent fruit-kind chains. Every registered original
cut immediately earns 10 guaranteed points, including wilds and a new third kind.
This keeps beginners rewarded; fragments and duplicate hits never earn points.
Three is the first bonus threshold, not a storage cap: longer chains continue
and are more valuable. Credit each positive increase of the chain bonus immediately alongside hit points.
Each collected fruit gets a real shared-mesh icon, arranged horizontally outward (left stock left, right stock right),
plus a tiny plain debug counter (no glow, FPS bitmap font). Counts are secondary
to the fruit and earned effects. Four/five must visibly show four/five fruit, not three
samples. The display caps at 144 icons per stock (the minute has 140 targets);
reuse icons, remove surplus on replacement/bank/reset. Keep fruit/frame base
scale and horizontal spacing constant at every count. Honza explicitly prefers
a row continuing out of view over shrinking the earned fruit to fit.
A collection must be a readable little show, not an icon appearing in place.
Fruit and its own frame arrive together from the outer side (left from left,
right from right), already visible at 55%; ease to 135%, squash to 75%, settle
at 100% over .76 seconds with a fading neon highlight. Give every collected icon
a matching shared frame, including fourth/fifth fruit. Empty stocks draw no
placeholder frames. Earned frames remain fully lit in the resting state, at the
same brightness as during arrival; do not dim the player's visible progress when
the effect ends. Landing briefly boosts the fruit, then its accent fades while
the frames keep glowing. Anticipation, arrival and reward have distinct timing.
Keep the motion local
and smoothly eased rather than repeated or full-screen flashing. Any hand feeds either kind.

New kinds take an empty stock. A third kind replaces the oldest-created occupied
stock and starts at one. All earned hit points AND chain bonuses remain in the
main score. Matching hits do not refresh creation age. Left/right trigger releases
that stock; neither release nor round end pays its bonus again. Wild fruit joins
a held chain. Misses do not clear chains; fragments never score as new fruit.

Keep the basic +10-per-cut and bank controls explicit; the paytable lists triple
bonuses, not total scores. The larger-chain advantage is intentional. Retain its exact arithmetic in native
code/tests and [design notes](references/design-notes.md), not UI hints. The SDK
also retains opt-in legacy triple scoring for other callers and regression tests;
do not describe those legacy rules as the arena's current gameplay.

Show each stock's total earned points (10 per collected original plus its bonus), upright beside its corresponding stock;
pulse only the hit stock. The cut popup shows that same stock's cumulative total:
+10 for the first, +20 for the second, then all stored hits plus its growing bonus.
Keep different originals in the same sweep distinct. Native scoreAwards carries
both the actual awarded delta and the cumulative total; the arena chooses stock-total.
Never show +0 for collected fruit; zero-reward fragment cuts retain the effect
without a numeric caption. On occupied-kind replacement, transfer the outgoing row's real
fruit/frame instances to an eased flight below the score, then fade and remove
them after .95 seconds. Include the total earned chain points as a POSITIVE
number traveling and fading with that row; remove its transient panel too.
Never imply a deduction: this value is already included in the main score. Retain at most one outgoing row and clear it on reset or
further replacement. Keep all scoring and effect timing native; do not clone
geometry or add a JS score/animation loop.

## Author, verify and learn

Use the [engine workflow](../rust-webgpu-project/SKILL.md) and
[XR layout conventions](../scene-object-layout/SKILL.md). Rust owns scoring,
physics, display updates and effect lifetimes; JS/JSON issue commands. Avoid a
second scoring loop, per-frame UI polling, unbounded history or duplicate rules.

The [minute plan](../../../examples/neon-arena/minute-scene.json) authors the
sequence; `arena_preview`, `arena_configure`, `arena_save` and `arena_export`
share human/AI controls. `arena_bank({hand:"left"|"right"})` uses the same native
banking command as triggers. LocalStorage presets belong to browser/origin;
JSON transfers settings. Minute geometry is authored; practice uses wave controls.

Check an entire no-hit round: at most ten originals, every cue admitted, finale
cleared, one director timer and clean stop/restart. Check actual moving cuts and
native scoring: independent chains, counts beyond three, oldest-stock replacement, both stocks'
release, all earned points surviving replacement, no double payment on release/end
and bounded display instances. Compare
naive and deliberate choices under the actual selected scoring mode.

Simulated XR verifies rendering, timing and native rules, not comfort, headset
FPS or human enjoyment. Honza's feedback overrides a prettier screenshot or
analytically correct physics. Record only a short decision-changing observation;
keep supporting articles and historical detail in the linked design notes.
