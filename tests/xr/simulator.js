import {matrixFromPose, multiply, inverse} from './poses.js';

const DEFAULT_PROFILE = {id:'standing-72',hz:72,ipd:0.064,eyeWidth:320,eyeHeight:320};
const copy = value => JSON.parse(JSON.stringify(value));
const matrix = value => {
    if (!value || value.length!==16 || !Array.from(value).every(Number.isFinite)) throw new TypeError('Invalid XR matrix');
    const result = new Float32Array(value); inverse(result); return result;
};
const poseMatrix = (pose, previous) => pose.matrix ? matrix(pose.matrix) :
    (pose.position || pose.orientation ? matrixFromPose({position:pose.position ?? Array.from(previous).slice(12,15),orientation:pose.orientation ?? quaternion(previous)}) : previous.slice());
function quaternion(m) {
    const trace=m[0]+m[5]+m[10]; let x,y,z,w;
    if(trace>0) {const s=Math.sqrt(trace+1)*2; w=s/4;x=(m[6]-m[9])/s;y=(m[8]-m[2])/s;z=(m[1]-m[4])/s;}
    else if(m[0]>m[5]&&m[0]>m[10]) {const s=Math.sqrt(1+m[0]-m[5]-m[10])*2;w=(m[6]-m[9])/s;x=s/4;y=(m[4]+m[1])/s;z=(m[8]+m[2])/s;}
    else if(m[5]>m[10]) {const s=Math.sqrt(1+m[5]-m[0]-m[10])*2;w=(m[8]-m[2])/s;x=(m[4]+m[1])/s;y=s/4;z=(m[9]+m[6])/s;}
    else {const s=Math.sqrt(1+m[10]-m[0]-m[5])*2;w=(m[1]-m[4])/s;x=(m[8]+m[2])/s;y=(m[9]+m[6])/s;z=s/4;}
    return [x,y,z,w];
}

export function installXRSimulator({profile:options={},clockTimers=true,floorAvailable=true,supported=true}={}) {
    const profile={...DEFAULT_PROFILE,...options};
    if(typeof profile.id!=='string'||!Number.isFinite(profile.hz)||profile.hz<=0||profile.hz>1000||!Number.isFinite(profile.ipd)||profile.ipd<0||profile.ipd>1||!['eyeWidth','eyeHeight'].every(key=>Number.isInteger(profile[key])&&profile[key]>0&&profile[key]<=8192)) throw new TypeError('Invalid XR profile');
    const viewports=profile.viewports??[{x:0,y:0,width:profile.eyeWidth,height:profile.eyeHeight},{x:profile.eyeWidth,y:0,width:profile.eyeWidth,height:profile.eyeHeight}];
    if(!Array.isArray(viewports)||viewports.length!==2||viewports.some(v=>!['x','y','width','height'].every(k=>Number.isInteger(v[k])&&v[k]>=(k==='width'||k==='height'?1:0)&&v[k]<=8192)))throw new TypeError('Invalid XR viewports');
    const framebufferWidth=Math.max(...viewports.map(v=>v.x+v.width)),framebufferHeight=Math.max(...viewports.map(v=>v.y+v.height));
    let session=null,layer=null,now=0,nextId=-1,disposed=false,deny=false,deferred=null,sessionIndex=-1,referenceReset=false;
    const pendingReleases=new Set();
    let head={matrix:matrixFromPose({position:[0,floorAvailable?1.55:-.05,0]}),tracked:true};
    const controllers={left:null,right:null}, frames=[], timers=new Map(), restorations=[], layers=new Set();
    const nativeTimeout=globalThis.setTimeout.bind(globalThis);
    const replace=(object,key,value)=>{
        const descriptor=Object.getOwnPropertyDescriptor(object,key);
        Object.defineProperty(object,key,{configurable:true,writable:true,value});
        restorations.push(()=>descriptor ? Object.defineProperty(object,key,descriptor) : delete object[key]);
    };
    const alive=()=>{if(disposed) throw new Error('XR simulator disposed');};
    const event=(target,type,properties={})=>{const value=new Event(type);Object.assign(value,properties);target.dispatchEvent(value);};
    class Transform {
        constructor(position={},orientation={}) {
            this.matrix=matrixFromPose({position:[position.x??0,position.y??0,position.z??0],orientation:[orientation.x??0,orientation.y??0,orientation.z??0,orientation.w??1]});
            this.position={x:this.matrix[12],y:this.matrix[13],z:this.matrix[14],w:1};
            const [x,y,z,w]=quaternion(this.matrix);this.orientation={x,y,z,w};
        }
        get inverse() {return transform(inverse(this.matrix));}
    }
    function transform(m) {
        const value=Object.create(Transform.prototype);value.matrix=m;
        value.position={x:m[12],y:m[13],z:m[14],w:1};
        const [x,y,z,w]=quaternion(m);value.orientation={x,y,z,w};return value;
    }
    class ReferenceSpace extends EventTarget {
        constructor(world=matrixFromPose(),owner=session) {super();this.world=world;this.owner=owner;}
        getOffsetReferenceSpace(offset) {const result=new ReferenceSpace(multiply(this.world,offset.matrix),this.owner);spaces.add(result);return result;}
    }
    const spaces=new Set();
    class Session extends EventTarget {
        constructor() {super();this.frameRate=profile.hz;this.supportedFrameRates=new Float32Array([profile.hz]);this.inputSources=[];this.ended=false;this.visibilityState='visible';this.renderState={depthNear:.1,depthFar:1000,baseLayer:null};this.callbacks=new Map();}
        get pendingFrameCount() {return this.callbacks.size;}
        requestAnimationFrame(callback) {if(this.ended) return 0;const id=nextId--;this.callbacks.set(id,callback);return id;}
        cancelAnimationFrame(id) {this.callbacks.delete(id);}
        updateRenderState(state) {this.renderState={...this.renderState,...state};}
        async requestReferenceSpace(type) {
            if(type==='local-floor'&&!floorAvailable) throw new DOMException('Floor unavailable','NotSupportedError');
            if(!['local-floor','local','viewer'].includes(type)) throw new DOMException('Reference space unavailable','NotSupportedError');
            const space=new ReferenceSpace(matrixFromPose({position:type==='local'&&floorAvailable?[0,1.6,0]:[0,0,0]}));spaces.add(space);return space;
        }
        async end() {if(this.ended)return;this.ended=true;this.callbacks.clear();event(this,'end');}
    }
    const xr={isSessionSupported:async()=>supported,requestSession:async()=>{
        alive();if(deferred){const pending=deferred;deferred=null;await pending;alive();}
        if(deny){deny=false;throw new DOMException('Simulated XR denial','NotAllowedError');}
        if(!supported)throw new DOMException('XR unavailable','NotSupportedError');
        if(session&&!session.ended)throw new DOMException('XR session already active','InvalidStateError');
        session=new Session();sessionIndex++;referenceReset=false;return session;
    }};
    replace(navigator,'xr',xr);replace(globalThis,'XRRigidTransform',Transform);
    for(const name of ['WebGLRenderingContext','WebGL2RenderingContext']) if(globalThis[name]) replace(globalThis[name].prototype,'makeXRCompatible',async function(){});
    replace(globalThis,'XRWebGLLayer',class {
        constructor(owner,gl,options={}) {
            const oldTexture=gl.getParameter(gl.TEXTURE_BINDING_2D),oldFramebuffer=gl.getParameter(gl.FRAMEBUFFER_BINDING),oldRenderbuffer=gl.getParameter(gl.RENDERBUFFER_BINDING);
            this.gl=gl;this.framebufferWidth=framebufferWidth;this.framebufferHeight=framebufferHeight;this.antialias=false;this.fixedFoveation=0;this.requestedOptions={...options};
            this.texture=gl.createTexture();this.framebuffer=gl.createFramebuffer();this.depth=gl.createRenderbuffer();layers.add(this);
            try {
                gl.bindTexture(gl.TEXTURE_2D,this.texture);
                gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.NEAREST);
                gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
                gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,this.framebufferWidth,this.framebufferHeight,0,gl.RGBA,gl.UNSIGNED_BYTE,null);
                gl.bindFramebuffer(gl.FRAMEBUFFER,this.framebuffer);
                gl.framebufferTexture2D(gl.FRAMEBUFFER,gl.COLOR_ATTACHMENT0,gl.TEXTURE_2D,this.texture,0);
                gl.bindRenderbuffer(gl.RENDERBUFFER,this.depth);
                gl.renderbufferStorage(gl.RENDERBUFFER,gl.DEPTH_COMPONENT16,this.framebufferWidth,this.framebufferHeight);
                gl.framebufferRenderbuffer(gl.FRAMEBUFFER,gl.DEPTH_ATTACHMENT,gl.RENDERBUFFER,this.depth);
                if(gl.checkFramebufferStatus(gl.FRAMEBUFFER)!==gl.FRAMEBUFFER_COMPLETE)throw new Error('XR framebuffer incomplete');
            } finally {gl.bindTexture(gl.TEXTURE_2D,oldTexture);gl.bindFramebuffer(gl.FRAMEBUFFER,oldFramebuffer);gl.bindRenderbuffer(gl.RENDERBUFFER,oldRenderbuffer);}
            layer=this;
        }
        getViewport(view) {return {...viewports[(view.eye==='right'||view.index===1)?1:0]};}
    });
    if(clockTimers) {
        const schedule=(repeat)=>(callback,delay=0,...args)=>{alive();if(typeof callback!=='function')throw new TypeError('XR timer needs a function');const interval=Math.max(1,Number(delay)||0);if(!Number.isFinite(interval))throw new TypeError('Invalid timer delay');const id=nextId--;timers.set(id,{callback:()=>callback(...args),delay:interval,next:now+interval,repeat});return id;};
        for(const host of new Set([globalThis,globalThis.window].filter(Boolean))) {
            replace(host,'setTimeout',schedule(false));replace(host,'setInterval',schedule(true));
            for(const name of ['clearTimeout','clearInterval']) {const original=host[name].bind(host);replace(host,name,id=>{if(timers.has(id))timers.delete(id);else if(!(typeof id==='number'&&id<0))original(id);});}
        }
    }
    function advance(dt) {
        const target=now+dt;let count=0;
        for(;;) {
            let selected=null,id=null;
            for(const [key,timer] of timers) if(timer.next<=target&&(!selected||timer.next<selected.next)){selected=timer;id=key;}
            if(!selected)break;if(++count>100000)throw new Error('XR timer runaway');
            now=selected.next;if(selected.repeat)selected.next+=selected.delay;else timers.delete(id);selected.callback();
        }
        now=target;
    }
    function updateController(hand,value) {
        if(value===null){controllers[hand]=null;return;}
        const old=controllers[hand];
        const state=old??{matrix:matrixFromPose({position:[hand==='left'?-.3:.3,floorAvailable?1.1:-.5,-.2]}),tracked:true,grip:true,buttons:[{pressed:false,touched:false,value:0}],axes:[0,0]};
        const next={...state,...value,matrix:poseMatrix(value,state.matrix)};
        next.targetRayMatrix=value.targetRayMatrix?matrix(value.targetRayMatrix):(state.rayExplicit?state.targetRayMatrix:next.matrix);
        next.rayExplicit=state.rayExplicit||!!value.targetRayMatrix;
        if(value.pressed!==undefined)next.buttons=[{pressed:!!value.pressed,touched:!!value.pressed,value:value.pressed?1:0}];
        else if(value.buttons)next.buttons=value.buttons.map(button=>typeof button==='number'?{pressed:button>0,touched:button>0,value:button}:{pressed:!!button.pressed,touched:button.touched??!!button.pressed,value:button.value??Number(!!button.pressed)});
        if(!Array.isArray(next.axes)||!next.axes.every(Number.isFinite))throw new TypeError('Invalid XR axes');
        if(!next.buttons.every(button=>Number.isFinite(button.value)))throw new TypeError('Invalid XR buttons');
        if(!old)next.source={handedness:hand,targetRayMode:'tracked-pointer',profiles:['generic-trigger'],targetRaySpace:{hand,kind:'ray'},gripSpace:{hand,kind:'grip'},gamepad:{mapping:'xr-standard',buttons:[],axes:[]}};
        next.gripSpace=state.gripSpace??next.source.gripSpace;
        next.source.gamepad.buttons=copy(next.buttons);next.source.gamepad.axes=[...next.axes];next.source.gripSpace=next.grip?next.gripSpace:null;
        controllers[hand]=next;
    }
    const api={
        get layer(){return layer;},
        get profile(){return {...profile};},get session(){return session;},get time(){return now;},get timerCount(){return timers.size;},
        timerCountForDelay(delay){return [...timers.values()].filter(timer=>timer.repeat&&timer.delay===Number(delay)).length;},
        gripX(angle,position){return matrixFromPose({position,orientation:[Math.sin(angle/2),0,0,Math.cos(angle/2)]});},
        frame(input={}) {
            alive();const dt=input.dt??1000/profile.hz;if(!Number.isFinite(dt)||dt<0||dt>10000)throw new TypeError('XR frame dt must be between 0 and 10000 ms');
            if(!session||session.ended)throw new Error('No active XR session');
            if(input.head)head={matrix:poseMatrix(input.head,head.matrix),tracked:input.head.tracked??head.tracked};
            const before=session.inputSources;
            for(const hand of ['left','right'])if(input.controllers&&Object.hasOwn(input.controllers,hand))updateController(hand,input.controllers[hand]);
            session.inputSources=['right','left'].flatMap(hand=>controllers[hand]?[controllers[hand].source]:[]);
            const added=session.inputSources.filter(source=>!before.includes(source)),removed=before.filter(source=>!session.inputSources.includes(source));
            if(added.length||removed.length)event(session,'inputsourceschange',{session,added,removed});
            frames.push({dt,visibilityState:session.visibilityState,referenceReset,sessionIndex,head:{matrix:Array.from(head.matrix),tracked:head.tracked},controllers:Object.fromEntries(['left','right'].map(hand=>[hand,controllers[hand]?{matrix:Array.from(controllers[hand].matrix),targetRayMatrix:Array.from(controllers[hand].targetRayMatrix),tracked:controllers[hand].tracked,grip:controllers[hand].grip,buttons:copy(controllers[hand].buttons),axes:[...controllers[hand].axes]}:null]))});
            referenceReset=false;
            advance(dt);
            if(session.ended||session.visibilityState==='hidden')return;
            const relative=(world,reference)=>multiply(inverse(reference?.world??matrixFromPose()),world);
            const frame={session,getPose(space,reference){const state=controllers[space?.hand];return state?.tracked?{transform:transform(relative(space.kind==='ray'?state.targetRayMatrix:state.matrix,reference)),emulatedPosition:false}:null;},
                getViewerPose(reference){
                    if(!head.tracked)return null;
                    const near=session.renderState.depthNear,far=session.renderState.depthFar;
                    const f=1.2,aspect=profile.eyeWidth/profile.eyeHeight;
                    const projection=new Float32Array([f/aspect,0,0,0,0,f,0,0,0,0,-(far+near)/(far-near),-1,0,0,-2*far*near/(far-near),0]);
                    return {transform:transform(relative(head.matrix,reference)),emulatedPosition:false,views:[-1,1].map((side,index)=>({eye:index?'right':'left',index,projectionMatrix:new Float32Array([f/(viewports[index].width/viewports[index].height),...projection.slice(1)]),transform:transform(relative(multiply(head.matrix,matrixFromPose({position:[side*profile.ipd/2,0,0]})),reference))}))};
                }};
            const callbacks=[...session.callbacks.values()];session.callbacks.clear();for(const callback of callbacks)callback(now,frame);
        },
        step(dt,grip,{tracking=true,head:position=[0,1.55,0],leftPressed=false}={}) {
            if(session&&!session.ended&&session.visibilityState!=='hidden'&&!session.callbacks.size)throw new Error('XR simulation frame loop stopped');
            api.frame({dt,head:{position,orientation:[0,0,0,1],tracked:tracking},controllers:{right:grip?{matrix:grip,tracked:tracking,pressed:false}:null,left:grip?{position:[-.3,1.1,-.2],tracked:tracking,pressed:leftPressed}:null}});
        },
        pixels(eye='stereo') {
            alive();if(!layer)throw new Error('No XR render layer');if(!['stereo','left','right'].includes(eye))throw new TypeError('Invalid XR eye');
            const region=eye==='stereo'?{x:0,y:0,width:framebufferWidth,height:framebufferHeight}:viewports[eye==='right'?1:0];
            const gl=layer.gl,{width,height}=region,bytes=new Uint8Array(width*height*4),old=gl.getParameter(gl.FRAMEBUFFER_BINDING);
            try {gl.bindFramebuffer(gl.FRAMEBUFFER,layer.framebuffer);gl.readPixels(region.x,region.y,width,height,gl.RGBA,gl.UNSIGNED_BYTE,bytes);}finally{gl.bindFramebuffer(gl.FRAMEBUFFER,old);}
            const data=new Uint8ClampedArray(bytes.length);for(let y=0;y<height;y++)data.set(bytes.subarray(y*width*4,(y+1)*width*4),(height-1-y)*width*4);
            return new ImageData(data,width,height);
        },
        recording(){return copy({version:1,profile,floorAvailable,frames});},
        wait(ms) {
            alive();if(!Number.isFinite(ms)||ms<0||ms>60000)throw new TypeError('XR wait must be between 0 and 60000 ms');
            return new Promise(resolve=>nativeTimeout(resolve,ms));
        },
        async replay(trace,{afterFrame,slow=false,onSession}={}) {
            alive();if(trace?.version!==1||!Array.isArray(trace.frames))throw new TypeError('Invalid XR recording');
            for(const key of ['hz','ipd','eyeWidth','eyeHeight'])if(trace.profile?.[key]!==profile[key])throw new TypeError(`XR recording profile mismatch: ${key}`);
            if((trace.floorAvailable??true)!==floorAvailable)throw new TypeError('XR recording floorAvailable mismatch');
            let previousIndex=0;
            for(const frame of trace.frames) {
                const currentIndex=frame.sessionIndex??0;
                if(!Number.isInteger(currentIndex)||currentIndex<previousIndex)throw new TypeError('Invalid XR recording sessionIndex');
                if(currentIndex!==previousIndex&&typeof onSession!=='function')throw new TypeError('XR recording crosses session boundaries; replay requires onSession(index) to end and reenter XR');
                if(!['visible','visible-blurred','hidden'].includes(frame.visibilityState??'visible'))throw new TypeError('Invalid XR recording visibilityState');
                previousIndex=currentIndex;
            }
            previousIndex=0;
            for(let index=0;index<trace.frames.length;index++) {
                const frame=trace.frames[index],currentIndex=frame.sessionIndex??0;
                if(currentIndex!==previousIndex) {
                    const previous=session;await onSession(currentIndex);
                    if(session===previous||!previous?.ended||!session||session.ended)throw new Error('XR replay onSession must end the previous session and create an active session');
                    previousIndex=currentIndex;
                }
                api.setVisibility(frame.visibilityState??'visible');
                if(frame.referenceReset)api.resetReferenceSpace();
                api.frame(frame);await afterFrame?.(index,frame);if(slow)await api.wait(frame.dt);
            }
        },
        setVisibility(value){alive();if(!['visible','visible-blurred','hidden'].includes(value))throw new TypeError('Invalid XR visibility');if(!session||session.ended)throw new Error('No active XR session');if(session.visibilityState!==value){session.visibilityState=value;event(session,'visibilitychange');}},
        resetReferenceSpace(){alive();if(!session||session.ended)throw new Error('No active XR session');referenceReset=true;for(const space of spaces)if(space.owner===session)event(space,'reset',{referenceSpace:space,transform:transform(matrixFromPose())});},
        denyNextRequest(){alive();deny=true;},
        deferNextRequest(){alive();if(deferred)throw new Error('XR request already deferred');let release;deferred=new Promise(resolve=>{release=()=>{pendingReleases.delete(release);resolve();};});pendingReleases.add(release);return release;},
        dispose(){if(disposed)return;disposed=true;for(const release of pendingReleases)release();void session?.end();timers.clear();for(const item of layers){item.gl.deleteFramebuffer(item.framebuffer);item.gl.deleteTexture(item.texture);item.gl.deleteRenderbuffer(item.depth);}layers.clear();for(const restore of restorations.reverse())restore();},
    };
    return api;
}
