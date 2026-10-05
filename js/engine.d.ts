import type * as wasm from "../pkg/webgpu_wasm.js";

export type Vector3 = readonly [number, number, number];
export interface RotationOptions { delay?: number; duration?: number }
export interface LightOptions { position?: Vector3; color?: Vector3 }
export interface OrbitOptions {
    radius: number; height: number; speed: number;
    phase?: number; pulse?: number; color?: Vector3;
}
/** Handle to Rust state. Throws when Rust rejects an invalid command. */
export class SceneObject {
    private constructor();
    readonly id: number;
    readonly exists: boolean;
    setPosition(position: Vector3): this;
    /** Absolute rotation in radians. */
    setRotation(radians: Vector3): this;
    setScale(scale: Vector3): this;
    /** Delta rotation in degrees; timing in seconds. Calculated by Rust. */
    rotate(degrees: Vector3, options?: RotationOptions): this;
    setRenderOptions(options?: {isStatic?: boolean; castsShadow?: boolean; receivesShadow?: boolean; reflectivity?: number}): this;
    spin(radiansPerSecond: Vector3): this;
    removeAfter(seconds: number): this;
    remove(): void;
}
export class SceneLight {
    private constructor();
    readonly id: number;
    readonly exists: boolean;
    set(position: Vector3, color: Vector3): this;
    orbit(options: OrbitOptions): this;
    stopOrbit(): this;
    removeAfter(seconds: number): this;
    remove(): void;
}
/** Commands to the Rust SceneManager. Contains no JS scene or animation state. */
export class Scene {
    private constructor();
    readonly objectCount: number;
    readonly lightCount: number;
    createCube(position?: Vector3): SceneObject;
    createPlane(position?: Vector3): SceneObject;
    createSphere(position?: Vector3): SceneObject;
    createLight(options?: LightOptions): SceneLight;
    clear(): void;
    loadExample(): void;
    setMirror(object: SceneObject): void;
    loadEffectsDemo(): void;
    loadCullingDemo(): void;
}
export type RawEngine = Pick<typeof wasm,
    "add_cube" | "add_plane" | "add_sphere" | "remove_object" | "clear_scene" |
    "set_object_transform" | "set_cube_transform" | "add_light" | "remove_light" |
    "clear_lights" | "set_light" | "set_light_orbit" | "clear_light_orbit" |
    "schedule_rotate" | "schedule_remove_object" | "schedule_remove_light" |
    "set_camera_mode" | "set_grid_visible" | "resize" | "renderer_backend" | "set_frustum_culling" | "set_render_effects" | "set_simulation_paused">;
export interface RenderStats {
    total: number; visible: number; culled: number;
    /** Object batches only, excludes optional debug lines. */
    drawCalls: number;
    uploadBytes: number;
    /** Rust simulation and preparation/encoding/submission of all passes; excludes GPU execution. */
    prepareMs: number;
    /** Averaged browser RAF rate, calculated in Rust; not GPU timestamp timing. */
    fps: number;
    frameMs: number;
}
export interface EffectStats {
    staticUpdates:number; dynamicUpdates:number; reflectionUpdates:number;
    staticVisible:number; dynamicVisible:number; reflectionVisible:number;
    passes:number; shadowResolution:number; reflectionWidth:number; reflectionHeight:number;
}
export type Engine = Readonly<RawEngine & { readonly scene: Scene; set_output_mode(mode: "canvas" | "texture"): boolean; effects_stats(): EffectStats; renderer_stats(): RenderStats }>;
/** One canvas with id gpu-canvas per page. Await before issuing commands. */
export function initEngine(canvas: HTMLCanvasElement, options?: { backend?: "auto" | "webgl" }): Promise<Engine>;
