// Demo host input only. Physics, waves, fruit and cuts run in the shared Rust game.
import {createSimulation} from '../../examples/neon-arena/parallel-runtime.js';
import {ScenePublisher} from './publisher.js';
import {matrixFromPose} from '../../tests/xr/poses.js';
let game,publisher,timer,elapsed=0,paused=false,previous=0;
const pose=(position,pitch=0)=>matrixFromPose({position,orientation:[Math.sin(pitch/2),0,0,Math.cos(pitch/2)]});
const headInput=t=>({position:[Math.sin(t*.45)*.16,1.6+Math.sin(t*.7)*.04,Math.cos(t*.45)*.07],rotation:[0,Math.sin(t*.8)*.65,0]});
self.onmessage=async({data})=>{
    try{
        if(data.type==='start'){
            clearInterval(timer);publisher?.close();game?.free();
            const base=new URL('../../examples/neon-arena/parallel-assets/',import.meta.url);
            const manifest=await(await fetch(new URL('scene.json',base))).json();
            // This demo authors an empty round: only tracked equipment is exercised.
            for(const cue of manifest.runtime.minutePlan.cues)cue.recipes=[];
            const overrides=new Map([['scene.json',new TextEncoder().encode(JSON.stringify(manifest))]]);
            game=await createSimulation(base.href,[],overrides);elapsed=0;paused=false;
            const refs=JSON.parse(game.refs());
            publisher=new ScenePublisher(data.url,{mode:'demo',objectIds:[refs.leftSword,refs.sword],onStatus:status=>self.postMessage(status)});
            publisher.publish(game.initial(),{head:headInput(0)});previous=performance.now();
            timer=setInterval(()=>{
                if(paused){previous=performance.now();return;}
                try{
                    const now=performance.now(),dt=Math.min((now-previous)/1000,.05);previous=now;elapsed+=dt;
                    const frame=new Float32Array(88);frame[0]=dt;frame[1]=7;
                    const head=headInput(elapsed),yaw=head.rotation[1];
                    frame.set(matrixFromPose({position:head.position,orientation:[0,Math.sin(yaw/2),0,Math.cos(yaw/2)]}),8);
                    frame.set(pose([-.45+Math.sin(elapsed*2.9)*.3,1.25+Math.cos(elapsed*3.4)*.25,-.35],.45+Math.sin(elapsed*2.1)*.4),24);
                    frame.set(pose([.45+Math.sin(elapsed*3.1)*.5,1.1+Math.cos(elapsed*3.7)*.35,-.45],.4+Math.cos(elapsed*2.5)*.5),40);
                    const bytes=game.tick(frame);
                    publisher.publish(bytes,{head},performance.timeOrigin+now);
                }catch(error){clearInterval(timer);self.postMessage({error:String(error)});}
            },1000/30);
        }else if(data.type==='pause'){paused=data.value;}
        else if(data.type==='stop'){clearInterval(timer);publisher?.close();game?.free();game=null;}
    }catch(error){self.postMessage({error:String(error)});}
};
