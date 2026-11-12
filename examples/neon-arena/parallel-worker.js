// Platform shim: the shared native game's complete simulation stays in Rust/WASM.
import {createSimulation} from './parallel-runtime.js';
let game,observer;
const now=()=>performance.now();
self.onmessage=async event=>{
    const message=event.data;
    try{
        if(message.type==='init'){
            game=await createSimulation(message.assets,message.scores);
            const started=now(),bytes=game.initial();
            if(message.observerUrl){
                // Optional observer stays on this CPU worker; transport failure never stops gameplay.
                try{const {ScenePublisher}=await import('../../tools/scene-observer/publisher.js');const refs=JSON.parse(game.refs());observer=new ScenePublisher(message.observerUrl,{objectIds:[refs.leftSword,refs.sword]});}catch(error){console.warn('Observer unavailable',error);}
            }
            observer?.publish(bytes,JSON.parse(game.state()));
            self.postMessage({type:'ready',bytes:bytes.buffer,refs:JSON.parse(game.refs()),state:JSON.parse(game.state()),workerMs:now()-started},[bytes.buffer]);
        }else if(message.type==='tick'){
            if(!game)throw new Error('Worker not initialized');
            const started=now(),bytes=game.tick(new Float32Array(message.frames,0,message.count*88));
            const state=JSON.parse(game.state()),haptics=game.take_haptics();
            observer?.publish(bytes,state,message.sampledAt);
            self.postMessage({type:'frame',bytes:bytes.buffer,frames:message.frames,workerMs:now()-started,state,haptics,sent:message.sent},[bytes.buffer,message.frames]);
        }else if(message.type==='immersive'){game.set_immersive(message.value);
        }else if(message.type==='scores'){game.restore_scores(JSON.stringify(message.scores));
        }else if(message.type==='dispose'){observer?.close();game?.free();game=null;self.close();}
    }catch(error){self.postMessage({type:'error',error:String(error)});}
};
