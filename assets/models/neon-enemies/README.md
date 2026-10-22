# Neon enemies — native Blender assets

Three original modular 3D enemies constructed from the supplied concept sheet:

- **Drone Orbiter**: spherical graphite reactor, concentric front energy rings, segmented ivory carapace, three swept radial dagger wings and ventral stabilizer.
- **Blade Wraith**: slim mechanical humanoid, diamond helmet/slit, segmented shoulder/hip/shin plates, two long triangular forearm energy swords.
- **Core Tank**: heavy spherical orange reactor, segmented armor, broad hanging side shields and twin rectangular top cannons.

Each `<name>.blend` is the editable native Blender source. `<name>.glb` uses the reference default (pink drone/wraith; amber tank). Uniform `<name>-pink.glb`, `-blue.glb`, `-green.glb`, `-amber.glb` variants share identical geometry, animation, accessors and binary buffers; only named `Neon | …` material colors change. Ivory and graphite remain unchanged. `studio-stage.glb` is a small independent copy of the established project studio floor.

Models face **+Z**, **+Y is up** in glTF; neutral geometry rests at Y=0. Dimensions/counts live in `manifest.json`. Hidden sides are closed real meshes with rear shell, exhaust/service plates, mechanical joints and armor backs. The inferred hidden design is an interpretation of the single supplied sheet.

## Animation

The rig uses a genuine articulated hierarchy below a `Motion pivot`: Wraith torso → shoulders → elbows/forearms → blade wrists, and torso → pelvis → thighs → shins; Drone wings/armor and Tank shields/cannons follow their reactor body. Native quaternion NLA clips explicitly key translation, rotation **and scale** on every pivot, preventing stale transforms after switching clips.

- `Idle`: **4 s**, loop. Drone hover/wing stabilization, Wraith breathing/guard and head scan, Tank slow aiming sweep.
- `Run`: **1.4 s**, in-place loop. Wraith alternates legs/arms with knee flex and body bounce; Drone banks its stabilizers; Tank leans and cycles its shields. The host supplies world translation.
- `Attack2`: **3 s**. Wraith overhead cleave; Drone single plasma shot; Tank paired cannon shot.
- `Attack3`: **3.6 s**. Wraith lunging thrust; Drone/Tank three paired volleys.
- `Attack4`: **4 s**. Wraith spinning slash; Drone/Tank charged three-volley salvo.
- `Attack`: **3.5 s** Drone/Wraith, **4 s** Tank, recover to neutral. Drone banks in anticipation, dives forward and rolls through recovery. Wraith winds up, twists the torso, sweeps the two swords in sequence with separate elbow/wrist follow-through and leg counterbalance. Tank braces its shields, aims, fires alternating cannons with recoil, body response and animated muzzle plasma flashes.
- `Explode`: **3 s**, one shot. Impact shudder, staggered outward modular blast, rotated debris settles above the stage and remains separated. No automatic resurrection.
- `Assemble`: **2.5 s**, one shot. Exact reversed debris trajectory, ending in neutral assembly. Intended as an explicit preview/game command.

Shooting uses six separate transient plasma pivots, hidden at every neutral endpoint and in non-firing clips. Bolts travel 3.4 m forward over .22 normalized clip time. These are portable visual effects baked into native glTF animation, not game collision, aiming or damage. `combat_animation.py` owns the extra clips/effects. Original Tank Attack also emits two visible bolts.

Authored normalized attack phases: Drone anticipation .19–.30, dive .40, spin .52–.75, recovery .85–1; Wraith wind-up .22–.25, left sweep .40–.43, right sweep .58–.62, recovery .80–1; Tank brace .20, left flash/recoil .335–.34, right .515–.52, recovery .72–1.

Blender sources open with the native Idle tracks active and the inspection camera ready. To inspect another clip, mute Idle and unmute the desired named track on the pivots. The eight named actions/tracks are editable. Meshes are joined per animated rigid pivot with multiple material slots; edit mode retains the modeled faces. V3 replaces the front and rear orb plates with thick curved shell segments and genuinely hollow stepped optical barrels. Wraith has a solid faceted helmet, layered shoulder scales, separated leg armor and tapered energy swords. White-hot filaments are separate geometry/materials that keep the cutting edge readable through the engine bloom. The model is a stylized interpretation, not a pixel-identical reconstruction.

## Rebuild and checks

```sh
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --python assets/models/neon-enemies/build_blender.py
python3 assets/models/neon-enemies/verify_assets.py
```

`design_v3.py` owns the V3 geometry redesign; `build_blender.py` owns the base mechanical assemblies, articulated rig, clips and export. Exact triangle counts live in `manifest.json`. Cylinders, rings and curved shell panels use smooth normals with sharp boundaries; armor bevels use weighted normals. Sharp armor planes and blades intentionally retain defined facets. Wraith uses variable cross sections for its helmet, torso, pelvis, forearms and greaves so the side silhouette also tapers.

The builder creates every asset from native bpy primitives/custom extruded armor meshes, exports glTF, restores exporter-omitted neutral TRS/reset channels, writes all color variants and renders front/hero/rear/exploded inspection views under `output/neon-enemies/`. No old character GLB is imported. Each source contains its studio lights, camera and restrained compositor glow; those are excluded from character GLB exports.

The standard-library checker verifies GLB structure, finite unit normals on every primitive, actual vertex FK bounds against Blender's neutral bounds, complete TRS reset tracks, normalized quaternions, articulated parent chains, looping Idle/recovered Attack, held Explode/reversed Assemble endpoints, muzzle scale reset, and identical geometry/animation across color variants. Native render acceptance and GLB structure do not by themselves prove Rust-renderer or physical Quest acceptance; the independent example runner owns those checks.
