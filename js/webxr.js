// @ts-nocheck -- WebXR browser interfaces are not yet part of TypeScript lib.dom.
// Platform adapter only: XR scheduling and canvas transfer. Rust computes views and scene state.
/** Logical tracked person. Scene objects are equipment, never built-in weapons or body meshes. */
export class XRPlayer {
    #engine; #host; #owns; #object; #alive;
    constructor(engine, host, owns, object, alive) {
        this.#engine=engine;this.#host=host;this.#owns=owns;this.#object=object;this.#alive=alive;
    }
    #check(object) {
        if(!this.#alive())throw new Error('WebXR byl ukončen.');
        if(object!==undefined&&object!==null&&!this.#owns(object))throw new Error('Objekt musí patřit tomuto enginu.');
    }
    get state() {
        this.#check();
        const state=JSON.parse(this.#engine.xr_player_state());
        for(const key of ['head','leftHand','rightHand'])state[key]={...state[key],object:state[key].object===null?null:this.#object(state[key].object)};
        return {...state,inventory:state.inventory.map(this.#object)};
    }
    get head(){return this.state.head;}
    get leftHand(){return this.state.leftHand;}
    get rightHand(){return this.state.rightHand;}
    get inventory(){return this.state.inventory;}
    bindHead(object){this.#check(object);this.#engine.xr_bind_head(object?.id??0xffffffff);return this;}
    addItem(object){this.#check(object);if(!object)throw new Error('Chybí objekt.');this.#engine.xr_add_item(object.id);return this;}
    removeItem(object){
        this.#check(object);if(!object)throw new Error('Chybí objekt.');
        const state=this.state;
        for(const [hand,key] of [['left','leftHand'],['right','rightHand']])if(state[key].object?.id===object.id)this.equip(hand,null);
        this.#engine.xr_remove_item(object.id);return this;
    }
    equip(hand,object,options={}){this.#check(object);this.#host.bindController(hand,object,options);return this;}
    unequip(hand){return this.equip(hand,null);}
}

export class WebXRHost extends EventTarget {
    #gpuErrorDevice=null;
    #onGpuError=event=>this.dispatchEvent(new CustomEvent('gpuerror',{detail:{message:event.error.message,type:event.error.constructor.name}}));
    #player;
    get player(){return this.#player;}
    #engine; #canvas; #session = null; #pending = false; #disposed = false;
    #gpuBinding = null; #gpuLayer = null; #gpuSessionUnavailable = false;
    #gl; #program; #textures = []; #vao; #frame = null; #size; #previous = null;
    #pressed = [null, null];
    #sources = [null, null];
    #space = null;
    #trackingActive = false;
    #roundEnded = false;
    #button = null;
    #buttonPoints = [undefined, undefined];
    #controllerObjects = [null, null];
    #bladePoses = [null, null];
    #buttonTouch = [false, false];
    #haptics = [false, false];
    #contactActive = [false, false];
    #resetInputs = () => {
        this.#pressed = [null, null];
        this.#bladePoses = [null, null];
        this.#buttonTouch = [false, false];
        this.#buttonPoints = [undefined, undefined];
        this.#sources = [null, null];
        this.#contactActive = [false, false];
        if (this.#session && this.#trackingActive && !this.#disposed) {
            this.#engine.xr_update_head(new Float32Array());
            for (let index=0; index<2; index++) this.#engine.xr_update_controller(index,new Float32Array(),false);
        }
    };
    #visibilityChanged = () => { this.#previous = null; this.#resetInputs(); };
    #requireTrigger = [true, true];
    #owns; #object;
    #onEnd = () => this.#cleanup();
    /** @param {any} engine @param {HTMLCanvasElement} canvas
     * @param {(object: object) => boolean} owns @param {(id: number) => any} object */
    constructor(engine, canvas, owns = () => false, object = id => id) { super(); this.#engine = engine; this.#canvas = canvas; this.#owns = owns; this.#object = object; this.#player=new XRPlayer(engine,this,owns,object,()=>!this.#disposed); }
    bindController(hand, object, {cutting = false, requireTrigger = true, haptics = false} = {}) {
        if (this.#disposed) throw new Error('WebXR byl ukončen.');
        const index = hand === 'left' ? 0 : hand === 'right' ? 1 : -1;
        if (index < 0 || (object !== null && !this.#owns(object))) throw new Error('Ovladač vyžaduje left/right a objekt tohoto enginu.');
        this.#engine.xr_bind_controller(index, object === null ? 0xffffffff : object.id, cutting);
        this.#requireTrigger[index] = requireTrigger;
        this.#controllerObjects[index] = object;
        this.#haptics[index] = !!object && haptics;
        this.#contactActive[index] = false;
        this.#bladePoses[index] = null;
        this.#buttonTouch[index] = false;
    }
    bindButton(object) {
        if (this.#disposed || (object !== null && !this.#owns(object))) throw new Error('Tlačítko musí patřit tomuto enginu.');
        this.#button = object;
        this.#bladePoses = [null, null];
        this.#buttonTouch = [false, false];
        this.#buttonPoints = [undefined, undefined];
    }
    #updateControllers(session, frame, space, tracking) {
        const sources = [...(session.inputSources ?? [])];
        for (let index = 0; index < 2; index++) {
            const hand = index === 0 ? 'left' : 'right';
            const source = sources.find(source => source.handedness === hand && source.gripSpace);
            if (source !== this.#sources[index]) {
                this.#engine.xr_update_controller(index,new Float32Array(),false);
                this.#pressed[index] = null;
                this.#bladePoses[index] = null;
                this.#buttonTouch[index] = false;
                this.#sources[index] = source;
                this.#contactActive[index] = false;
            }
            const pose = tracking && source ? frame.getPose(source.gripSpace, space) : null;
            const pressed = !!pose && !!source.gamepad?.buttons[0]?.pressed;
            const report = JSON.parse(this.#engine.xr_update_controller(index, pose ? pose.transform.matrix : new Float32Array(), !!pose && (pressed || !this.#requireTrigger[index])));
            const contact = !!pose && report.bladeContacts > 0;
            if (pose && this.#haptics[index]) {
                if (report.removed.length) this.#pulse(source, 0.45, 35);
                else if (contact && !this.#contactActive[index]) this.#pulse(source, 0.2, 20);
            }
            this.#contactActive[index] = contact;
            if (report.removed.length) this.dispatchEvent(new CustomEvent('cut', {detail: {...report, pieces: report.pieces.map(this.#object), handedness: hand}}));
            let point = null;
            if (this.#button && pose && source?.targetRaySpace) {
                const ray = frame.getPose(source.targetRaySpace, space);
                if (ray) {
                    const hit = this.#engine.xr_panel_hit(this.#button.id, ray.transform.matrix);
                    if (hit.length === 3) point = Array.from(hit);
                }
            }
            if (this.#button) {
                const previous=this.#buttonPoints[index];
                if (point===null ? previous!==null : !previous || point.some((v,i)=>v!==previous[i])) {
                    this.#buttonPoints[index]=point;
                    this.dispatchEvent(new CustomEvent('buttonhover', {detail: {handedness: hand, point}}));
                }
            }
            if (pressed && this.#pressed[index] === false) this.dispatchEvent(new CustomEvent('select', {detail: {handedness: hand, target: point ? this.#button : null}}));
            const tool = this.#controllerObjects[index];
            if (this.#button && tool && pose) {
                const hit = this.#engine.xr_panel_blade_hit(this.#button.id, tool.id,
                    this.#bladePoses[index] ?? new Float32Array(), pose.transform.matrix);
                const strikes = hit[0] === 1, touches = hit[1] === 1;
                if (strikes && !this.#buttonTouch[index]) this.dispatchEvent(new CustomEvent('buttonpress', {
                    detail: {handedness: hand, target: this.#button}
                }));
                this.#buttonTouch[index] = strikes || touches;
                this.#bladePoses[index] = new Float32Array(pose.transform.matrix);
            } else {
                this.#bladePoses[index] = null;
                this.#buttonTouch[index] = false;
            }
            this.#pressed[index] = pose ? pressed : null;
        }
    }
    #pulse(source, intensity, milliseconds) {
        const actuator = source?.gamepad?.hapticActuators?.[0] ?? source?.gamepad?.vibrationActuator;
        if (typeof actuator?.pulse !== 'function') return;
        try { Promise.resolve(actuator.pulse(intensity, milliseconds)).catch(() => {}); }
        catch { /* Haptics are optional platform feedback. */ }
    }
    get isPresenting() { return this.#session !== null; }
    get roundState() { return JSON.parse(this.#engine.xr_round_state()); }
    startRound(seconds = 30) {
        if (this.#disposed || !this.#session) throw new Error('Kolo vyžaduje aktivní VR.');
        this.#engine.xr_start_round(seconds);
        this.#roundEnded = false;
    }
    async isSupported() {
        return !!globalThis.isSecureContext && !!navigator.xr && (this.#engine.renderer_backend() === 'Gl' || (!this.#gpuSessionUnavailable && typeof globalThis.XRGPUBinding === 'function' && this.#engine.xr_gpu_supported())) && await navigator.xr.isSessionSupported('immersive-vr');
    }
    async enter() {
        if (this.#disposed) throw new Error('WebXR byl ukončen.');
        if (this.#pending || this.#session) throw new Error('WebXR již běží nebo se spouští.');
        if (!globalThis.isSecureContext) throw new Error('WebXR potřebuje důvěryhodné HTTPS nebo localhost přes ADB.');
        if (!navigator.xr) throw new Error('Tento prohlížeč nemá WebXR. Otevřete stránku v Meta Quest Browser.');
        const gpu = this.#engine.renderer_backend() === 'BrowserWebGpu';
        if (gpu && typeof globalThis.XRGPUBinding !== 'function') throw new Error('Prohlížeč nemá WebGPU pro WebXR (XRGPUBinding). Použijte odkaz WebGL.');
        if (gpu && this.#gpuSessionUnavailable) throw new Error('Tento prohlížeč odmítl WebGPU ve VR. Aktualizujte Meta Quest Browser, nebo použijte odkaz WebGL / WebXR.');
        if (gpu && !this.#engine.xr_gpu_supported()) throw new Error('WebGPU XR vyžaduje kompatibilní adaptér. Použijte WebGL.');
        this.lastError = undefined;
        this.#pending = true;
        let session;
        try {
            // Request immediately in the click gesture, before any asynchronous support probe.
            try {
                session = await navigator.xr.requestSession('immersive-vr', { optionalFeatures: ['local-floor'], ...(gpu ? {requiredFeatures:['webgpu']} : {}) });
            } catch (error) {
                if (gpu && error?.name === 'NotSupportedError') {
                    this.#gpuSessionUnavailable = true;
                    throw Object.assign(new Error('WebGPU náhled funguje, ale tento prohlížeč nepovoluje WebGPU ve VR. Aktualizujte Meta Quest Browser, nebo použijte odkaz WebGL / WebXR.', {cause:error}), {name:'NotSupportedError'});
                }
                throw error;
            }
            if (this.#disposed) throw new Error('Spuštění WebXR bylo zrušeno.');
            this.#session = session;
            session.addEventListener('end', this.#onEnd);
            this.#size = [this.#canvas.width, this.#canvas.height];
            let gl = null, layer = null;
            if (gpu) {
                const device=this.#engine.xr_gpu_device();
                if(this.#gpuErrorDevice!==device){
                    this.#gpuErrorDevice?.removeEventListener('uncapturederror',this.#onGpuError);
                    this.#gpuErrorDevice=device;
                    device.addEventListener('uncapturederror',this.#onGpuError);
                    const owner=new WeakRef(this);
                    void device.lost.then(info=>{
                        const host=owner.deref();
                        if(host&&!host.#disposed&&host.#gpuErrorDevice===device&&info.reason!=='destroyed')host.dispatchEvent(new CustomEvent('gpulost',{detail:{message:info.message,reason:info.reason}}));
                    });
                }
                this.#gpuBinding = new XRGPUBinding(session, device);
                this.#gpuLayer = this.#gpuBinding.createProjectionLayer({
                    colorFormat:this.#engine.xr_gpu_color_format(), depthStencilFormat:'depth32float', textureUsage:GPUTextureUsage.RENDER_ATTACHMENT, scaleFactor:0.75,
                });
                session.updateRenderState({layers:[this.#gpuLayer],depthNear:0.05,depthFar:100});
            } else {
                const canvas = document.createElement('canvas');
                gl = canvas.getContext('webgl2', { xrCompatible: true, alpha: false, antialias: false });
                if (!gl) throw new Error('WebGL2 není dostupné.');
                this.#gl = gl;
                await gl.makeXRCompatible();
                if (this.#disposed || this.#session !== session) throw new Error('Spuštění WebXR bylo zrušeno.');
                layer = new XRWebGLLayer(session, gl, { alpha: false, depth: false, antialias: false, framebufferScaleFactor: 0.75 });
                session.updateRenderState({ baseLayer: layer, depthNear: 0.05, depthFar: 100 });
            }
            let space;
            try { space = await session.requestReferenceSpace('local-floor'); }
            catch {
                const local = await session.requestReferenceSpace('local');
                space = local.getOffsetReferenceSpace(new XRRigidTransform({ y: -1.6 }));
            }
            if (this.#disposed || this.#session !== session) throw new Error('Spuštění WebXR bylo zrušeno.');
            this.#space = space;
            space.addEventListener?.('reset',this.#resetInputs);
            session.addEventListener('visibilitychange',this.#visibilityChanged);
            if (!gpu) this.#createBlit();
            this.#engine.xr_set_presenting(true);
            this.#trackingActive = true;
            let firstFrame = true;
            // Bounded diagnostics: measure delivered XR cadence and CPU submission,
            // never stall the GPU or confuse CPU time with GPU execution time.
            let warmupFrames=60,perfFrames=0,perfElapsed=0,perfCpu=0,perfReports=0;
            let eyeViewports=[];
            const frame = (time, xrFrame) => {
                this.#frame = null;
                if (this.#session !== session) return;
                try {
                    const cpuStart=performance.now();
                    const elapsed=this.#previous===null?0:time-this.#previous;
                    const pose = xrFrame.getViewerPose(space);
                    const firstGpuEyes=firstFrame&&gpu?[]:null;
                    this.#engine.xr_begin_frame(this.#previous === null ? 0 : Math.max(0, (time - this.#previous) / 1000));
                    this.#previous = time;
                    const round = this.roundState;
                    if (round?.finished && !this.#roundEnded) {
                        this.#roundEnded = true;
                        this.dispatchEvent(new CustomEvent('roundend', {detail: round}));
                    }
                    const tracked = !!pose && (!session.visibilityState || session.visibilityState === 'visible');
                    this.#engine.xr_update_head(tracked ? pose.transform.matrix : new Float32Array());
                    this.#updateControllers(session, xrFrame, space, !!pose && (!session.visibilityState || session.visibilityState === 'visible'));
                    if (gpu) {
                        const images=[];
                        if (pose) for (const view of pose.views) {
                            const image = this.#gpuBinding.getViewSubImage(this.#gpuLayer, view);
                            const viewDescriptor=image.getViewDescriptor();
                            if(firstFrame)eyeViewports.push({eye:view.eye,width:image.viewport.width,height:image.viewport.height});
                            try {
                                const attachments={colorTexture:image.colorTexture,depthStencilTexture:image.depthStencilTexture,
                                    viewDescriptor,viewport:image.viewport,ignoreDepthValues:this.#gpuLayer.ignoreDepthValues===true};
                                this.#engine.xr_render_gpu_eye(view.projectionMatrix,view.transform.matrix,attachments);
                                images.push(attachments);
                                if(firstGpuEyes){
                                    const metadata=texture=>texture ? Object.fromEntries(['width','height','depthOrArrayLayers','mipLevelCount','sampleCount','dimension','format','usage'].map(key=>[key,texture[key]])) : null;
                                    firstGpuEyes.push({eye:view.eye,color:metadata(image.colorTexture),depth:metadata(image.depthStencilTexture),
                                        viewDescriptor:{dimension:viewDescriptor.dimension,baseArrayLayer:viewDescriptor.baseArrayLayer,arrayLayerCount:viewDescriptor.arrayLayerCount},
                                        viewport:{x:image.viewport.x,y:image.viewport.y,width:image.viewport.width,height:image.viewport.height}});
                                }
                            } catch(error) {
                                const metadata=texture=>texture ? Object.fromEntries(['width','height','depthOrArrayLayers','mipLevelCount','sampleCount','dimension','format','usage'].map(key=>[key,texture[key]])) : null;
                                const attachment={eye:view.eye,color:metadata(image.colorTexture),depth:metadata(image.depthStencilTexture),viewDescriptor,
                                    viewport:{x:image.viewport.x,y:image.viewport.y,width:image.viewport.width,height:image.viewport.height}};
                                throw new Error(`${error} · XR attachments: ${JSON.stringify(attachment)}`,{cause:error});
                            }
                        }
                        if(images.length)this.#engine.xr_present_gpu_frame(images);
                    } else {
                        gl.bindFramebuffer(gl.FRAMEBUFFER, layer.framebuffer);
                        gl.clearColor(0.1, 0.1, 0.3, 1);
                        gl.clear(gl.COLOR_BUFFER_BIT);
                        if (pose) for (const [index, view] of pose.views.entries()) {
                            const viewport = layer.getViewport(view);
                            if (!viewport) continue;
                            if(firstFrame)eyeViewports.push({eye:view.eye,width:viewport.width,height:viewport.height});
                            this.#engine.xr_render_eye(view.projectionMatrix, view.transform.matrix, viewport.width, viewport.height);
                            this.#uploadCanvas(index);
                            gl.viewport(viewport.x, viewport.y, viewport.width, viewport.height);
                            gl.useProgram(this.#program);
                            gl.bindVertexArray(this.#vao);
                            gl.drawArrays(gl.TRIANGLES, 0, 3);
                        }
                    }
                    if (firstFrame && pose?.views.length) {
                        firstFrame = false;
                        this.dispatchEvent(new CustomEvent('firstframe',{detail:{enabledFeatures:Array.from(session.enabledFeatures??[]),
                            ...(gpu?{presentation:'stereo-final-submit-v1',usesDepthValues:this.#gpuBinding.usesDepthValues,ignoreDepthValues:this.#gpuLayer.ignoreDepthValues,fixedFoveation:this.#gpuLayer.fixedFoveation,eyes:firstGpuEyes}:{})}}));
                    }
                    if(tracked&&elapsed>0&&elapsed<250&&perfReports<3){
                        if(warmupFrames>0)warmupFrames--;
                        else {
                            perfFrames++;perfElapsed+=elapsed;perfCpu+=performance.now()-cpuStart;
                            if(perfFrames===180){
                                this.dispatchEvent(new CustomEvent('performance',{detail:{frames:perfFrames,fps:1000*perfFrames/perfElapsed,frameMs:perfElapsed/perfFrames,cpuSubmitMs:perfCpu/perfFrames,eyeViewports}}));
                                perfReports++;perfFrames=0;perfElapsed=0;perfCpu=0;
                            }
                        }
                    }else {perfFrames=0;perfElapsed=0;perfCpu=0;}
                    this.#frame = session.requestAnimationFrame(frame);
                } catch (error) {
                    this.lastError = error;
                    void session.end().catch(() => {});
                    this.#cleanup();
                    this.dispatchEvent(new Event('error'));
                }
            };
            this.#frame = session.requestAnimationFrame(frame);
            this.dispatchEvent(new Event('start'));
        } catch (error) {
            if (session) await session.end().catch(() => {});
            this.#cleanup();
            throw error;
        } finally { this.#pending = false; }
    }
    async exit() { if (this.#session) await this.#session.end(); }
    dispose() {
        if (this.#disposed) return;
        this.#disposed = true;
        this.#gpuErrorDevice?.removeEventListener('uncapturederror',this.#onGpuError);
        this.#gpuErrorDevice=null;
        void this.exit().catch(() => {});
        this.#cleanup();
    }
    #cleanup() {
        const session = this.#session;
        if (!session) return;
        this.#session = null;
        this.#trackingActive = false;
        if (this.#frame !== null) session.cancelAnimationFrame(this.#frame);
        session.removeEventListener('end', this.#onEnd);
        session.removeEventListener('visibilitychange',this.#visibilityChanged);
        this.#space?.removeEventListener?.('reset',this.#resetInputs);
        this.#space = null;
        this.#sources = [null,null];
        this.#contactActive = [false, false];
        this.#frame = null;
        this.#previous = null;
        this.#pressed = [null, null];
        this.#bladePoses = [null, null];
        this.#buttonTouch = [false, false];
        this.#buttonPoints = [undefined, undefined];
        try {
            this.#engine.xr_set_presenting(false);
            if (this.#size && !this.#gpuBinding) this.#engine.resize(...this.#size);
        } catch { /* Engine disposal can precede an asynchronous session end. */ }
        if (this.#gl) {
            for (const image of this.#textures) if (image) this.#gl.deleteTexture(image.texture);
            this.#gl.deleteProgram(this.#program);
            this.#gl.deleteVertexArray(this.#vao);
            this.#gl.getExtension('WEBGL_lose_context')?.loseContext();
        }
        this.#gpuLayer?.destroy?.();
        this.#gpuBinding = this.#gpuLayer = null;
        this.#textures = [];
        this.#gl = this.#program = this.#vao = null;
        this.dispatchEvent(new Event('end'));
    }
    #uploadCanvas(index) {
        const gl = this.#gl;
        let image = this.#textures[index];
        if (!image) {
            image = this.#textures[index] = {texture: gl.createTexture(), width: 0, height: 0};
            gl.bindTexture(gl.TEXTURE_2D, image.texture);
            gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
            gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
            gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
            gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        } else gl.bindTexture(gl.TEXTURE_2D, image.texture);
        const {width, height} = this.#canvas;
        // Each eye retains its storage even when the viewports have different sizes.
        if (image.width !== width || image.height !== height) {
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
            image.width = width; image.height = height;
        }
        // Cross-context canvas upload still has browser synchronization/copy costs; no JS pixel readback.
        gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
        gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, this.#canvas);
    }
    #createBlit() {
        const gl = this.#gl;
        const shader = (type, source) => {
            const shader = gl.createShader(type);
            gl.shaderSource(shader, source); gl.compileShader(shader);
            if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
                const error = gl.getShaderInfoLog(shader); gl.deleteShader(shader); throw new Error(error);
            }
            return shader;
        };
        const vertex = shader(gl.VERTEX_SHADER, `#version 300 es
        out vec2 uv;
        void main(){ vec2 p=vec2(float((gl_VertexID<<1)&2),float(gl_VertexID&2)); uv=p; gl_Position=vec4(p*2.-1.,0.,1.); }`);
        const fragment = shader(gl.FRAGMENT_SHADER, `#version 300 es
        precision highp float; in vec2 uv; uniform sampler2D image; out vec4 color;
        void main(){color=texture(image,uv);}`);
        this.#program = gl.createProgram();
        gl.attachShader(this.#program, vertex); gl.attachShader(this.#program, fragment); gl.linkProgram(this.#program);
        gl.deleteShader(vertex); gl.deleteShader(fragment);
        if (!gl.getProgramParameter(this.#program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(this.#program));
        this.#vao = gl.createVertexArray();
    }
}
