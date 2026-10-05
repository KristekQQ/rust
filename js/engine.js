// JavaScript bridge to the Rust engine. GPU state and the frame loop live in Rust.
// The current Rust API supports one canvas with the id "gpu-canvas" per page.
function accepted(ok) {
    if (!ok) throw new Error("Rust engine odmítl příkaz: neplatný objekt nebo parametry.");
}

// These handles hold only an ID and a WASM binding, never scene/simulation state.
export class SceneObject {
    #wasm;
    constructor(wasm, id) { this.#wasm = wasm; this.id = id; Object.freeze(this); }
    get exists() { return this.#wasm.object_exists(this.id); }
    setPosition(position) { accepted(this.#wasm.set_object_position(this.id, ...position)); return this; }
    setRotation(radians) { accepted(this.#wasm.set_object_rotation(this.id, ...radians)); return this; }
    setScale(scale) { accepted(this.#wasm.set_object_scale(this.id, ...scale)); return this; }
    rotate(degrees, { delay = 0, duration = 1 } = {}) {
        accepted(this.#wasm.schedule_rotate(this.id, ...degrees, delay, duration)); return this;
    }
    setRenderOptions({isStatic=false, castsShadow=true, receivesShadow=true, reflectivity=0} = {}) {
        accepted(this.#wasm.set_object_render_options(this.id,isStatic,castsShadow,receivesShadow,reflectivity)); return this;
    }
    spin(radiansPerSecond) { accepted(this.#wasm.set_object_spin(this.id,...radiansPerSecond)); return this; }
    removeAfter(seconds) { accepted(this.#wasm.schedule_remove_object(this.id, seconds)); return this; }
    remove() { accepted(this.#wasm.remove_object(this.id)); }
}

export class SceneLight {
    #wasm;
    constructor(wasm, id) { this.#wasm = wasm; this.id = id; Object.freeze(this); }
    get exists() { return this.#wasm.light_exists(this.id); }
    set(position, color) { accepted(this.#wasm.set_light(this.id, ...position, ...color)); return this; }
    orbit({ radius, height, speed, phase = 0, pulse = 0, color = [1, 1, 1] }) {
        accepted(this.#wasm.set_light_orbit(this.id, radius, height, speed, phase, pulse, ...color)); return this;
    }
    stopOrbit() { accepted(this.#wasm.clear_light_orbit(this.id)); return this; }
    removeAfter(seconds) { accepted(this.#wasm.schedule_remove_light(this.id, seconds)); return this; }
    remove() { accepted(this.#wasm.remove_light(this.id)); }
}

// Thin command facade for the Rust SceneManager, not a second manager in JS.
export class Scene {
    #wasm;
    constructor(wasm) { this.#wasm = wasm; Object.freeze(this); }
    get objectCount() { return this.#wasm.scene_object_count(); }
    get lightCount() { return this.#wasm.scene_light_count(); }
    createCube(position = [0, 0, 0]) { return this.#create(this.#wasm.add_cube(...position)); }
    createPlane(position = [0, 0, 0]) { return this.#create(this.#wasm.add_plane(...position)); }
    createSphere(position = [0, 0, 0]) { return this.#create(this.#wasm.add_sphere(...position)); }
    #create(id) {
        accepted(id !== 0xffffffff);
        return new SceneObject(this.#wasm, id);
    }
    createLight({ position = [0, 0, 0], color = [1, 1, 1] } = {}) {
        const id = this.#wasm.add_light(...position, ...color);
        accepted(id >= 0);
        return new SceneLight(this.#wasm, id);
    }
    clear() { this.#wasm.clear_all(); }
    loadExample() { this.#wasm.load_example_scene(); }
    setMirror(object) { accepted(this.#wasm.configure_planar_mirror(object.id)); }
    loadEffectsDemo() { this.#wasm.load_effects_demo(); }
    loadCullingDemo() { this.#wasm.load_culling_demo(); }
}

export async function initEngine(canvas, { backend = "auto" } = {}) {
    if (!(canvas instanceof HTMLCanvasElement) || canvas.id !== "gpu-canvas") {
        throw new Error('Engine vyžaduje canvas s id="gpu-canvas".');
    }
    canvas.dataset.backend = backend;
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;

    const version = Date.now();
    const wasm = await import(`../pkg/webgpu_wasm.js?v=${version}`);
    await wasm.default({ module_or_path: new URL(`../pkg/webgpu_wasm_bg.wasm?v=${version}`, import.meta.url) });
    await wasm.start();

    return Object.freeze({
        scene: new Scene(wasm),
        add_cube: wasm.add_cube,
        add_plane: wasm.add_plane,
        add_sphere: wasm.add_sphere,
        remove_object: wasm.remove_object,
        clear_scene: wasm.clear_scene,
        set_object_transform: wasm.set_object_transform,
        set_cube_transform: wasm.set_cube_transform,
        add_light: wasm.add_light,
        remove_light: wasm.remove_light,
        clear_lights: wasm.clear_lights,
        set_light: wasm.set_light,
        set_light_orbit: wasm.set_light_orbit,
        clear_light_orbit: wasm.clear_light_orbit,
        schedule_rotate: wasm.schedule_rotate,
        schedule_remove_object: wasm.schedule_remove_object,
        schedule_remove_light: wasm.schedule_remove_light,
        set_camera_mode: wasm.set_camera_mode,
        set_grid_visible: wasm.set_grid_visible,
        set_output_mode: wasm.set_output_mode,
        set_render_effects: wasm.set_render_effects,
        set_simulation_paused: wasm.set_simulation_paused,
        effects_stats() {
            const [staticUpdates,dynamicUpdates,reflectionUpdates,staticVisible,dynamicVisible,reflectionVisible,passes,shadowResolution,reflectionWidth,reflectionHeight] = wasm.effects_stats();
            return {staticUpdates,dynamicUpdates,reflectionUpdates,staticVisible,dynamicVisible,reflectionVisible,passes,shadowResolution,reflectionWidth,reflectionHeight};
        },
        resize: wasm.resize,
        renderer_backend: wasm.renderer_backend,
        set_frustum_culling: wasm.set_frustum_culling,
        renderer_stats() {
            const [total, visible, culled, drawCalls, uploadBytes, prepareMs, fps, frameMs] = wasm.renderer_stats();
            return {total, visible, culled, drawCalls, uploadBytes, prepareMs, fps, frameMs};
        },
    });
}
