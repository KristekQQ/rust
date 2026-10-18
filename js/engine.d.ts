import type * as wasm from "../pkg/webgpu_wasm.js";

export type Vector3 = readonly [number, number, number];
export interface TriangleMeshOptions {
    /** Flat xyz buffer; indexed watertight, outward oriented, non-self-intersecting solid. */
    positions: readonly number[] | Float32Array;
    indices: readonly number[] | Uint32Array;
    color?: Vector3;
    /** Flat RGB per triangle. Omit for uniform color. */
    triangleColors?: readonly number[] | Float32Array;
    /** Area-weighted shared-vertex skin normals; newly cut caps remain flat. Default false. */
    smooth?: boolean;
}
export interface CutterAimOptions {center:Vector3;yaw?:number;tilt?:number;height?:number;floorY?:number;guides?:readonly SceneObject[]}
export interface CameraViewState {position:Vector3;orbitTarget:Vector3|null}
export interface TriangleMeshInfo {
    triangles: number; vertices: number; volume: number; closed: boolean; cuttable: boolean;
    resident: boolean; cpuMaterialized: boolean;
    /** Fraction of the actual cross-section swept by the sharp edge (0..1). */
    cutProgress: number;
    /** Cutter object ID while fruit is wedged on a blade; otherwise null. */
    jammedBy: number | null;
    /** GPU residents return conservative support (bounds outside the sampled animation path). */
    position: Vector3; rotation: Vector3; bottom: number;
}
export interface TriangleMeshSnapshot {positions:Vector3[];triangles:[number,number,number][];normals:Vector3[];colors:Vector3[]}
export interface GpuMeshSnapshot {positions:Vector3[];normals:Vector3[];colors:Vector3[]}
export interface CutScatterOptions {floorY:number; distance?:number; duration?:number}
export interface CutResult {
    removed: number[]; pieces: SceneObject[];
    objectCandidates: number; bvhNodes: number; triangleTests: number;
}
export interface AsyncCutOptions {motion:Vector3;separation?:number;scatter?:CutScatterOptions;execution?:'auto'|'cpu'|'gpu'|'gpu-resident'}
export interface AsyncCutResult extends CutResult {execution:'cpu'|'webgpu-compute'|'webgpu-resident';gpuMeshes:number;fallbackReason:string|null;totalMs:number;computeMs:number;cpuMs:number;metadataReadbackBytes?:number;geometryReadbackBytes?:number}
export interface CharacterOptions {
    speed?: number;
    runSpeed?: number;
    /** Rotation offset in radians for a model whose forward axis is not +Z. */
    modelForwardYaw?: number;
    animations?: { idle?: string; walk?: string; run?: string };
    /** Opt-in upright box motor, gravity and Space jump. Omit for unconstrained movement. */
    physics?: { halfExtents?: Vector3; gravity?: number; jumpSpeed?: number };
}
export interface CharacterState {
    id: number;
    position: Vector3;
    moving: boolean;
    running: boolean;
    grounded: boolean;
    verticalVelocity: number;
    collisionCandidates: number;
}
export interface RotationOptions { delay?: number; duration?: number }
export interface LightOptions { position?: Vector3; color?: Vector3 }
export interface OrbitOptions {
    radius: number; height: number; speed: number;
    phase?: number; pulse?: number; color?: Vector3;
}
/** Handle to Rust state. Throws when Rust rejects an invalid command. */
export class SceneObject {
    protected constructor();
    readonly id: number;
    readonly exists: boolean;
    setCuttable(enabled?: boolean, options?: {capColor?: Vector3}): this;
    setCutter(blade: {base: Vector3; tip: Vector3} | null): this;
    setBladePhysics(options: {edge: Vector3; halfWidth?: number; halfThickness?: number}|null): this;
    readonly meshInfo: TriangleMeshInfo;
    readonly meshSnapshot: TriangleMeshSnapshot;
    gpuMeshSnapshot(): Promise<GpuMeshSnapshot>;
    setPosition(position: Vector3): this;
    /** Absolute rotation in radians. */
    setRotation(radians: Vector3): this;
    setScale(scale: Vector3): this;
    /** Local-space solid box. Defaults match a unit cube, then follow its root transform. */
    setCollider(options?: {halfExtents?: Vector3; offset?: Vector3}): this;
    removeCollider(): this;
    /** Delta rotation in degrees; timing in seconds. Calculated by Rust. */
    rotate(degrees: Vector3, options?: RotationOptions): this;
    setRenderOptions(options?: {isStatic?: boolean; castsShadow?: boolean; receivesShadow?: boolean; reflectivity?: number}): this;
    spin(radiansPerSecond: Vector3): this;
    /** Rust-owned ballistic motion with ground bounce. */
    projectile(velocity: Vector3, options?: {gravity?: number; bounce?: number}): this;
    removeAfter(seconds: number): this;
    remove(): void;
}
export interface ModelInfo {
    animations: {name: string; duration: number}[];
    nodes: {name: string; mesh: number | null; camera: number | null; morphTargets: number}[];
    scenes: {name: string}[];
    scene: number;
    playback: {clip: number; time: number; speed: number; looping: boolean; playing: boolean} | null;
    primitives: number; materials: number; textures: number; skins: number;
    lastError: string | null;
}
export class ModelObject extends SceneObject {
    private constructor();
    readonly info: ModelInfo;
    playAnimation(clip?: number | string, options?: {loop?: boolean; speed?: number}): this;
    pauseAnimation(paused?: boolean): this;
    seekAnimation(seconds: number): this;
    stopAnimation(): this;
    selectScene(index: number): this;
    setMorphWeights(node: number, weights: readonly number[]): this;
    /** Explicitly activates a glTF camera node; importing never changes the view. */
    useCamera(node: number): this;
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
    createMesh(options: TriangleMeshOptions): SceneObject;
    /** Actual world-space text mesh; width/height in metres, facing local +Z. */
    createInfoPanel(kind: 'scene'|'round'|'restart', options?: {position?: Vector3; width?: number; height?: number}): SceneObject;
    cutWithAsync(cutter:SceneObject,options:AsyncCutOptions):Promise<AsyncCutResult>;
    aimCutter(cutter:SceneObject,options:CutterAimOptions):Vector3;
    cutWith(cutter: SceneObject, options: {motion: Vector3; separation?: number; scatter?:CutScatterOptions}): CutResult;
    /** glTF2/GLB asset; decoding, PBR, hierarchy and animation are owned by Rust. */
    loadModel(url: string | URL): Promise<ModelObject>;
    createLight(options?: LightOptions): SceneLight;
    clear(): void;
    setMirror(object: SceneObject): void;
}
export type RawEngine = Pick<typeof wasm,
    "add_cube" | "add_plane" | "add_sphere" | "remove_object" | "clear_scene" |
    "set_object_transform" | "set_cube_transform" | "add_light" | "remove_light" |
    "clear_lights" | "set_light" | "set_light_orbit" | "clear_light_orbit" |
    "schedule_rotate" | "schedule_remove_object" | "schedule_remove_light" |
    "set_projectile" |
    "set_camera_mode" | "set_grid_visible" | "set_light_helpers_visible" | "resize" | "renderer_backend" | "set_frustum_culling" | "set_render_effects" | "set_simulation_paused"> & {screenToPlane(point:readonly [number,number],height?:number):Vector3;worldToScreen(point:Vector3):readonly[number,number]|null;setOrbitView(position:Vector3,target:Vector3):void;adjustOrbit(dx:number,dy:number,zoom?:number):void;cameraViewState():CameraViewState};
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
    shadowLightCount:number; staticUpdatesByLight:readonly number[]; dynamicUpdatesByLight:readonly number[];
    staticUpdates:number; dynamicUpdates:number; reflectionUpdates:number;
    staticVisible:number; dynamicVisible:number; reflectionVisible:number;
    passes:number; shadowResolution:number; reflectionWidth:number; reflectionHeight:number;
}
export interface DebugStats {
    enabled: boolean; fps: number; frameMs: number; cpuMs: number;
    /** Main-view extraction only, not GPU execution. Cached frames do not run culling. */
    cullMs: number; cullCached: boolean;
    total: number; visible: number; culled: number;
    /** Actual commands this frame, including shadows/reflection/postprocessing/HUD/presentation. */
    drawCalls: number;
    /** Main-view geometry, transparency and grid/light helpers; excludes HUD/postprocessing. */
    mainDrawCalls: number;
    bvhNodes: number; objectTests: number; staticRebuilds: number; dynamicRebuilds: number;
    /** Cumulative counters, unchanged while the HUD is disabled. */
    hudFrames: number; hudRefreshes: number;
    xrCuts: number; xrRoundRemaining: number|null;
}
export type PostEffectName = "ssr" | "godRays";
export interface PostProcessing {
    /** Strength 0..1. Unknown effects and invalid values throw. Computed in Rust/WGSL. */
    enable(name: PostEffectName, options?: { strength?: number }): void;
    /** Volume density 0..1 and exposure 0..100; independent of surface lighting. */
    configureGodRays(options?: { density?: number; exposure?: number }): void;
    disable(name: PostEffectName): void;
    /** Grayscale mask on world XZ: 0 dry, 255 reflective. Uploaded once. */
    setReflectionMask(options: { width: number; height: number; pixels: Uint8Array; center?: readonly [number, number]; size?: readonly [number, number] }): void;
    stats(): { passes: number; width: number; height: number; targets: number; frames: number };
}
export interface XRCutReport extends CutResult { handedness: 'left'|'right' }
export interface XRRoundState { duration: number; remaining: number; finished: boolean; cuts: number }
export interface WebXRHost extends EventTarget {
    /** Select a scene button with target ray + trigger or a bound blade strike. Emits buttonhover, select.detail.target and buttonpress. */
    bindButton(object: SceneObject|null): void;
    /** Optional Rust-owned round clock and score; emits roundend at zero. */
    startRound(seconds?: number): void;
    readonly roundState: XRRoundState|null;
    /** Grip pose is applied in Rust. requireTrigger defaults to true; false enables natural swings. Null unbinds. */
    bindController(hand: 'left'|'right', object: SceneObject|null, options?: {cutting?: boolean; requireTrigger?: boolean}): void;
    readonly isPresenting: boolean;
    readonly lastError?: unknown;
    isSupported(): Promise<boolean>;
    /** Call directly from a click. Requires WebGL backend and secure context. */
    enter(): Promise<void>;
    exit(): Promise<void>;
    dispose(): void;
}
export type Engine = Readonly<RawEngine & { readonly xr: WebXRHost; readonly scene: Scene; projectileContacts(): number; readonly postprocessing: PostProcessing; /** Idempotently cancels loads and releases RAF, input and GPU resources. Old handles throw afterwards. */ dispose(): void; setCameraView(position: Vector3, target: Vector3): void; /** WASD + Shift and a chase camera. Mouse works with pointer lock requested by the application, otherwise left-drag. Motion and clip selection run in Rust. Switching camera mode releases control. */ controlCharacter(model: ModelObject, options?: CharacterOptions): void; characterState(): CharacterState | null; /** Optional Rust/WGSL output overlay; initially disabled. */ setDebugOverlay(enabled: boolean): void; debugStats(): DebugStats; set_output_mode(mode: "canvas" | "texture"): boolean; effects_stats(): EffectStats; renderer_stats(): RenderStats }>;
/** One canvas with id gpu-canvas per page. Await before issuing commands. */
export function initEngine(canvas: HTMLCanvasElement, options?: { backend?: "auto" | "webgl" }): Promise<Engine>;
