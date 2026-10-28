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
/** Shared topology; variants contain flat RGB per triangle in identical order.
 * Bright neon seam colors (max RGB >= .25) must match in every variant. */
export interface MeshVariantsOptions {
    positions: readonly number[] | Float32Array;
    indices: readonly number[] | Uint32Array;
    variants: readonly (readonly number[] | Float32Array)[];
    smooth?: boolean;
}
export class MeshVariants {
    private constructor();
    readonly id: number;
    createInstance(variant?: number): SceneObject;
    /** Releases the handle; live instances keep their resources until removed. */
    remove(): void;
}
/** Portable baked glyph schema generated from authorized TTF/OTF sources. */
export interface MeshFontData {
    font: string;
    depth: number;
    license?: string;
    author?: string;
    source?: string;
    glyphs: Record<string,{advance:number;positions:number[];indices:number[];front:number[]}>;
}
export interface Text3DLayout {
    size?: number;
    maxWidth?: number|null;
    align?: 'left'|'center'|'right';
    letterSpacing?: number;
    color?: Vector3;
    sideIntensity?: number;
}
export interface Text3DOptions extends Text3DLayout {position?: Vector3}
export interface Font3DInfo {
    name:string;characters:string;depth:number;license:string;registered:boolean;
    cachedGlyphs:number;liveTexts:number;liveGlyphs:number;
}
export interface Text3DInfo {
    text:string;fontId:number;width:number;height:number;depth:number;glyphCount:number;
    size:number;align:'left'|'center'|'right';letterSpacing:number;color:Vector3;sideIntensity:number;
}
export class Font3D {
    private constructor();
    readonly id:number;
    readonly info:Font3DInfo;
    createText(value:string,options?:Text3DOptions):Text3D;
    /** Existing texts retain their resources until removed. */
    remove():void;
}
export class Text3D extends SceneObject {
    private constructor();
    readonly info:Text3DInfo;
    setText(value:string,options?:Text3DLayout):this;
    setOptions(options:Text3DLayout):this;
}
export interface CutterAimOptions {center:Vector3;yaw?:number;tilt?:number;height?:number;floorY?:number;guides?:readonly SceneObject[]}
export interface CameraViewState {position:Vector3;orbitTarget:Vector3|null}
export interface TriangleMeshInfo {
    scale: Vector3; emission: number;
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
    bindComboIndicator(kind: string, slot: 0|1|2): this;
    setComboTarget(kind: string, points: number, options?: {wild?: boolean}): this;
    setCuttable(enabled?: boolean, options?: {capColor?: Vector3}): this;
    setCutter(blade: {base: Vector3; tip: Vector3} | null): this;
    setBladeTrail(enabled: boolean): this;
    /** Tracked finite ribbon coverage (0..1); zero restores physical cutting. */
    setCutAssist(coverage: number): this;
    /** Opt-in rigid fruit contacts during assisted cuts; requires setBladePhysics. */
    setBladeContactResponse(enabled: boolean): this;
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
    setColliders(boxes: readonly {halfExtents: Vector3; offset?: Vector3}[]): this;
    removeCollider(): this;
    /** Delta rotation in degrees; timing in seconds. Calculated by Rust. */
    rotate(degrees: Vector3, options?: RotationOptions): this;
    /** Vertex-color emission, 0..8; primitive and triangle meshes. */
    setEmission(intensity: number): this;
    /** Live single-value info panel only. Rust keeps its native data source, pose and ID. */
    setFont3D(font:Font3D|null,options?:Text3DLayout):this;
    readonly font3DInfo:Text3DInfo;
    /** Discard fragments below maximum vertex RGB; finite 0..1, zero disables. Triangle meshes show both sides. */
    setColorCutout(threshold: number): this;
    /** Alpha 0..1 for vertex-color meshes, inherited by cut pieces. Facets with max RGB >= opaqueAbove stay opaque (default 8). variation 0..1 gives stable face-based alpha from opacity towards 1 (default 0). Optional coverage 0..1 switches to a sparse/dense near-clear vs solid facet mask; omit for linear variation. glTF keeps its own materials. */
    setOpacity(opacity: number, options?: {opaqueAbove?: number; variation?: number; coverage?: number}): this;
    setRenderOptions(options?: {isStatic?: boolean; castsShadow?: boolean; receivesShadow?: boolean; reflectivity?: number; opaqueReorderable?: boolean}): this;
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
    /** New default pose/root sharing immutable asset/GPU resources; independent animation/physics. */
    createInstance(): ModelObject;
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
export interface TableTennisState {score:[number,number];server:number;winner:number|null;phase:'ready'|'rally'|'point'|'finished';event:string;rallyHits:number;practice:boolean;ballSpin:Vector3;ballRadius:number;paddleFacePosition:Vector3;paddleNormal:Vector3;paddleVelocity:Vector3;ballPosition:Vector3;ballVelocity:Vector3;}
export interface MeleeOptions {
    enemies: ModelObject[]; swords: [SceneObject,SceneObject]; spawns: Vector3[];
    bounds: [number,number,number,number]; obstacles?: [number,number,number,number][];
}
export interface MeleeState {active:number; id:number; phase:string; kills:number; position:Vector3; hp:number}
/** Commands to the Rust SceneManager. Contains no JS scene or animation state. */
export class Scene {
    configureRim(options:RimOptions|null):void;
    rimInput(input:RimInput):void;
    readonly rimState:RimState|null;
    rimTakeCutJob():Uint8Array;
    rimAcceptCut(bytes:Uint8Array):void;
    rimCutFailed():void;

    configureMelee(options:MeleeOptions|null):void;
    configureTableTennis(options:{ball:SceneObject;player:SceneObject;opponent:SceneObject;scorePanel:SceneObject}|null):void;
    tableTennisCommand(command:'move'|'serve'|'reset'|'practice'|'match',x?:number,y?:number):void;
    readonly tableTennisState:TableTennisState|null;
    readonly meleeState:MeleeState|null;
    private constructor();
    readonly objectCount: number;
    readonly lightCount: number;
    getObject(id: number): SceneObject;
    createCube(position?: Vector3): SceneObject;
    createPlane(position?: Vector3): SceneObject;
    createSphere(position?: Vector3): SceneObject;
    createMesh(options: TriangleMeshOptions): SceneObject;
    createMeshVariants(options: MeshVariantsOptions): MeshVariants;
    registerFont3D(data:MeshFontData):Font3D;
    loadFont3D(url:string|URL):Promise<Font3D>;
    builtinFont3D(name?:'pixel'):Font3D;
    /** Actual world-space text mesh; width/height in metres, facing local +Z. */
    createInfoPanel(kind: 'scene'|'round'|'restart'|'diagnostics'|'countdown'|'score'|'combo-award'|'debug-text', options?: {position?: Vector3; width?: number; height?: number}): SceneObject;
    /** Static inscription of 1..32 ASCII digits using shared 3D tube meshes. */
    createNumericText(value: string, options?: {position?: Vector3; width?: number; height?: number}): SceneObject;
    createTextPanel(options: {title: string; lines: string[]; position?: Vector3; width?: number; height?: number}): SceneObject;
    playCutFeedback(pieces: SceneObject[], options?: {mirrorText?: boolean; /** Native hit reward or current chain bonus sum, sampled at the hit. */ scoreLabel?:'pending-bonus'|'hit-points'|'stock-total'; /** Gentle hit size multiplier, 0.01..2; fast tracked attacks retain full size. */ size?: number}): {objects: SceneObject[]; duration: number};
    clearCutFeedback(): void;
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
    /** Legacy object-batch estimate after object culling; excludes helpers and does not account for glTF primitive culling/instancing. Use debugStats().mainDrawCalls for actual main-view commands. */
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
    /** Cumulative glTF work and currently live shared asset resources. */
    gltfDraws: {enabled:boolean;limit:number;draws:{object:number;node:number;primitive:number;indices:number;instances:number;objects:number[];viewSlot:number;shadow:boolean;reflected:boolean;blend:boolean;materialMask:boolean}[]};
    gltf: {indices:{enabled:boolean;uint16Primitives:number;uint32Primitives:number;bufferBytes:number;payloadBytes:number;paddingBytes:number;uploadBytes:number;allocatedBytes:number;memoryScope:string};vertexColors:{enabled:boolean;colorlessShaders:number;coloredShaders:number;colorlessPipelines:number;rigidVertexBufferBytes:number;rigidPrimitives:number;colorlessPrimitives:number;rigidVertexUploadBytes:number;omittedColorBytes:number;memoryScope:string};instancePool:{byteLimit:number;resourceLimit:number;streamByteLimit:number;ownedBytes:number;activeBytes:number;inactiveBytes:number;liveBytes:number;retiredBytes:number;peakLiveBytes:number;activePages:number;inactivePages:number;activeStreams:number;inactiveStreams:number;buffersCreated:number;buffersReleased:number;bindGroupsCreated:number;bindGroupsReleased:number;createdBytes:number;releasedBytes:number;liveBuffers:number;liveBindGroups:number;reuses:number;evictions:number;overBudgetEvents:number;createReasons:number[];createReasonBytes:number[];reasonNames:string[];memoryScope:string;releaseScope:string};fineGroupsVisited:number;fineGroupMembershipHits:number;fineGroupMembershipMisses:number;finePreparedRowsReused:number;finePreparedRowsCreated:number;finePayloadGroupsReused:number;finePayloadGroupsRebuilt:number;finePayloadRecordsWritten:number;finePayloadBytesWritten:number;fineBasisComparisons:number;fineWorldDataUpdates:number;fineFullRecordPacks:number;fineCompatibilityHits:number;fineCacheBudgetFallbacks:number;fineDynamicFallbacks:number;fineNoInstanceFallbacks:number;fineCacheBytes:number; fineLayoutActiveBytes: number; fineLayoutReservedBytes: number;preparedPlanCache:{enabled:boolean;hits:number;misses:number;reasons:number[];reasonNames:string[];retainedBytes:number;byteLimit:number;slotLimit:number;retainedSlots:number;inputEpoch:number;contentEpoch:number;memoryScope:string};prepareEligibleRebuilds:number;prepareGroupRebuilds:number;prepareOpaqueGroupRebuilds:number;preparePayloadRebuilds:number;prepareRecordRebuilds:number;prepareTimedCalls:number;prepareStageMs:null|{eligible:number;groups:number;basis:number;payload:number;records:number;resources:number};primitiveTests:number;primitiveCulled:number;rootBoundsTests:number;rootFullyInside:number;activeDrawIndexBytes:number;instancedDrawCalls:number;translatedDrawCalls:number;translatedInstances:number;instancesDrawn:number;trianglesDrawn:number;overlapTests:number;overlapNodeTests:number;batchBreaks:{rootBlend:number;rootUnsupported:number;rootBounds:number;rootAsset:number;rootOverlap:number;rootSpatial:number;drawCap:number;drawOrdinal:number;drawAsset:number;drawDeformed:number;drawMirror:number;drawMaterial:number;drawPrimitive:number;drawOptions:number};instancePagesCreated:number;instanceBuffersCreated:number;instanceBindGroupsCreated:number;instancePagesActive:number;instancePagesRetained:number;instanceUploadBytes:number;instanceBufferBytes:number;queueScratchGrowths:number;queueScratchBytes:number;cpuDeformedVertices:number;rigidUploadBytes:number;skinUploadBytes:number;uniformUploadBytes:number;bufferUploadCalls:number;skinPaletteUploadBytes:number;geometryCacheHits:number;materialCacheHits:number;geometryResources:number;materialResources:number;cachedAssets:number;drawCalls:number;pipelineChanges:number;bindGroupChanges:number};
    font3d: {
        runtime:{fonts:number;texts:number;cachedGlyphs:number;liveGlyphs:number;layoutRebuilds:number;syncUpdates:number};
        geometryBytes:number;instanceGeometryBytes:number;
    };
    /** Pooled imported-mesh instance buffers across render passes. */
    instanceUploadPages: number;
    numericFont: {glyphMeshes:number;liveGlyphs:number;panels:number;geometryBytes:number;instanceGeometryBytes:number};
    sharedMeshes: {assets:number;liveInstances:number;gpuMeshes:number;colorTableBytes:number;geometryBytes:number;instanceGeometryBytes:number};
    /** Alpha index subsets; index counts cover the same frame span as drawCalls. */
    translucency: {cachedMeshes:number;indexBufferBytes:number;submittedIndices:number;fullIndices:number;draws:number};
    neonGlow: {enabled:boolean;draws:number;segments:number;cachedMeshes:number;bufferBytes:number};
    enabled: boolean; fps: number; frameMs: number; frameP95Ms: number; cpuMs: number;
    /** Main-view extraction only, not GPU execution. Cached frames do not run culling. */
    cullMs: number; cullCached: boolean;
    total: number; visible: number; culled: number;
    /** Actual commands this frame, including shadows/reflection/postprocessing/HUD/presentation. */
    drawCalls: number;
    /** Main-view geometry, transparency and grid/light helpers; excludes HUD/postprocessing. */
    mainDrawCalls: number;
    mainViewCache: {slot:number;hits:number;misses:number;instanceUploadBytes:number;slots:number};
    bvhNodes: number; objectTests: number; staticRebuilds: number; dynamicRebuilds: number;
    /** Cumulative counters, unchanged while the HUD is disabled. */
    hudFrames: number; hudRefreshes: number;
    xrCuts: number; xrRoundRemaining: number|null;
}
export type PostEffectName = "ssr" | "godRays" | "bloom" | "fxaa";
export interface FrameProfile {
    enabled: boolean;
    frame: number|null;
    cpu: Record<string,{lastMs:number;meanMs:number;p95Ms:number;p99Ms:number;maxMs:number;samples:number}>;
    gpu: {supported:boolean;available:boolean;sampleEveryFrames?:number;skippedSamples?:number;sampleFrame:number|null;ageFrames:number|null;passesMs:Record<string,number>|null;error?:string|null};
}
export interface FrameGraphStats {
    active:boolean; passes:string[]; transientTextures:number; transientBuffers:number; transientBytesEstimate?:number;
    prunedPassCount?:number; slots?:string[]; packedDepth?:boolean; sceneColorAttachments?:number;
    resources?:Array<{name:string;lifetime:string;slot:number|null;firstUse:number|null;lastUse:number|null}>;
}
export interface PostProcessing {
    /** Six square sRGB RGBA8 faces packed +X,-X,+Y,-Y,+Z,-Z, size 1..2048. RGB emission adds before LDR bloom; intensity 0..8 (default 1.2). Pass null to release uploaded maps and disable their background/reflection fallback. */
    setEnvironmentCubeMap(options: { size: number; color: Uint8Array; emission: Uint8Array; intensity?: number; projection?: { center: Vector3; halfExtents: Vector3 } | null } | null): void;
    setEnvironmentIntensity(intensity: number): void;
    configureEnvironment(options?: {enabled?: boolean; center?: Vector3; halfExtents?: Vector3}): void;
    environmentCaptures(): number;
    /** Compiled native pass/resource plan. Reading never rebuilds it. */
    graph(): FrameGraphStats;
    /** Strength 0..1. Unknown effects and invalid values throw. Computed in Rust/WGSL. */
    enable(name: PostEffectName, options?: { strength?: number }): void;
    /** Volume density 0..1 and exposure 0..100; independent of surface lighting. */
    configureGodRays(options?: { density?: number; exposure?: number }): void;
    /** Trace reflections at half width/height; full scene and depth remain unchanged. 1 selects the reference path. */
    configureSSR(options?: { resolutionScale?: .5 | 1; receiverBounds?: {min:Vector3;max:Vector3}|null; }): void;
    disable(name: PostEffectName): void;
    /** Grayscale mask on world XZ: 0 dry, 255 reflective. Uploaded once. */
    setReflectionMask(options: { width: number; height: number; pixels: Uint8Array; center?: readonly [number, number]; size?: readonly [number, number] }): void;
    stats(): { passes: number; width: number; height: number; targets: number; frames: number; ssrWidth:number; ssrHeight:number; ssrPixels:number; ssrTracePixels:number; ssrViewportPixels:number };
}
export interface XRCutReport extends CutResult { handedness: 'left'|'right'; bladeContacts?: number; /** Actual native score increase per original in this cut. */ scoreAwards?: {original:number;points:number;totalPoints:number}[] }
export interface FruitChainState {kind:string;count:number;points:number;potential:number}
export interface XRRoundState { mode?:'legacy'|'chains'; chains?:(FruitChainState|null)[]; rejected?:number; pendingBonus?:number; duration: number; remaining: number; finished: boolean; cuts: number; score: number; streak: number; fruit: string; triples: number }
export interface XRHandOptions {cutting?: boolean; requireTrigger?: boolean; haptics?: boolean}
export interface XRTrackedPose {
    readonly tracked: boolean;
    readonly position: Vector3|null;
    /** Quaternion [x,y,z,w], in the XR reference space. */
    readonly orientation: readonly [number,number,number,number]|null;
    readonly matrix: readonly number[]|null;
    /** Filtered grip translation in m/s. Head currently reports zero. */
    readonly speed: number;
    readonly object: SceneObject|null;
}
export interface XRLocomotionOptions {
    /** Walkable head-centre limits [minX,maxX,minZ,maxZ]. Legacy mode uses floor y=0. */
    bounds: readonly [number,number,number,number];
    /** Solid footprints; Rust adds 0.4m player clearance. */
    obstacles?: readonly (readonly [number,number,number,number])[];
    /** Use the shared static scene collider BVH for elevated surfaces and gravity. */
    sceneCollisions?: boolean;
    /** Arc launch speed in m/s, 1..30 (default 9). */
    teleportSpeed?: number;
    speed?: number; teleportRange?: number; slowScale?: number;
    marker: SceneObject;
}
export interface XRLocomotionState {enabled:boolean;origin:readonly number[];aiming:boolean;target:Vector3|null;timeScale:number}
export interface XRPlayerState {head:XRTrackedPose;leftHand:XRTrackedPose;rightHand:XRTrackedPose;inventory:SceneObject[];locomotion:XRLocomotionState}

/** Engine-owned person with generic equipment. No avatar geometry or weapons are spawned. */
export class XRPlayer {
    protected constructor();
    readonly state: XRPlayerState;
    readonly head: XRTrackedPose;
    readonly leftHand: XRTrackedPose;
    readonly rightHand: XRTrackedPose;
    readonly inventory: SceneObject[];
    configureLocomotion(options: XRLocomotionOptions|null): this;
    bindHead(object: SceneObject|null, options?: {offset?:[number,number,number],anchor?:'head'|'feet'}): this;
    addItem(object: SceneObject): this;
    /** Unregister/unequip without deleting the scene object. */
    removeItem(object: SceneObject): this;
    /** Equip any scene object; a cutting role is explicit and optional. Null empties the hand. */
    equip(hand:'left'|'right',object:SceneObject|null,options?:XRHandOptions): this;
    unequip(hand:'left'|'right'): this;
}
export interface ComboDisplayOptions {
    mode?: 'legacy'|'chains';
    symbols: readonly {kind:string;asset:number;variant:number}[];
    frameAsset: number;
    origin: Vector3; spacing: number; rowGap: number; fruitScale: number;
    frameScale?: Vector3;
    historySeconds?: number;
    stockScorePanels?: readonly SceneObject[];
    stackScorePanel?: SceneObject; timerPanel: SceneObject; scorePanel: SceneObject; awardPanels: readonly SceneObject[];
}
export interface ComboDisplayRow {
    kind: string; count: number; icons: number[]; frames: number[];
}
export interface ComboDisplayState {
    mode?:'legacy'|'chains';
    pendingBonus?:number; stackScorePanel?:number|null; stockScorePanels?:number[]|null;
    retiring?:{age:number;earnedBonus:number;earnedPoints:number;scoreLabel:number|null;objects:number[]}|null;
    chains?:(FruitChainState & {earnedPoints:number;icons:number[];frames:number[];label:number})[];
    current: ComboDisplayRow|null;
    history: (ComboDisplayRow & {success:boolean;points:number;age:number;opacity:number;label:number})[];
}
/** Emits frame after the native stereo frame completes, including when document RAF is suspended. */
export interface WebXRHost extends EventTarget {
    readonly player: XRPlayer;
    /** Select a scene button with target ray + trigger or a bound blade strike. Emits buttonhover, select.detail.target and buttonpress. */
    bindButton(object: SceneObject|null): void;
    /** Optional Rust-owned round clock and score; emits roundend at zero. */
    startRound(seconds?: number): void;
    readonly roundState: XRRoundState|null;
    configureComboDisplay(options: ComboDisplayOptions): void;
    bankChain(hand: 'left'|'right'): boolean;
    clearComboDisplay(): void;
    readonly comboDisplayState: ComboDisplayState|null;
    /** Grip pose is applied in Rust. requireTrigger defaults to true; false enables natural swings. Null unbinds. */
    bindController(hand: 'left'|'right', object: SceneObject|null, options?: XRHandOptions): void;
    readonly isPresenting: boolean;
    readonly lastError?: unknown;
    isSupported(): Promise<boolean>;
    /** Call directly from a click. Requires WebGL backend and secure context. */
    enter(options?: {resolutionScale?:number}): Promise<void>;
    exit(): Promise<void>;
    dispose(): void;
}
export type Engine = Readonly<RawEngine & {
    setBackgroundColor(color: Vector3): void;
    /** Opt-in hemisphere/sun lighting, ACES exposure, fog and directional shadow volume. Linear RGB. Null restores legacy appearance. */
    setSceneAppearance(options: {sky: Vector3; ground: Vector3; sunDirection: Vector3; exposure: number; fogColor: Vector3; fogNear: number; fogFar: number; shadowExtent: number; shadowTarget: Vector3; shadowNear: number; shadowFar: number; shadowNormalBias?: number; specularAA: boolean} | null): void;
    /** Additive glow radius in world meters; strength 0..2, radius .001.. .05. */
    setNeonGlow(options?: {strength?:number;radius?:number}): void; readonly xr: WebXRHost; readonly scene: Scene; projectileContacts(): number; readonly postprocessing: PostProcessing; /** Idempotently cancels loads and releases RAF, input and GPU resources. Old handles throw afterwards. */ dispose(): void; setCameraView(position: Vector3, target: Vector3): void; /** WASD + Shift and a chase camera. Mouse works with pointer lock requested by the application, otherwise left-drag. Motion and clip selection run in Rust. Switching camera mode releases control. */ controlCharacter(model: ModelObject, options?: CharacterOptions): void; characterState(): CharacterState | null; /** Optional Rust/WGSL output overlay; initially disabled. */ setDebugOverlay(enabled: boolean): void; /** Diagnostic only:0 production,1 cheap fragment (quality reduced). */ setRendererExperiment(mode:0|1):boolean; gltfIndexLayouts():{primitive:number;format:"Uint16"|"Uint32";topology:string;count:number;max:number|null;payloadBytes:number;bufferBytes:number}[]; setGltfCompactIndicesEnabled(enabled:boolean):boolean; gltfVertexLayouts():{primitive:number;stride:number;bufferBytes:number;omitColor:boolean}[]; /** Reference control; rejects layout changes while GPU assets are live. */ setGltfWhiteColorOmissionEnabled(enabled:boolean):boolean; configureGltfInstancePool(options?:{byteLimit?:number;resourceLimit?:number;streamByteLimit?:number}):boolean; setPreparedPlanCacheEnabled(enabled: boolean): void; debugStats(): DebugStats; frameProfile(): FrameProfile; set_output_mode(mode: "canvas" | "texture"): boolean; effects_stats(): EffectStats; renderer_stats(): RenderStats }>;
/** One canvas with id gpu-canvas per page. Await before issuing commands. */
export function initEngine(canvas: HTMLCanvasElement, options?: { backend?: "auto" | "webgl"; profiling?: boolean }): Promise<Engine>;

/** Aurelia scene-authored Rust simulation. JSON collision data is exported offline. */
export interface RimOptions {
 enemies:{model:ModelObject;kind:string;height:number;offset:Vector3;scale:Vector3}[];
 swords:[SceneObject,SceneObject];bolts:SceneObject[];beams:SceneObject[];charges:SceneObject[];healthBars?:[SceneObject,SceneObject][];/** Optional Rust-owned ground shadow blobs, one per enemy when supplied. */blobShadows?:SceneObject[]|null;hud:SceneObject[];marker:SceneObject;
 shapes:{bounds:{minX:number;maxX:number;minY:number;maxY:number;minZ:number;maxZ:number};planes:{x:number;y:number;z:number;d:number}[]}[];
 obstacles:[number,number,number,number][];
}
export interface RimInput{start?:boolean;pause?:boolean;reset?:boolean;movement?:[number,number];sprint?:boolean;aim?:boolean;dash?:boolean;slash?:number;look?:[number,number];teleport?:Vector3;death_mode?:'cut'|'dissolve';stress_count?:number}
export interface RimState{stressModels:number;blade:{contactSamples:number;hits:number;slowSamples:number;discontinuities:number;maxSpeedForFrame:number};active:boolean;paused:boolean;health:number;dead:boolean;wave:number;kills:number;deaths:number;player:{position:{x:number;y:number;z:number};yaw:number;pitch:number;aiming:boolean;target:Vector3|null;jump:{active:boolean;count:number};dash:{active:boolean;count:number};stamina:{value:number;max:number}};frame:{simulation:number;visual:number};enemies:{id:number;slug:string;position:Vector3;height:number;hp:number;maxHp:number;phase:number;shotTimer:number;plasma:number}[];projectiles:{active:number;fired:number;reflected:number;hitEnemies:number;blockedWorld:number;bolts:{position:Vector3;velocity:Vector3;friendly:boolean}[]};cuts:{motion:{id:number;age:number;direction:Vector3;distance:number}[];queued:number;pending:number;pieces:number;errors:number;preparedTriangles:number;stagedVertices:number;staging:number;stagedMeshes:number;commits:number;stageMs:number;stageMaxMs:number};xr:boolean}
