import {capturePixels,assertForeground,assertChanged} from '../../tests/pixels.js';

/** @param {import('../../js/engine.js').Engine} engine
 * @param {import('../../js/engine.js').ModelObject} robot
 * @param {HTMLCanvasElement} canvas
 * @param {()=>void} front
 * @param {()=>void} rear */
export async function verifyRobot(engine,robot,canvas,front,rear) {
    const check=(/** @type {unknown} */ value,/** @type {string} */ message)=>{if(!value)throw new Error(message);};
    try {
        check(robot.exists&&robot.info.primitives>15,'Imported articulated robot geometry');
        for(const name of ['Idle','Attack','Victory']){
            const animation=robot.info.animations.find(a=>a.name===name);
            check(animation&&animation.duration>1,`Missing ${name} animation`);
            front();robot.playAnimation(name,{loop:true}).pauseAnimation().seekAnimation(0);
            const neutral=await capturePixels(canvas);assertForeground(neutral,`${name} robot`,2000);
            robot.seekAnimation((animation?.duration??2)*.35);
            assertChanged(neutral,await capturePixels(canvas),`${name} changes actual model pixels`,100);
        }
        front();robot.playAnimation('Idle',{loop:true}).pauseAnimation(false);
        for(let i=0;i<12;i++)await new Promise(requestAnimationFrame);
        check(robot.info.playback?.playing&&robot.info.playback.time>0,'Rust advances animation automatically');
        robot.pauseAnimation().seekAnimation(0);
        const a=await capturePixels(canvas);
        let pink=0,cyan=0;
        for(let i=0;i<a.data.length;i+=4){const [r,g,b]=a.data.slice(i,i+3);if(r>130&&b>100&&r>g*1.4)pink++;if(g>110&&b>140&&b>r*1.4)cyan++;}
        check(pink>50&&cyan>50,`Both neon colors render (${pink}/${cyan})`);
        robot.setPosition([100,0,0]);assertChanged(a,await capturePixels(canvas),'Model is actual removable world geometry',2000);robot.setPosition([0,0,0]);
        rear();const b=await capturePixels(canvas);assertForeground(b,'Actual rear geometry',2000);assertChanged(a,b,'Distinct front and rear views',1000);
        check(!robot.info.lastError,'No animation error');
    } finally {robot.setPosition([0,0,0]);front();robot.playAnimation('Idle',{loop:true});}
}
