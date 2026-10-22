# VR gameplay design notes

Working information for design and further playtesting, not proof that our
scene is fun. Original summaries, without copied extended quotations.
Sources last checked: 2026-10-22.

## Hand movement and transitions between actions

Source: [interview with a Halfbrick developer about Fruit Ninja VR](https://www.gameanalytics.com/blog/fruit-ninja-dev-talks-porting-to-vr).
This describes one team's experience, not a universal controlled experiment.

The team considers the hand's final position after a swing and the next
movement required. Groups of fruit can offer a satisfying combined hit, but
more targets do not necessarily create a physically reasonable challenge.
Layouts should account for player height, reach and blade length, avoiding
uncomfortable interactions close to the face. Device performance and what
players actually do also matter.

Application here: sketch the hand's path and its transition into the next
section. Merely mirroring one curve does not demonstrate a different gameplay
experience. Try readable groups and validate them in a headset. Our specific
counts, spacing and round length remain hypotheses; the article does not prescribe them.

## One core action, multiple ways to master it

Source: [Halfbrick on the original Fruit Ninja VR](https://www.halfbrick.com/blog/slice-storm-fruit-ninja-vr-now-available-htc-vive), 2016-07-07.
A historical game description, not evidence of current compatibility or promised features.

The designers developed slicing for spatial control with two swords and
additional ways of manipulating fruit. The useful design idea is to develop
a familiar action through different situations: every phase does not need a
new weapon or rule. Juggling, skewering and deflecting are not automatically
implemented features of our minute scene. Borrow the design question rather
than promising the referenced game's feature list.

## Score mastery changes the action

Sources: [Halfbrick beginner guide](https://www.halfbrick.com/blog/the-ultimate-beginners-guide-to-fruit-ninja)
(2016) and [Arcade announcement](https://www.halfbrick.com/blog/fruit-ninja-arcade-mode-announced)
(2010). These are historical descriptions of different game versions, not our
score specification. The guide connects patient timing with combo opportunities;
Arcade gives a short score chase changing intensity and combinations of bonuses.
Application: a minute should offer opportunities worth recognizing and executing,
not simply more identical throws. Our current implementation has no freeze,
double-score or frenzy power-up; these sources do not imply that it does.

Source: [Reddit score discussion](https://www.reddit.com/r/FruitNinja/comments/1ejkbmp/63k_arcade_score/).
One player explains choosing several smaller combos over one spectacular combo
because of that version's blitz rewards. Another reports improving after trying
it. This is anecdotal and version-dependent; do not import the stated numerical
threshold into our engine. Useful principle: optimize for the actual reward
function. The biggest-looking cut need not be the best scoring decision.

Source: [Reddit large-combo discussion](https://www.reddit.com/r/FruitNinja/comments/1cm5l9c/163_fruit_combo/).
Players distinguish a large-combo challenge from a highest-score attempt. This
suggests different motivations rather than a universal preferred play style.
Our current task is explicitly high-score mastery; spectacular full-screen
sweeps can remain an option without becoming the only rewarded action.

## Current minute: a testable hypothesis

The JSON at `examples/neon-arena/minute-scene.json` now authors six challenges:
value choice, delayed triple completion, precision, moving alignment, fast
series and a finale. Its 24 cues contain 140 targets, with at most ten live
originals without any cuts. Small valuable targets and larger cheap alternatives
create a selectable difficulty. Timing targets approach an alignment window;
actual Rust physics and headset reach still determine its usability.

Using the existing triple rules, processing authored registration order gives
110 points; selecting valuable matching fruit gives 2440; selecting cheap fruit
gives 350; sorting all targets left to right gives 270. These are diagnostic
orders, not human play records or a proof of an optimal strategy. Browser tests
also probe real tracked sword cuts through native scoring separately from the
full moving-target timeline. Keep those evidence scopes distinct.

## Launch motion and show: latest correction

Honza reports that the opening still repeats and that the flight feels wrong;
he also explicitly wants spectacle that motivates another attempt. Source
inspection found fruit spawned directly at strike height with gravity .5–.8
m/s², while many opening cues reused two straight rows. These are observed
configuration defects, not a user diagnosis of atmospheric density.

The new hypothesis alternates diagonals, a hook and separated fans immediately.
An authored strike point becomes a ballistic apex: native Rust starts fruit
below it with gravity 9.81 m/s². Short launch glints announce the burst without
physics bodies or extra animation timers. There is no aerodynamic drag model.
Check actual motion near the apex and through the fall, not only a screenshot
at creation. Keep the existing earned cut effects and visible combo reward.

## What we have learned with Honza

- More simultaneously visible fruit was accepted as an improvement.
- A long straight row felt too easy; repeating slightly changed arcs did not
  provide the desired minute-long variety.
- Honza explicitly rejected formation variety as sufficient: the player should
  need skill to earn more points. This is design feedback, not a Quest playtest.
- Ask after actual play where a valuable choice felt rewarding, which target was
  unreadable or unreachable, and whether a mistake suggested a better next try.
- Keep internal instructions and research notes in English and summaries in
  concise Czech. Do not turn research into a collage of quotations.

Add observations only when they change a design decision. Identify whether
they come from a source, our hypothesis or an actual player's experience.

## Player correction: readable arcade timing wins over physical realism

Honza reported that the 9.81 m/s² flight was too fast to complete; realistic
physics was not the desired experience. This is direct negative player feedback,
stronger evidence for this task than an analytically correct trajectory.
The revised minute doubles flight time along the same path: half the velocity,
quarter gravity (2.4525 m/s²), apex .84 seconds. Near-apex time within 15 cm is
about .70 seconds instead of .35 seconds. This is an arcade tuning decision,
not atmospheric drag or a claim that a human has accepted the revised timing.
The round remains 60 seconds with the existing sequence and target counts.

## Implemented two-chain prototype

Honza corrected the interpretation: three fruit is not a capacity cap. Keep two
kinds active and allow four, five and longer runs. This is a commitment and
selection challenge, not a demand to swing faster. Every collected fruit must visibly add an icon, including the fourth and fifth;
use shared meshes in horizontal outward rows, capped at 144 per stock for safety. Exact
counters supplement the fruit. No automatic clear at three.

Honza subsequently clarified: a third kind erases one held series and becomes
the new second kind. Replace the oldest-created occupied stock; its bonus has already been credited.
matching hits do not refresh age. Either hand feeds the matching kind.
Every registered original cut immediately earns 10 guaranteed points. Replacing
a stock never takes away any earned points; no points for fragments or duplicate cuts.
Credit positive chain-bonus growth immediately on hits. Left/right trigger frees
the stock without awarding twice; round end also adds nothing. Display the old
series with its positive earned bonus during its eased departure. Honza explicitly
rejected any negative score/loss presentation.

Native chain bonus for count n>=3: base * (n-2)*(n-1)/2, saturating u64.
Thus 3/4/5 produce 1/3/6 base bonuses, added to the hit rewards.
Six cherries banked together earn 160 total; two safe triples earn 80; six
different kinds with no surviving bonus still earn 60. This tests the incentive,
not whether a human enjoyed the minute. Do not publish this formula or advertise a
secret bonus in the player tutorial: actual score growth and feedback support discovery.
Legacy 110/2440 diagnostic orders above use legacy triple mode, not this new mode.

The working preference to retain is concise: arcade comfort, readable choices,
earned show, discoverable depth, and actual player feedback before claims of fun.
Keep implementation specifics in source/tests instead of duplicating chat history.
