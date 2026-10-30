import { WebXRHost, XRPlayer } from './webxr.js';
export { XRPlayer };
// JavaScript bridge to the Rust engine. GPU state and the frame loop live in Rust.
// The current Rust API supports one canvas with the id "gpu-canvas" per page.
/** @typedef {import('./engine.d.ts').Vector3} Vector3 */
/** @typedef {typeof import('../pkg/webgpu_wasm.js') & {dispose(): void}} Wasm */
/** @typedef {{canvas: HTMLCanvasElement, backend: 'auto'|'webgl', profiling: boolean, promise: Promise<import('./engine.d.ts').Engine> | null}} Initialization */
/** @param {boolean} ok */
function accepted(ok) {
    if (!ok) throw new Error("Rust engine odmítl příkaz: neplatný objekt nebo parametry.");
}

/** @type {WeakMap<object, Wasm>} */
const handleBindings = new WeakMap();

// These handles hold only an ID and a WASM binding, never scene/simulation state.
export class SceneObject {
    #wasm;
    /** @param {Wasm} wasm @param {number} id */
    constructor(wasm, id) { this.#wasm = wasm; handleBindings.set(this, wasm); this.id = id; Object.freeze(this); }
    get exists() { return this.#wasm.object_exists(this.id); }
    /** @param {boolean} [enabled] @param {{capColor?: Vector3}} [options] */
    setCuttable(enabled = true, {capColor = [1, 0.3, 0.12]} = {}) {
        this.#wasm.set_cuttable(this.id, enabled, ...capColor); return this;
    }
    /** Award points after three consecutive original targets of this kind in an XR round.
     * @param {string} kind @param {number} points @param {{wild?:boolean}} [options] */
    setComboTarget(kind, points, {wild=false} = {}) {
        if (!Number.isInteger(points) || points < 1 || points > 1000000) throw new Error('Combo points must be an integer in 1..1000000');
        accepted(this.#wasm.set_combo_target(this.id, kind, points, wild)); return this;
    }
    /** Rust highlights this world object when this combo slot is earned.
     * @param {string} kind @param {0|1|2} slot */
    bindComboIndicator(kind, slot) {
        if (!Number.isInteger(slot) || slot < 0 || slot > 2) throw new Error('Combo slot must be 0, 1 or 2');
        accepted(this.#wasm.bind_combo_indicator(this.id, kind, slot)); return this;
    }
    /** Endpoints in this object's local coordinates, transformed by Rust at sweep time.
     * @param {{base: Vector3, tip: Vector3}|null} blade */
    setCutter(blade) {
        this.#wasm.set_cutter(this.id, blade !== null, new Float32Array(blade ? [...blade.base, ...blade.tip] : [])); return this;
    }
    /** @param {boolean} enabled */
    setBladeTrail(enabled) { this.#wasm.set_blade_trail(this.id, enabled); return this; }
    /** Opt-in rigid fruit contacts during assisted cuts; requires setBladePhysics.
     * @param {boolean} enabled */
    setBladeContactResponse(enabled) { this.#wasm.set_blade_contact_response(this.id, enabled); return this; }
    /** Tracked finite ribbon coverage (0..1); zero restores physical cutting.
     * @param {number} coverage */
    setCutAssist(coverage) { this.#wasm.set_cut_assist(this.id, coverage); return this; }
    /** Physical flat/back contacts plus one sharp edge. Null restores legacy cutter sweeps.
     * @param {{edge: Vector3, halfWidth?: number, halfThickness?: number}|null} options */
    setBladePhysics(options) {
        this.#wasm.set_blade_physics(this.id, new Float32Array(options?.edge ?? []), options?.halfWidth ?? .0225, options?.halfThickness ?? .004);
        return this;
    }
    /** @returns {import('./engine.d.ts').TriangleMeshInfo} */
    get meshInfo() { return JSON.parse(this.#wasm.triangle_mesh_info(this.id)); }
    /** Explicit CPU diagnostic snapshot; not intended for per-frame polling.
     * @returns {import('./engine.d.ts').TriangleMeshSnapshot} */
    get meshSnapshot() { return JSON.parse(this.#wasm.triangle_mesh_snapshot(this.id)); }
    /** Explicit actual GPU vertex readback for diagnostics, not the render loop.
     * @returns {Promise<import('./engine.d.ts').GpuMeshSnapshot>} */
    async gpuMeshSnapshot() { return JSON.parse(await this.#wasm.triangle_mesh_gpu_snapshot(this.id)); }
    /** @param {Vector3} position */
    setPosition(position) { accepted(this.#wasm.set_object_position(this.id, ...position)); return this; }
    /** @param {Vector3} radians */
    setRotation(radians) { accepted(this.#wasm.set_object_rotation(this.id, ...radians)); return this; }
    /** @param {Vector3} scale */
    setScale(scale) { accepted(this.#wasm.set_object_scale(this.id, ...scale)); return this; }
    /** @param {{halfExtents?: Vector3, offset?: Vector3}} [options] */
    setCollider({ halfExtents = [0.5, 0.5, 0.5], offset = [0, 0, 0] } = {}) {
        accepted(this.#wasm.set_box_collider(this.id, ...halfExtents, ...offset)); return this;
    }
    /** Compound local-space collision proxies; no additional rendered objects.
     * @param {readonly {halfExtents: Vector3, offset?: Vector3}[]} boxes */
    setColliders(boxes) {
        accepted(this.#wasm.set_box_colliders(this.id, new Float32Array(boxes.flatMap(
            ({halfExtents, offset = [0,0,0]}) => [...halfExtents, ...offset])))); return this;
    }
    removeCollider() { accepted(this.#wasm.remove_collider(this.id)); return this; }
    /** @param {Vector3} degrees @param {import("./engine.d.ts").RotationOptions} [options] */
    rotate(degrees, { delay = 0, duration = 1 } = {}) {
        accepted(this.#wasm.schedule_rotate(this.id, ...degrees, delay, duration)); return this;
    }
    /** Vertex-color emission, 0..8; primitive and triangle meshes. @param {number} intensity */
    setEmission(intensity) { accepted(this.#wasm.set_object_emission(this.id, intensity)); return this; }
    /** Use a loaded mesh font for a live single-value information panel. Null restores its original font.
     * @param {Font3D|null} font @param {import('./engine.d.ts').Text3DLayout} [options] */
    setFont3D(font,options={}) {
        if(font!==null&&handleBindings.get(font)!==this.#wasm)throw new Error('Font musí patřit stejnému enginu jako panel.');
        this.#wasm.bind_info_panel_font3d(this.id,font?.id??0xffffffff,JSON.stringify(options));return this;
    }
    /** Snapshot of a live info panel's mesh-font text; throws when not font-bound.
     * @returns {import('./engine.d.ts').Text3DInfo} */
    get font3DInfo() {return JSON.parse(this.#wasm.font3d_text_info(this.id));}
    /** Discard fragments below this maximum vertex RGB; finite 0..1, zero disables. Triangle meshes show both sides. @param {number} threshold */
    setColorCutout(threshold) { accepted(this.#wasm.set_object_color_cutout(this.id, threshold)); return this; }
    /** Vertex-color mesh alpha; brighter facets can stay opaque (neon seams).
     * @param {number} opacity @param {{opaqueAbove?:number,variation?:number,coverage?:number}} [options] */
    setOpacity(opacity, {opaqueAbove=8,variation=0,coverage=-1} = {}) { accepted(this.#wasm.set_object_opacity(this.id,opacity,opaqueAbove,variation,coverage)); return this; }
    /** Opaque reordering is opt-in: no inter-root coplanar/order dependency. Skin, morph and blend retain their fallback.
     * @param {{isStatic?: boolean, castsShadow?: boolean, receivesShadow?: boolean, reflectivity?: number, opaqueReorderable?: boolean}} [options] */
    setRenderOptions({isStatic=false, castsShadow=true, receivesShadow=true, reflectivity=0, opaqueReorderable=false} = {}) {
        accepted(this.#wasm.set_object_render_options(this.id,isStatic,castsShadow,receivesShadow,reflectivity,opaqueReorderable)); return this;
    }
    /** @param {Vector3} radiansPerSecond */
    spin(radiansPerSecond) { accepted(this.#wasm.set_object_spin(this.id,...radiansPerSecond)); return this; }
    /** @param {Vector3} velocity @param {{gravity?:number,bounce?:number}} [options] */
    projectile(velocity, {gravity = 9.81, bounce = 0.45} = {}) { accepted(this.#wasm.set_projectile(this.id,...velocity,gravity,bounce)); return this; }
    /** @param {number} seconds */
    removeAfter(seconds) { accepted(this.#wasm.schedule_remove_object(this.id, seconds)); return this; }
    remove() { accepted(this.#wasm.remove_object(this.id)); }
}

/** Immutable shared geometry and face-color tables owned by Rust. */
export class MeshVariants {
    #wasm;
    /** @param {Wasm} wasm @param {number} id */
    constructor(wasm,id) { this.#wasm=wasm; this.id=id; Object.freeze(this); }
    /** Independent scene object sharing this asset's GPU geometry.
     * @param {number} variant */
    createInstance(variant=0) {
        if(!Number.isInteger(variant)||variant<0||variant>0xffffffff)throw new Error('Neplatné číslo varianty.');
        const id=this.#wasm.add_mesh_variant(this.id,variant);
        accepted(id!==0xffffffff);
        return new SceneObject(this.#wasm,id);
    }
    /** Release the asset handle; existing instances remain valid. */
    remove() { accepted(this.#wasm.remove_mesh_variants(this.id)); }
}

/** Immutable font asset handle; parsing, layout and glyph caches live in Rust. */
export class Font3D {
    #wasm;
    /** @param {Wasm} wasm @param {number} id */
    constructor(wasm,id) {this.#wasm=wasm;handleBindings.set(this,wasm);this.id=id;Object.freeze(this);}
    /** @returns {import('./engine.d.ts').Font3DInfo} */
    get info() {return JSON.parse(this.#wasm.font3d_info(this.id));}
    /** @param {string} value @param {import('./engine.d.ts').Text3DOptions} [options] */
    createText(value,{position=[0,0,0],...options}={}) {
        if(!position||position.length!==3||!Array.from(position).every(Number.isFinite))throw new Error('Neplatná pozice textu.');
        const id=this.#wasm.add_font3d_text(this.id,value,JSON.stringify(options));
        accepted(id!==0xffffffff);
        const text=new Text3D(this.#wasm,id);
        try{return text.setPosition(position);}catch(error){
            try{text.remove();}catch{/* Preserve the rejected transform error. */}
            throw error;
        }
    }
    /** Existing texts remain alive; creating new texts through this handle stops. */
    remove() {accepted(this.#wasm.remove_font3d(this.id));}
}

/** One Rust-owned text root; child glyphs inherit its transform and material. */
export class Text3D extends SceneObject {
    #wasm;
    /** @param {Wasm} wasm @param {number} id */
    constructor(wasm,id) {super(wasm,id);this.#wasm=wasm;}
    /** @returns {import('./engine.d.ts').Text3DInfo} */
    get info() {return JSON.parse(this.#wasm.font3d_text_info(this.id));}
    /** Atomic content/layout/style update; unsupported characters throw. @param {string} value
     * @param {import('./engine.d.ts').Text3DLayout} [options] */
    setText(value,options={}) {this.#wasm.update_font3d_text(this.id,value,JSON.stringify(options));return this;}
    /** @param {import('./engine.d.ts').Text3DLayout} options */
    setOptions(options) {return this.setText(this.info.text,options);}
}

export class SceneLight {
    #wasm;
    /** @param {Wasm} wasm @param {number} id */
    constructor(wasm, id) { this.#wasm = wasm; handleBindings.set(this, wasm); this.id = id; Object.freeze(this); }
    get exists() { return this.#wasm.light_exists(this.id); }
    /** @param {Vector3} position @param {Vector3} color */
    set(position, color) { accepted(this.#wasm.set_light(this.id, ...position, ...color)); return this; }
    /** @param {import("./engine.d.ts").OrbitOptions} options */
    orbit({ radius, height, speed, phase = 0, pulse = 0, color = [1, 1, 1] }) {
        accepted(this.#wasm.set_light_orbit(this.id, radius, height, speed, phase, pulse, ...color)); return this;
    }
    stopOrbit() { accepted(this.#wasm.clear_light_orbit(this.id)); return this; }
    /** @param {number} seconds */
    removeAfter(seconds) { accepted(this.#wasm.schedule_remove_light(this.id, seconds)); return this; }
    remove() { accepted(this.#wasm.remove_light(this.id)); }
}

/** glTF root handle. Pose, playback and imported hierarchy remain in Rust. */
export class ModelObject extends SceneObject {
    /** Independent default pose/root sharing immutable Rust asset and GPU resources.
     * Does not copy transforms, animation playback or physics from this object.
     * @returns {ModelObject} */
    createInstance() { return new ModelObject(this.#wasm, this.#wasm.instantiate_model(this.id)); }
    #wasm;
    /** @param {Wasm} wasm @param {number} id */
    constructor(wasm, id) { super(wasm, id); this.#wasm = wasm; }
    /** @returns {import("./engine.d.ts").ModelInfo} */
    get info() { return JSON.parse(this.#wasm.model_info(this.id)); }
    /** @param {number|string} [clip] @param {{loop?: boolean, speed?: number}} [options] */
    playAnimation(clip = 0, { loop = true, speed = 1 } = {}) {
        const index = typeof clip === 'string' ? this.info.animations.findIndex(a => a.name === clip) : clip;
        accepted(this.#wasm.play_model_animation(this.id, index, loop, speed)); return this;
    }
    pauseAnimation(paused = true) { accepted(this.#wasm.pause_model_animation(this.id, paused)); return this; }
    /** @param {number} seconds */
    seekAnimation(seconds) { accepted(this.#wasm.seek_model_animation(this.id, seconds)); return this; }
    stopAnimation() { accepted(this.#wasm.stop_model_animation(this.id)); return this; }
    /** @param {number} index */
    selectScene(index) { accepted(this.#wasm.select_model_scene(this.id, index)); return this; }
    /** @param {number} node @param {readonly number[]} weights */
    setMorphWeights(node, weights) { accepted(this.#wasm.set_model_morph_weights(this.id, node, new Float32Array(weights))); return this; }
    /** @param {number} node */
    useCamera(node) { accepted(this.#wasm.use_model_camera(this.id, node)); return this; }
}

// Thin command facade for the Rust SceneManager, not a second manager in JS.
export class Scene {
    #wasm;
    #loadEpoch = 0;
    /** @type {Map<AbortController, Set<ReadableStreamDefaultReader<Uint8Array>>>} */
    #loads = new Map();
    /** @param {Wasm} wasm */
    constructor(wasm) { this.#wasm = wasm; Object.freeze(this); }
    /** @param {import('./engine.d.ts').RimOptions|null} options */
    configureRim(options){
        if(options===null){this.#wasm.configure_rim('null');return;}
        const {enemies,swords,bolts,beams,charges,healthBars=[],blobShadows=null,hud,marker,shapes,obstacles}=options;
        const shadows=blobShadows??[];
        const handles=[...enemies.map(e=>e.model),...swords,...bolts,...beams,...charges,...healthBars.flat(),...shadows,...hud,marker];
        if(handles.some(o=>handleBindings.get(o)!==this.#wasm))throw new Error('Objekty Říma musí patřit této scéně.');
        this.#wasm.configure_rim(JSON.stringify({enemies:enemies.map(({model,...spec})=>({...spec,id:model.id})),swords:swords.map(o=>o.id),bolts:bolts.map(o=>o.id),beams:beams.map(o=>o.id),charges:charges.map(o=>o.id),healthBars:healthBars.map(pair=>pair.map(o=>o.id)),blobShadows:shadows.map(o=>o.id),hud:hud.map(o=>o.id),marker:marker.id,shapes,obstacles}));
    }
    /** @param {import('./engine.d.ts').RimInput} input */
    rimInput(input){this.#wasm.rim_input(JSON.stringify(input));}
    /** @returns {import('./engine.d.ts').RimState|null} */
    get rimState(){return JSON.parse(this.#wasm.rim_state());}
    rimTakeCutJob(){return this.#wasm.rim_take_cut_job();}
    /** @param {Uint8Array} bytes */
    rimAcceptCut(bytes){this.#wasm.rim_accept_cut(bytes);}
    rimCutFailed(){this.#wasm.rim_cut_failed();}
    /** @param {import('./engine.d.ts').MeleeOptions|null} options */
    configureMelee(options) {
        if(options===null){this.#wasm.configure_melee('null');return;}
        const {enemies,swords,spawns,bounds,obstacles=[]}=options;
        if([...enemies,...swords].some(object=>handleBindings.get(object)!==this.#wasm))throw new Error('Soubojové objekty musí patřit této scéně.');
        this.#wasm.configure_melee(JSON.stringify({enemies:enemies.map(o=>o.id),swords:swords.map(o=>o.id),spawns,bounds,obstacles}));
    }
    /** @returns {import('./engine.d.ts').MeleeState|null} */
    get meleeState(){return JSON.parse(this.#wasm.melee_state());}
    /** @param {{ball:SceneObject,player:SceneObject,opponent:SceneObject,scorePanel:SceneObject}|null} options */
    configureTableTennis(options) {
        if(options===null){this.#wasm.table_tennis_command('stop',0,0);return;}
        const {ball,player,opponent,scorePanel}=options;
        if([ball,player,opponent,scorePanel].some(o=>handleBindings.get(o)!==this.#wasm))throw new Error('Objekty musí patřit této scéně.');
        this.#wasm.configure_table_tennis(ball.id,player.id,opponent.id,scorePanel.id);
    }
    /** @param {'move'|'serve'|'reset'|'practice'|'match'} command @param {number} [x] @param {number} [y] */
    tableTennisCommand(command,x=0,y=0){this.#wasm.table_tennis_command(command,x,y);}
    /** @returns {import('./engine.d.ts').TableTennisState|null} */
    get tableTennisState(){return JSON.parse(this.#wasm.table_tennis_state());}
    get objectCount() { return this.#wasm.scene_object_count(); }
    get lightCount() { return this.#wasm.scene_light_count(); }
    /** Resolve a live Rust-owned object ID returned by diagnostics or bindings.
     * @param {number} id */
    getObject(id) {
        if(!Number.isInteger(id)||id<0||id>=0xffffffff||!this.#wasm.object_exists(id))throw new Error('Objekt ve scéně neexistuje.');
        return new SceneObject(this.#wasm,id);
    }
    /** @param {Vector3} [position] */
    createCube(position = [0, 0, 0]) { return this.#create(this.#wasm.add_cube(...position)); }
    /** @param {Vector3} [position] */
    createPlane(position = [0, 0, 0]) { return this.#create(this.#wasm.add_plane(...position)); }
    /** @param {Vector3} [position] */
    createSphere(position = [0, 0, 0]) { return this.#create(this.#wasm.add_sphere(...position)); }
    /** World-space Rust text geometry, updated from native scene/round data.
     * @param {'scene'|'round'|'restart'|'diagnostics'|'countdown'|'score'|'combo-award'|'debug-text'} kind
     * @param {{position?: Vector3, width?: number, height?: number}} [options] */
    createInfoPanel(kind, {position = [0,0,0], width = 1.2, height = .72} = {}) {
        return this.#create(this.#wasm.add_info_panel(kind, width, height)).setPosition(position);
    }
    /** Static Rust-owned numeric tubes sharing cached digit meshes with the HUD.
     * @param {string} value 1..32 ASCII digits.
     * @param {{position?: Vector3, width?: number, height?: number}} [options] */
    createNumericText(value, {position = [0,0,0], width = 1.2, height = .72} = {}) {
        return this.#create(this.#wasm.add_numeric_text(value, width, height)).setPosition(position);
    }
    /** Static world-space bitmap inscription built by Rust, without a background.
     * @param {{title: string, lines: string[], position?: Vector3, width?: number, height?: number}} options */
    createTextPanel({title, lines, position = [0,0,0], width = 1.2, height = .72}) {
        return this.#create(this.#wasm.add_text_panel(title, JSON.stringify(lines), width, height)).setPosition(position);
    }
    /** Closed indexed triangle solid. Parsing, geometry, BVH and GPU upload are Rust.
     * @param {import('./engine.d.ts').TriangleMeshOptions} options */
    createMesh({positions, indices, color = [0.18, 0.65, 0.85], triangleColors, smooth = false}) {
        if (indices.length > 60000 || positions.length > 180000 || !Array.from(indices).every(i => Number.isInteger(i) && i >= 0 && i <= 0xffffffff)) {
            throw new Error('Neplatné indexy nebo překročený limit meshe.');
        }
        if (triangleColors && triangleColors.length !== indices.length) throw new Error('Každý trojúhelník potřebuje RGB barvu.');
        return this.#create(triangleColors
            ? this.#wasm.add_colored_triangle_mesh(new Float32Array(positions),new Uint32Array(indices),new Float32Array(triangleColors),smooth)
            : this.#wasm.add_triangle_mesh(new Float32Array(positions), new Uint32Array(indices), ...color,smooth));
    }
    /** One GPU geometry, immutable per-face color variants and independent instances.
     * @param {import('./engine.d.ts').MeshVariantsOptions} options */
    createMeshVariants({positions,indices,variants,smooth=false}) {
        if(!variants.length||variants.length>64||indices.length===0||indices.length>60000||positions.length>180000||
            !Array.from(indices).every(i=>Number.isInteger(i)&&i>=0&&i<=0xffffffff)||
            variants.some(colors=>colors.length!==indices.length))throw new Error('Neplatná geometrie nebo tabulka variant.');
        const colors=new Float32Array(variants.length*indices.length);
        variants.forEach((variant,index)=>colors.set(variant,index*indices.length));
        const id=this.#wasm.register_mesh_variants(new Float32Array(positions),new Uint32Array(indices),colors,smooth);
        accepted(id!==0xffffffff);
        return new MeshVariants(this.#wasm,id);
    }
    /** Register baked outline geometry; Rust validates it atomically. No JS glyph cache.
     * @param {import('./engine.d.ts').MeshFontData} data */
    registerFont3D(data) {
        const id=this.#wasm.register_font3d(JSON.stringify(data));accepted(id!==0xffffffff);
        return new Font3D(this.#wasm,id);
    }
    /** Existing engine bitmap alphabet, extruded into a reusable mesh font.
     * @param {'pixel'} [name] */
    builtinFont3D(name='pixel') {
        const id=this.#wasm.builtin_font3d(name);accepted(id!==0xffffffff);return new Font3D(this.#wasm,id);
    }
    /** Stream a bounded mesh-font asset, cancelling on clear/dispose. Rust parses JSON.
     * @param {string|URL} url */
    async loadFont3D(url) {
        this.#wasm.scene_object_count();
        const epoch=this.#loadEpoch,controller=new AbortController();
        /** @type {Set<ReadableStreamDefaultReader<Uint8Array>>} */
        const readers=new Set();
        this.#loads.set(controller,readers);
        const current=()=>{
            if(controller.signal.aborted||epoch!==this.#loadEpoch)throw new Error('Načítání fontu zrušeno vymazáním scény');
        };
        /** @type {ReadableStreamDefaultReader<Uint8Array>|undefined} */
        let reader;
        try {
            const response=await fetch(new URL(url,location.href),{signal:controller.signal});current();
            if(!response.ok)throw new Error(`Font nelze načíst: ${response.status}`);
            const limit=16*1024*1024;
            if(Number(response.headers.get('content-length'))>limit){
                await response.body?.cancel();throw new Error('Font je příliš velký');
            }
            reader=response.body?.getReader();
            if(!reader)throw new Error('Font nemá tělo odpovědi');
            readers.add(reader);
            const decoder=new TextDecoder('utf-8',{fatal:true});let text='',size=0;
            for(;;){
                const {done,value}=await reader.read();current();
                if(done)break;
                size+=value.length;if(size>limit)throw new Error('Font je příliš velký');
                text+=decoder.decode(value,{stream:true});
            }
            text+=decoder.decode();current();
            const id=this.#wasm.register_font3d(text);accepted(id!==0xffffffff);
            return new Font3D(this.#wasm,id);
        }catch(error){
            try{await reader?.cancel();}catch{/* Preserve the import error. */}
            throw error;
        }finally{
            if(reader){readers.delete(reader);reader.releaseLock();}
            this.#loads.delete(controller);
        }
    }
    /** @param {SceneObject} cutter @param {import('./engine.d.ts').CutterAimOptions} options @returns {Vector3} */
    aimCutter(cutter,{center,yaw=0,tilt=0,height=3.6,floorY=0.2,guides=[]}) {
        if(handleBindings.get(cutter)!==this.#wasm || guides.some(g=>handleBindings.get(g)!==this.#wasm))throw new Error('Čepel i vodítka musí patřit této scéně.');
        const p=this.#wasm.aim_cutter(cutter.id,new Uint32Array(guides.map(g=>g.id)),new Float32Array(center),new Float32Array([yaw,tilt,height,floorY]));
        return [p[0],p[1],p[2]];
    }
    /** Straight finite blade sweep. Rust moves the sword and replaces hit solids.
     * @param {SceneObject} cutter @param {{motion: Vector3, separation?: number, scatter?: import('./engine.d.ts').CutScatterOptions}} options
     * @returns {import('./engine.d.ts').CutResult} */
    cutWith(cutter, {motion, separation = 0.12, scatter}) {
        if (handleBindings.get(cutter) !== this.#wasm) throw new Error('Čepel musí patřit této scéně.');
        const result = JSON.parse(scatter
            ? this.#wasm.cut_with_scatter(cutter.id, new Float32Array(motion), separation, scatter.floorY, scatter.distance ?? 0.8, scatter.duration ?? 0.6)
            : this.#wasm.cut_with(cutter.id, ...motion, separation));
        return {...result, pieces: result.pieces.map((/** @type {number} */ id) => this.#create(id))};
    }
    /** Explicit bounded Rust burst and fragment presentation.
     * @param {SceneObject[]} pieces @param {{mirrorText?: boolean,size?: number,scoreLabel?: 'pending-bonus'|'hit-points'|'stock-total'}} [options] @returns {{objects: SceneObject[], duration: number}} */
    playCutFeedback(pieces, {mirrorText = false, size = 1, scoreLabel = undefined} = {}) {
        if (pieces.some(p => handleBindings.get(p) !== this.#wasm)) throw new Error('Fragmenty musí patřit této scéně.');
        if (scoreLabel !== undefined && scoreLabel !== 'pending-bonus' && scoreLabel !== 'hit-points' && scoreLabel !== 'stock-total') throw new Error('Neznamy bodovy popisek.');
        const ids = new Uint32Array(pieces.map(p => p.id));
        const result = JSON.parse(scoreLabel === undefined ? this.#wasm.play_cut_feedback_facing(ids, mirrorText, size) : this.#wasm.play_cut_feedback_facing(ids, mirrorText, size, scoreLabel));
        return {objects: result.objects.map((/** @type {number} */ id) => this.#create(id)), duration: result.duration};
    }
    clearCutFeedback() { this.#wasm.clear_cut_feedback(); }
    /** Hybrid compute preparation + identical Rust topology; WebGL uses CPU.
     * @param {SceneObject} cutter @param {import('./engine.d.ts').AsyncCutOptions} options
     * @returns {Promise<import('./engine.d.ts').AsyncCutResult>} */
    async cutWithAsync(cutter,{motion,separation=.12,scatter,execution='auto'}) {
        if(handleBindings.get(cutter)!==this.#wasm)throw new Error('Čepel musí patřit této scéně.');
        const epoch=this.#loadEpoch;
        const result=JSON.parse(await this.#wasm.cut_with_async(cutter.id,new Float32Array(motion),separation,new Float32Array(scatter?[scatter.floorY,scatter.distance??.8,scatter.duration??.6]:[]),execution));
        this.#wasm.scene_object_count();
        if(epoch!==this.#loadEpoch)throw new Error('Řez zrušen vymazáním scény');
        return {...result,pieces:result.pieces.map((/** @type {number} */ id)=>this.#create(id))};
    }
    /** @param {number} id */
    #create(id) {
        accepted(id !== 0xffffffff);
        return new SceneObject(this.#wasm, id);
    }
    /** Transport only; format parsing, transforms, bounds and GPU upload are Rust. */
    /** @param {string|URL} url */
    async loadModel(url) {
        // Reject a disposed scene before opening any transport.
        this.#wasm.scene_object_count();
        const epoch = this.#loadEpoch;
        const controller = new AbortController();
        /** @type {Set<ReadableStreamDefaultReader<Uint8Array>>} */
        const readers = new Set();
        let source = new URL(url, location.href);
        let remaining = 128 * 1024 * 1024;
        this.#loads.set(controller, readers);
        const checkCurrent = () => {
            if (controller.signal.aborted || epoch !== this.#loadEpoch) throw new Error('Načítání modelu zrušeno vymazáním scény');
        };
        /** @param {URL} resource @param {boolean} [root] */
        const fetchBytes = async (resource, root = false) => {
            checkCurrent();
            const response = await fetch(resource, { signal: controller.signal });
            checkCurrent();
            if (root && response.url) source = new URL(response.url);
            if (!response.ok) throw new Error(`Model nelze načíst: ${response.status} ${resource}`);
            if (Number(response.headers.get('content-length')) > remaining) {
                await response.body?.cancel();
                throw new Error('Model je příliš velký');
            }
            const reader = response.body?.getReader();
            if (!reader) throw new Error('Model nemá tělo odpovědi');
            readers.add(reader);
            const chunks = [];
            let length = 0;
            try {
                for (;;) {
                    const { done, value } = await reader.read();
                    checkCurrent();
                    if (done) break;
                    if (value.length > remaining) throw new Error('Model je příliš velký');
                    remaining -= value.length;
                    length += value.length;
                    chunks.push(value);
                }
            } catch (error) {
                try { await reader.cancel(); } catch { /* Preserve the transport error. */ }
                throw error;
            } finally { readers.delete(reader); reader.releaseLock(); }
            const bytes = new Uint8Array(length);
            let offset = 0;
            for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
            return bytes;
        };
        try {
            const bytes = await fetchBytes(source, true);
            const { buffers: uris, images: imageUris } = JSON.parse(this.#wasm.gltf_resource_uris(bytes));
            if (uris.length + imageUris.length > 1024) throw new Error('Model má příliš mnoho zdrojů');
            const buffers = [];
            // Bound total download memory; do not fetch all external buffers concurrently.
            for (const uri of uris) buffers.push(uri ? await fetchBytes(new URL(uri, source)) : new Uint8Array());
            const images = [];
            for (const uri of imageUris) images.push(uri ? await fetchBytes(new URL(uri, source)) : new Uint8Array());
            checkCurrent();
            const id = this.#wasm.load_gltf_model(bytes, buffers, images);
            accepted(id !== 0xffffffff);
            return new ModelObject(this.#wasm, id);
        } finally {
            this.#loads.delete(controller);
        }
    }
    /** @param {import("./engine.d.ts").LightOptions} [options] */
    createLight({ position = [0, 0, 0], color = [1, 1, 1] } = {}) {
        const id = this.#wasm.add_light(...position, ...color);
        accepted(id >= 0);
        return new SceneLight(this.#wasm, id);
    }
    clear() {
        this.cancelLoads();
        this.#wasm.clear_all();
    }
    /** @internal Cancel transport before releasing the runtime. */
    cancelLoads() {
        this.#loadEpoch++;
        for (const [controller, readers] of this.#loads) {
            controller.abort(new Error('Načítání modelu zrušeno vymazáním scény'));
            for (const reader of readers) {
                // Cancel custom streams too; aborting fetch alone need not unblock them.
                try { Promise.resolve(reader.cancel()).catch(() => {}); } catch { /* Already closed. */ }
            }
        }
        this.#loads.clear();
    }
    /** @param {SceneObject} object */
    setMirror(object) {
        accepted(handleBindings.get(object) === this.#wasm);
        accepted(this.#wasm.configure_planar_mirror(object.id));
    }
}

// Share ownership across cache-busted SDK imports as well as repeated calls.
const initializationKey = Symbol.for('webgpu_wasm.engine.initialization');
const ownership = /** @type {Record<symbol, Initialization | undefined>} */ (/** @type {unknown} */ (globalThis));

/** @param {HTMLCanvasElement} canvas @param {{backend?: "auto"|"webgl", profiling?: boolean}} [options] @returns {Promise<import("./engine.d.ts").Engine>} */
export async function initEngine(canvas, { backend = "auto", profiling = false } = {}) {
    if (!(canvas instanceof HTMLCanvasElement) || canvas.id !== "gpu-canvas") {
        throw new Error('Engine vyžaduje canvas s id="gpu-canvas".');
    }
    if (canvas.ownerDocument !== document || !canvas.isConnected || document.getElementById('gpu-canvas') !== canvas) {
        throw new Error('Engine vyžaduje canvas připojený k aktuálnímu dokumentu.');
    }
    if (backend !== 'auto' && backend !== 'webgl') throw new Error('Neplatný backend enginu.');
    if (typeof profiling !== 'boolean') throw new Error('Neplatné nastavení profiling.');
    const current = ownership[initializationKey];
    if (current) {
        if (current.canvas !== canvas || current.backend !== backend || current.profiling !== profiling) {
            throw new Error('Engine již inicializuje nebo používá jiný canvas či backend nebo profiling.');
        }
        return /** @type {Promise<import("./engine.d.ts").Engine>} */ (current.promise);
    }
    /** @type {Initialization} */
    const initialization = { canvas, backend, profiling, promise: null };
    ownership[initializationKey] = initialization;
    initialization.promise = initializeEngine(canvas, backend, initialization);
    try {
        return await initialization.promise;
    } catch (error) {
        if (ownership[initializationKey] === initialization) delete ownership[initializationKey];
        throw error;
    }
}

/** @param {HTMLCanvasElement} canvas @param {"auto"|"webgl"} backend @param {Initialization} initialization */
async function initializeEngine(canvas, backend, initialization) {
    canvas.dataset.backend = backend;
    canvas.dataset.frameProfile = String(initialization.profiling);
    // wgpu must request an XR-compatible adapter before the GPUDevice exists.
    canvas.dataset.xrWebgpu = String(backend !== 'webgl' && typeof Reflect.get(globalThis, 'XRGPUBinding') === 'function');
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;

    const version = Date.now();
    const bindings = /** @type {Wasm} */ (await import(`../pkg/webgpu_wasm.js?v=${version}`));
    await bindings.default({ module_or_path: new Request(new URL(`../pkg/webgpu_wasm_bg.wasm?v=${version}`, import.meta.url), {cache:"no-store"}) });
    await bindings.start();
    let disposed = false;
    // All commands, including raw exports and old handles, share a runtime lifetime.
    const wasm = new Proxy(bindings, { get(target, property, receiver) {
        const value = Reflect.get(target, property, receiver);
        if (typeof value !== "function") return value;
        return (/** @type {unknown[]} */ ...args) => {
            if (disposed) throw new Error("Engine byl ukončen.");
            return value(...args);
        };
    } });
    const scene = new Scene(wasm);
    const xr = new WebXRHost(wasm, canvas, object => handleBindings.get(object) === wasm, id => new SceneObject(wasm, id));

    /** @type {import("./engine.d.ts").Engine} */
    const engine = Object.freeze({
        scene,
        xr,
        dispose() {
            if (disposed) return;
            xr.dispose();
            scene.cancelLoads();
            disposed = true;
            try { bindings.dispose(); } finally {
                if (ownership[initializationKey] === initialization) delete ownership[initializationKey];
            }
        },
        /** @param {Vector3} color Linear RGB clear color. */
        setBackgroundColor(color) { accepted(wasm.set_background_color(...color)); },
        setSceneAppearance(options) { wasm.set_scene_appearance(JSON.stringify(options)); },
        /** Geometry glow in both eyes, independent of postprocessing. Zero strength releases caches.
         * @param {{strength?:number,radius?:number}} [options] */
        setNeonGlow({strength=.6,radius=.015} = {}) { accepted(wasm.set_neon_glow(strength,radius)); },
        // Thin commands only: pass order, resources and execution live in Rust.
        postprocessing: Object.freeze({
            /** @param {import("./engine.d.ts").PostEffectName} name */
            enable(name, { strength = 0.6 } = {}) {
                accepted(wasm.set_postprocess(name, true, strength));
            },
            configureGodRays({ density = 0.035, exposure = 2.5 } = {}) {
                accepted(wasm.configure_god_rays(density, exposure));
            },
            /** @param {Parameters<import("./engine.d.ts").PostProcessing["configureSSR"]>[0]} [options] */
            configureSSR({resolutionScale=.5,receiverBounds} = {}) {
                if(receiverBounds!==undefined&&receiverBounds!==null){
                    const {min,max}=receiverBounds;
                    if(min.length!==3||max.length!==3||![...min,...max].every(v=>Number.isFinite(v)&&Math.abs(v)<=1e6)||min.some((v,i)=>v>=max[i]))throw new RangeError('Invalid SSR receiver bounds');
                }
                accepted(wasm.configure_ssr(resolutionScale));
                if(receiverBounds!==undefined)accepted(wasm.configure_ssr_region(new Float32Array(receiverBounds===null?[]:[...receiverBounds.min,...receiverBounds.max])));
            },
            /** @param {import("./engine.d.ts").PostEffectName} name */
            disable(name) { accepted(wasm.set_postprocess(name, false, 0)); },
            /** @param {Parameters<import("./engine.d.ts").PostProcessing["setReflectionMask"]>[0]} options */
            setReflectionMask({ width, height, pixels, center = [0, 0], size = [1, 1] }) {
                accepted(wasm.set_reflection_mask(width, height, pixels, ...center, ...size));
            },
            /** @param {Parameters<import("./engine.d.ts").PostProcessing["configureEnvironment"]>[0]} [options] */
            configureEnvironment({ enabled = true, center = /** @type {Vector3} */ ([0, 2, 0]), halfExtents = /** @type {Vector3} */ ([14, 10, 14]) } = {}) {
                accepted(wasm.configure_environment(enabled, ...center, ...halfExtents));
            },
            /** @param {Parameters<import("./engine.d.ts").PostProcessing["setEnvironmentCubeMap"]>[0]} options */
            setEnvironmentCubeMap(options) {
                if (options === null) { accepted(wasm.clear_environment_cube_map()); return; }
                const { size, color, emission, intensity = 1.2, projection = null } = options;
                if (!Number.isInteger(size) || size < 1 || size > 2048 || !(color instanceof Uint8Array) || !(emission instanceof Uint8Array) || color.length !== size * size * 6 * 4 || emission.length !== color.length || !Number.isFinite(intensity) || intensity < 0 || intensity > 8) throw new RangeError("Invalid environment cube map");
                let projectionValues = [];
                if (projection !== null) {
                    const { center, halfExtents } = projection;
                    if (!Array.isArray(center) || center.length !== 3 || ![...center].every(v => Number.isFinite(v) && Math.abs(v) <= 1e6) || !Array.isArray(halfExtents) || halfExtents.length !== 3 || ![...halfExtents].every(v => Number.isFinite(v) && v > 0 && v <= 1e6)) throw new RangeError("Invalid environment projection");
                    projectionValues = [...center, ...halfExtents];
                }
                accepted(wasm.set_environment_cube_map(size, color, emission, intensity, new Float32Array(projectionValues)));
            },
            /** @param {number} intensity */
            setEnvironmentIntensity(intensity) {
                if (!Number.isFinite(intensity) || intensity < 0 || intensity > 8) throw new RangeError("Invalid environment intensity");
                accepted(wasm.set_environment_intensity(intensity));
            },
            environmentCaptures() { return wasm.environment_captures(); },
            graph() { return JSON.parse(wasm.postprocess_graph()); },
            stats() {
                const [passes, width, height, targets, frames] = wasm.postprocess_stats();
                const [ssrWidth,ssrHeight,ssrPixels] = wasm.ssr_stats();
                const [ssrTracePixels,ssrViewportPixels] = wasm.ssr_work_stats();
                return { passes, width, height, targets, frames,ssrWidth,ssrHeight,ssrPixels,ssrTracePixels,ssrViewportPixels };
            },
        }),
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
        set_projectile: wasm.set_projectile,
        set_camera_mode: wasm.set_camera_mode,
        /** @param {Vector3} position @param {Vector3} target */
        setCameraView(position, target) { accepted(wasm.set_camera_view(...position, ...target)); },
        /** @param {Vector3} position @param {Vector3} target */
        setOrbitView(position,target) {accepted(wasm.set_orbit_view(...position,...target));},
        /** @param {number} dx @param {number} dy @param {number} [zoom] */
        adjustOrbit(dx,dy,zoom=0) {accepted(wasm.adjust_orbit(dx,dy,zoom));},
        /** @returns {import('./engine.d.ts').CameraViewState} */
        cameraViewState() {return JSON.parse(wasm.camera_view_state());},
        /** @param {Vector3} point @returns {readonly [number,number]|null} */
        worldToScreen(point) {const p=wasm.world_to_screen(...point);return p.length===2?[p[0],p[1]]:null;},
        /** @param {readonly [number,number]} point @param {number} [height] @returns {Vector3} */
        screenToPlane(point, height = 0) {
            const p = wasm.screen_to_plane(...point, height);
            return [p[0], p[1], p[2]];
        },
        /** @param {ModelObject} model @param {import('./engine.d.ts').CharacterOptions} [options] */
        controlCharacter(model, { speed = 2.5, runSpeed = 5, modelForwardYaw = 0, animations = {}, physics } = {}) {
            if (!(model instanceof ModelObject) || handleBindings.get(model) !== wasm) {
                throw new Error('Postava musí být model tohoto enginu.');
            }
            if (![speed, runSpeed, modelForwardYaw].every(Number.isFinite) || speed <= 0 || runSpeed <= 0) {
                throw new Error('Rychlosti musí být kladná konečná čísla a natočení konečné číslo.');
            }
            const clips = model.info.animations;
            const indices = [animations.idle, animations.walk, animations.run].map(name => {
                if (name === undefined) return -1;
                const index = clips.findIndex(clip => clip.name === name);
                if (index < 0) throw new Error(`Model nemá animaci ${name}.`);
                return index;
            });
            const body = physics?.halfExtents ?? [0.4, 0.8, 0.8];
            const gravity = physics?.gravity ?? 18;
            const jumpSpeed = physics?.jumpSpeed ?? 7;
            if (physics && (body.length !== 3 || ![...body, gravity, jumpSpeed]
                .every(value => Number.isFinite(Math.fround(value)) && Math.fround(value) > 0))) {
                throw new Error('Rozměry těla, gravitace a rychlost skoku musí být kladná konečná čísla.');
            }
            accepted(wasm.control_character(model.id, speed, runSpeed, modelForwardYaw, indices[0], indices[1], indices[2]));
            if (physics) accepted(wasm.configure_character_physics(body[0], body[1], body[2], gravity, jumpSpeed));
        },
        characterState() { return JSON.parse(wasm.character_state()); },
        set_grid_visible: wasm.set_grid_visible,
        set_light_helpers_visible: wasm.set_light_helpers_visible,
        set_output_mode: wasm.set_output_mode,
        set_render_effects: wasm.set_render_effects,
        set_simulation_paused: wasm.set_simulation_paused,
        effects_stats() {
            const [staticUpdates,dynamicUpdates,reflectionUpdates,staticVisible,dynamicVisible,reflectionVisible,passes,shadowResolution,reflectionWidth,reflectionHeight,shadowLightCount,...updates] = wasm.effects_stats();
            return {staticUpdates,dynamicUpdates,reflectionUpdates,staticVisible,dynamicVisible,reflectionVisible,passes,shadowResolution,reflectionWidth,reflectionHeight,shadowLightCount,staticUpdatesByLight:updates.slice(0,4),dynamicUpdatesByLight:updates.slice(4,8)};
        },
        /** @param {number} width @param {number} height */
        resize(width, height) {
            if (![width, height].every(value => Number.isInteger(value) && value > 0 && value <= 0xffffffff)) {
                throw new Error('Rozměry enginu musí být kladná celá čísla typu u32.');
            }
            wasm.resize(width, height);
        },
        renderer_backend: wasm.renderer_backend,
        set_frustum_culling: wasm.set_frustum_culling,
        /** Diagnostic main-view controls; shadows/reflections stay global. @param {boolean} roots @param {boolean} primitives */
        setMainViewCulling(roots, primitives) { wasm.set_main_view_culling(roots, primitives); },
        /** Optional Rust-rendered HUD; disabled by default. */
        /** @param {boolean} enabled */
        setDebugOverlay(enabled) { wasm.set_debug_overlay(enabled); },
        /** Diagnostic only:0 production,1 cheap fragment,3 stable ordinal merge. @param {0|1|3} mode */
        setRendererExperiment(mode) { return Number.isInteger(mode) && [0,1,3].includes(mode) && wasm.set_renderer_experiment(mode); },
        /** Detailed native rigid buffer layouts; query outside clean timing windows. */
        gltfIndexLayouts() { return JSON.parse(wasm.gltf_index_layouts()); },
        /** Original Uint32 reference; rejects live-asset changes. @param {boolean} enabled */
        setGltfCompactIndicesEnabled(enabled) { return wasm.set_gltf_compact_indices_enabled(enabled); },
        gltfVertexLayouts() { return JSON.parse(wasm.gltf_vertex_layouts()); },
        /** Immutable rigid layout reference; rejects changes while assets are live. @param {boolean} enabled */
        setGltfWhiteColorOmissionEnabled(enabled) { return wasm.set_gltf_white_color_omission_enabled(enabled); },
        /** Rust-owned bounded instance resources. @param {{byteLimit?:number,resourceLimit?:number,streamByteLimit?:number}} [options] */
        configureGltfInstancePool({byteLimit,resourceLimit,streamByteLimit} = {}) { return wasm.configure_gltf_instance_pool(byteLimit,resourceLimit,streamByteLimit); },
        /** Diagnostic upload experiment: 0=union (default), 1=coalesced, 2=full active span. @param {number} mode @param {number} [gapBytes] @param {boolean} [diagnostic] */
        setInstanceUploadPolicy(mode, gapBytes = 0, diagnostic = false) { return wasm.set_instance_upload_policy(mode, gapBytes, diagnostic); },
        setPanelBufferReuse(enabled) { wasm.set_panel_buffer_reuse(enabled); },
        diagnosticPanelText(id,value) { wasm.diagnostic_panel_text(id,value); },
        /** Diagnostic only: true preserves the default original scan; false opts into the experimental immutable guard. @param {boolean} enabled */
        setOverlaySelectionReference(enabled) { wasm.set_overlay_selection_reference(enabled); },
        /** Reference control for model GPU publication; false scans every model. @param {boolean} enabled */
        setModelPublicationReuseEnabled(enabled) { wasm.set_model_publication_reuse_enabled(enabled); },
        /** Exact opaque prepared-plan reuse; disable for independent comparison. @param {boolean} enabled */
        setPreparedPlanCacheEnabled(enabled) { wasm.set_prepared_plan_cache_enabled(enabled); },
        debugStats() { return JSON.parse(wasm.debug_stats()); },
        frameProfile() { return JSON.parse(wasm.frame_profile_stats()); },
        projectileContacts() { return wasm.projectile_contacts(); },
        renderer_stats() {
            const [total, visible, culled, drawCalls, uploadBytes, prepareMs, fps, frameMs] = wasm.renderer_stats();
            return {total, visible, culled, drawCalls, uploadBytes, prepareMs, fps, frameMs};
        },
    });
    return engine;
}
