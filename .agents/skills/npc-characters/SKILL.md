---
name: npc-characters
description: Create or tune companions, flying targets and enemies in this Rust engine. Use for NPC behavior, round lifetime, interaction, view-dependent billboard artwork, atlas authoring and choosing between sprites and 3D characters.
---

# Build a character around its behavior

Start by deciding what survives a round restart, what triggers an interaction, and which parts depend on who is looking. Keep identity, movement, interaction progress and rewards in the authoritative simulation. Each camera can select different artwork for the same instant. Opening another observer must never trigger another visit, hit, gift or score.

## Choose the existing implementation

- **Neon cat.** Follow `SpriteController`, `interact_sprites` and `pet_hand_intent` in [sprite.rs](../../../src/sprite.rs). The ambient visit continues across rounds. The round owns gift eligibility. A fresh sustained stroke during an eligible round earns the delivery; idle contact in an earlier round must not carry credit into the next. Exercise approach, waiting within reach, stroking, pickup and departure. Repeat with one tracked hand and a restart during the visit.
- **Carried prop.** The can has its own [XP drink state](../../../src/xp_drink.rs). The carrier provides a mouth attachment and visit identity until pickup; the hand and then the drink action take over. Consuming it produces the reward once. Check the transition at the mouth, grip and face. The cat's painted pose alone must never award the buff. Follow the [Neon game integration](../../../games/neon-core/src/game.rs) for round and score policy.
- **Flying target.** Reuse the sprite's optional sword target, swept contact and fragment clipping. The hit determines the cut line on the quad and the two pieces keep that cut as they rotate. Scoring is a separate round rule. For a new flock, check respawn, repeat hits and the player's standing reach; looking down should not move the flight path onto their head.
- **3D enemy.** Inspect [Rim's enemy state and pools](../../../src/rim_game.rs) for navigation, attacks and damage. Share a small capability when two actual users agree on its semantics. Keep locomotion, rendering form and allegiance independent. The sprite controller and Rim enemy are distinct existing implementations.

## Author views and verify transitions

For new or revised cat art, keep independently editable poses and the [frame manifest](../../../examples/neon-arena/assets/stealth-cat-frames/manifest.json). The [atlas packer](../../../scripts/pack-cat-carry-atlas.mjs) builds the atlas and previews. Check tile dimensions, transparent margins, silhouette, foot anchor and mouth position at each transition. Imported bird sheets can retain their source atlas and trim metadata; see the [bird packer](../../../scripts/pack-slot-birds.py).

Use the travel heading for front, side and rear views. Select close or elevated artwork from camera elevation as the player looks down. Stable feet, a readable face and continuous animation matter most here. Compare the player and a side observer at the same moment, especially when the character turns to leave. A rigid 3D character can use the same interaction rules with its own 3D pose.

Choose a focused check for the change. For a reward transition run `cargo test --lib sprite_earn_fetch`; for networked pose ownership run `cargo test --lib sprite_semantic_binding`. Visual fixtures cover [elevated view](../../../tests/cat-elevated-view.html), [directions](../../../tests/cat-directional-view.html) and [atlas bands](../../../tests/cat-atlas-bands.html). A headset pass should include looking down at arm's length and approaching from either side.
