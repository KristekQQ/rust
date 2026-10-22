import {capturePixels,assertForeground,assertChanged} from '../../tests/pixels.js';
import {loadEnemy} from './scene.js';
/** @param {ImageData} a @param {ImageData} b */
function changedPixels(a,b){
    let count=0;
    for(let i=0;i<a.data.length;i+=4){
        if(Math.abs(a.data[i]-b.data[i])+Math.abs(a.data[i+1]-b.data[i+1])+Math.abs(a.data[i+2]-b.data[i+2])>20)count++;
    }
    return count;
}
/** @param {number} count */
async function frames(count){for(let i=0;i<count;i++)await new Promise(requestAnimationFrame);}
/** @param {import('../../js/engine.js').Engine} engine @param {HTMLCanvasElement} canvas @param {()=>void} front @param {()=>void} rear */
export async function verifyEnemies(engine,canvas,front,rear){
    const check=(/** @type {unknown} */value,/** @type {string} */message)=>{if(!value)throw new Error(message);};
    for(const slug of ['drone-orbiter','blade-wraith','core-tank']){
        /** @type {ImageData | undefined} */let pink;
        for(const color of ['pink','blue','green','amber']){
            const enemy=await loadEnemy(engine.scene,slug,color);
            try{
                front();enemy.playAnimation('Idle',{loop:true}).pauseAnimation().seekAnimation(0);
                const neutral=await capturePixels(canvas);assertForeground(neutral,`${slug}/${color}`,1000);
                check(enemy.info.primitives>5,`${slug} modular geometry`);
                if(color==='pink'){
                    pink=neutral;
                    /** @type {{name:string,pixels:ImageData}[]} */const attackViews=[];
                    for(const name of ['Idle','Run','Attack','Attack2','Attack3','Attack4','Explode','Assemble']){
                        const clip=enemy.info.animations.find(a=>a.name===name);
                        check(clip&&clip.duration>1,`${slug}/${name} clip`);
                        const duration=clip?.duration??2;
                        if(name==='Run')check(Math.abs(duration-1.4)<.12,`${slug}/Run lasts about 1.4s`);
                        if(name.startsWith('Attack')&&name!=='Attack')check(duration>=3&&duration<=4.1,`${slug}/${name} lasts about 3-4s`);
                        if(name.startsWith('Attack'))engine.setOrbitView([5,3.4,10],[0,1.4,1.3]);else front();
                        enemy.playAnimation(name,{loop:name==='Idle'||name==='Run'}).pauseAnimation().seekAnimation(0);
                        const start=await capturePixels(canvas);
                        const sampleTime=name==='Run'?duration*.25:duration*.5;
                        enemy.seekAnimation(sampleTime);
                        const middle=await capturePixels(canvas);
                        assertChanged(start,middle,`${slug}/${name} actual animated pixels`,70);
                        if(name.startsWith('Attack'))attackViews.push({name,pixels:middle});
                        if(name.startsWith('Attack')){
                            enemy.seekAnimation(duration*.2);const prepare=await capturePixels(canvas);
                            assertChanged(prepare,middle,`${slug} wind-up and strike differ`,150);
                            enemy.seekAnimation(duration*.85);
                            assertChanged(middle,await capturePixels(canvas),`${slug} strike and recovery differ`,150);
                        }
                        if(slug!=='blade-wraith'&&(name==='Attack2'||name==='Attack3'||name==='Attack4')){
                            const shots=name==='Attack2'?[.35,.65]:name==='Attack3'?[.28,.62]:[.52,.60];
                            enemy.seekAnimation(duration*shots[0]);const beforeShot=await capturePixels(canvas);
                            enemy.seekAnimation(duration*shots[1]);const afterShot=await capturePixels(canvas);
                            assertChanged(beforeShot,afterShot,`${slug}/${name} projectile moves during authored firing interval`,35);
                        }
                        if(slug==='blade-wraith'&&(name==='Attack2'||name==='Attack3'||name==='Attack4')){
                            enemy.seekAnimation(duration*.35);const startStrike=await capturePixels(canvas);
                            enemy.seekAnimation(duration*.65);const endStrike=await capturePixels(canvas);
                            assertChanged(startStrike,endStrike,`${slug}/${name} weapon strike advances`,35);
                        }
                        if(name==='Run'){
                            enemy.playAnimation('Run',{loop:true}).seekAnimation(duration-.03);
                            await frames(12);
                            check(enemy.info.playback?.playing===true,`${slug}/Run loops after its endpoint`);
                            check((enemy.info.playback?.time??duration)<.4,`${slug}/Run wraps to its start`);
                        }
                        if(name==='Explode'||name==='Assemble'){
                            enemy.seekAnimation(duration);const end=await capturePixels(canvas);
                            if(name==='Explode')assertChanged(neutral,end,`${slug} destruction remains separated`,500);
                            else check(changedPixels(neutral,end)<500,`${slug} assembles into neutral pose`);
                            enemy.playAnimation(name,{loop:false}).seekAnimation(duration-.03);
                            await frames(12);
                            check(enemy.info.playback?.playing===false,`${slug}/${name} stops at endpoint`);
                            check((enemy.info.playback?.time??0)>=duration-.005,`${slug}/${name} holds endpoint`);
                        }
                    }
                    for(let i=0;i<attackViews.length;i++)for(let j=i+1;j<attackViews.length;j++)
                        assertChanged(attackViews[i].pixels,attackViews[j].pixels,`${slug}/${attackViews[i].name} differs from ${attackViews[j].name}`,35);
                    // Clip changes must reset dislocated limbs and temporary flash scales.
                    front();
                    enemy.playAnimation('Idle',{loop:true}).pauseAnimation().seekAnimation(0);
                    check(changedPixels(neutral,await capturePixels(canvas))<100,`${slug} clean reset after assemble`);
                    enemy.pauseAnimation(false);await frames(8);
                    check(enemy.info.playback?.playing&&enemy.info.playback.time>0,'Native animation clock');
                    enemy.pauseAnimation().seekAnimation(0);rear();
                    const back=await capturePixels(canvas);assertForeground(back,`${slug} rear`,1000);assertChanged(neutral,back,`${slug} full 3D`,500);
                }else if(pink)assertChanged(pink,neutral,`${slug}/${color} material variant`,70);
                check(!enemy.info.lastError,`${slug} animation runtime`);
            }finally{enemy.remove();}
        }
    }
}
