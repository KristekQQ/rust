# Krystalová jeskyně / Crystal Cave

Standalone Rust/WASM engine example with an original enclosed Blender/glTF cave.
Open `/examples/crystal-cave/` through the existing `npm run serve:quest` HTTPS server,
then choose **Vstoupit do VR** on Quest. `?backend=webgl` selects the fallback.

- Left stick: head-relative walking at 2 m/s; right stick: 30-degree snap turn.
- Hold right A to aim at clear floor, release to teleport up to 10 m; B cancels.
- Both controller models follow native tracked grips. FPS is an optional spatial panel.
- Desktop buttons show the entrance and the rear crystal chamber.

`main.js` initializes the engine and XR lifecycle; `scene.js` owns environment,
lighting and navigation configuration. Runtime tracking, locomotion, collisions,
teleport arc and panel positioning remain in Rust. No other example is imported.

Environment: about 18 x 38 m, 8,034 triangles, eight material primitives, 709,308-byte
GLB. `assets/models/crystal-cave/` contains editable `.blend`, deterministic builder,
export validator and layout. Nine test destinations cover the clear central lane
and side branches. Navigation uses authored floor rectangles and 27 obstacles,
not a mesh navmesh. The walkable head-center bounds are inset from cave walls.
Physical room-scale motion is not constrained by virtual collision boxes.

Verification:

```sh
npm run typecheck
VERIFY_WASM_PROFILE=release npm run test:browser -- --page crystal-cave,crystal-cave-xr --backend all --capture
```

The browser checks render entrance, rear chamber and ceiling on both backends.
The XR simulator checks stereo, both hands, walk/snap/teleport, all nine authored
pads, bounds, tracking loss, optional right-foot floor-anchored panel and reentry. These are
ordinary-browser/simulated-XR checks; actual headset comfort and FPS need Quest
acceptance. No physical-headset performance claim is made.
