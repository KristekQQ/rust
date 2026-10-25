import {capturePixels,assertForeground,assertChanged} from '../../tests/pixels.js';
/** @param {import('../../js/engine.js').Engine} engine @param {ReturnType<typeof import('./scene.js').createScene>} world @param {HTMLCanvasElement} canvas */
export async function verify(engine,world,canvas){
    const result=document.querySelector('#result');if(!result)return;
    const wait=async(n=1)=>{for(let i=0;i<n;i++)await new Promise(requestAnimationFrame);};
    /** @param {unknown} value @param {string} message @returns {asserts value} */
    function assert(value,message){if(!value)throw new Error(message);}
    try{
        engine.scene.tableTennisCommand('match');await wait(3);
        assert(engine.scene.tableTennisState?.phase==='ready','native reset');
        const a=await capturePixels(canvas);assertForeground(a,'Table tennis',2000);
        // Real pointer commands pass through the same DOM path as human input.
        const rect=canvas.getBoundingClientRect();
        canvas.dispatchEvent(new PointerEvent('pointermove',{clientX:rect.left+rect.width*.25,clientY:rect.top+rect.height*.5}));await wait(3);
        const left=world.player.meshInfo.position;
        canvas.dispatchEvent(new PointerEvent('pointermove',{clientX:rect.left+rect.width*.75,clientY:rect.top+rect.height*.5}));await wait(3);
        const right=world.player.meshInfo.position;
        assert(left[0]>.4&&right[0]<-.4,'mouse moves native paddle across table');
        assertChanged(a,await capturePixels(canvas),'Paddle movement pixels',40);
        const projected=engine.worldToScreen([.3,.76,-2.2]);
        canvas.dispatchEvent(new PointerEvent('pointerdown',{buttons:1,clientX:rect.left+rect.width*.5,clientY:rect.top+rect.height*.5}));
        window.dispatchEvent(new MouseEvent('mousemove',{buttons:1,movementX:150,movementY:60}));
        window.dispatchEvent(new PointerEvent('pointerup'));
        await wait(3);
        const afterDrag=engine.worldToScreen([.3,.76,-2.2]);
        assert(projected&&afterDrag&&Math.abs(projected[0]-afterDrag[0])<.001&&Math.abs(projected[1]-afterDrag[1])<.001,'dragging paddle keeps camera fixed');
        engine.scene.tableTennisCommand('reset');
        document.querySelector('#serve')?.dispatchEvent(new MouseEvent('click'));
        await wait(4);
        const first=engine.scene.tableTennisState;
        assert(first && first.phase==='rally'&&first.ballVelocity.some(v=>Math.abs(v)>1),'native serve');
        await wait(15);
        const next=engine.scene.tableTennisState;assert(next,'native state');
        assert(Math.abs(next.ballPosition[2]-first.ballPosition[2])>.1,'ball advances in Rust');
        const count=engine.scene.objectCount;
        await new Promise(resolve=>setTimeout(resolve,4200));
        const game=engine.scene.tableTennisState;assert(game,'native state');
        assert(game.score[0]+game.score[1]>0,'unreturned shot awards a native point');
        assert(engine.scene.objectCount===count,'bounded pooled scene');
        engine.scene.tableTennisCommand('reset');await wait(3);
        assert(engine.scene.tableTennisState?.score.every(v=>v===0),'reset clears both scores');
        result.textContent=`PASS: ${engine.renderer_backend()} · table/paddle pixels, pointer input, native serve/movement/scoring/reset, bounded objects`;
    }catch(error){result.textContent=`FAIL: ${error instanceof Error?error.message:error}`;console.error(error);}
}
