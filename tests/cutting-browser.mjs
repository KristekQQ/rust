import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';

export async function verifyCutting(page){
    await page.waitForFunction(()=>typeof window.render_game_to_text==='function');
    const read=()=>page.evaluate(()=>JSON.parse(window.render_game_to_text()));
    const frames=()=>page.evaluate(async()=>{for(let i=0;i<5;i++)await new Promise(requestAnimationFrame);});
    const settle=async()=>{await page.waitForFunction(()=>!JSON.parse(window.render_game_to_text()).busy);await frames();};
    const set=(selector,value)=>page.$eval(selector,(el,v)=>{el.value=String(v);el.dispatchEvent(new Event('input'));},value);
    const capture=()=>page.evaluate(async()=>{
        const {capturePixels}=await import('../../tests/pixels.js');const image=await capturePixels(document.querySelector('#gpu-canvas'));
        let red=0,green=0,cream=0,hash=0;
        for(let i=0;i<image.data.length;i+=4){const r=image.data[i],g=image.data[i+1],b=image.data[i+2];if(r>80&&r>2*g&&r>2*b)red++;if(g>80&&g>r*1.6&&g>b*1.6)green++;if(r>190&&g>150&&b>60&&b<r*.9)cream++;hash=Math.imul(hash^r,16777619);}
        return {red,green,cream,hash};
    });
    const cameraDistance=(a,b)=>Math.hypot(...a.position.map((v,i)=>v-b.position[i]));
    const screen=async point=>{
        const uv=await page.evaluate(async p=>(await import('./main.js')).engine.worldToScreen(p),point);
        assert(uv,'World target is visible');const box=await(await page.$('#gpu-canvas')).boundingBox();return [box.x+uv[0]*box.width,box.y+uv[1]*box.height];
    };
    const backend=await page.evaluate(()=>document.documentElement.dataset.rendererBackend);
    const shot=name=>page.screenshot({path:fileURLToPath(new URL(`../output/verification/cutting-${name}-${backend}.png`,import.meta.url))});
    await frames();let state=await read();const initialCamera=state.camera;
    assert.equal(state.pieces.length,5,'Five authored apples');assert.equal(state.mode,'held');assert(state.handVisible&&state.guideVisible);
    const stats=()=>page.evaluate(async()=>(await import('./main.js')).engine.debugStats());
    const hudPixels=()=>page.evaluate(async()=>{
        const {capturePixels}=await import('../../tests/pixels.js');const image=await capturePixels(document.querySelector('#gpu-canvas'));let bright=0;
        for(let y=12;y<85;y++)for(let x=12;x<320;x++){const i=(y*image.width+x)*4;if(image.data[i]>190&&image.data[i+1]>210&&image.data[i+2]>220)bright++;}
        return bright;
    });
    assert(!(await stats()).enabled,'HUD starts disabled');const hudBefore=await hudPixels();
    await page.keyboard.press('F3');await page.waitForFunction(async()=>(await import('./main.js')).engine.debugStats().hudFrames>3);
    const drawStats=await stats();assert(drawStats.mainDrawCalls>0&&drawStats.drawCalls>=drawStats.mainDrawCalls+1,'F3 exposes actual draw-call counters including HUD');
    assert(await hudPixels()>hudBefore+100,'F3 draws actual Rust HUD glyphs');await shot('hud');
    await page.keyboard.press('F3');const off=await stats();await frames();const offLater=await stats();assert(!offLater.enabled);assert.equal(offLater.hudFrames,off.hudFrames,'Disabled HUD does no draws');assert.equal(offLater.hudRefreshes,off.hudRefreshes);
    assert(await hudPixels()<hudBefore+50,'F3 hides glyphs');
    const surfaceGuide=async()=>{
        await page.click('#guide');await frames();
        const skin=await page.evaluate(async()=>{
            const {capturePixels}=await import('../../tests/pixels.js');const image=await capturePixels(document.querySelector('#gpu-canvas'));const mask=[];
            for(let i=0;i<image.data.length;i+=4)if(image.data[i]>100&&image.data[i]>3*image.data[i+1]&&image.data[i]>3*image.data[i+2])mask.push(i);
            return mask;
        });
        await page.click('#guide');await frames();
        return page.evaluate(async mask=>{
            const {capturePixels}=await import('../../tests/pixels.js');const image=await capturePixels(document.querySelector('#gpu-canvas'));let n=0;
            for(const i of mask)if(image.data[i+1]>160&&image.data[i+1]>image.data[i]*1.6&&image.data[i+1]>image.data[i+2]*1.6)n++;
            return n;
        },skin);
    };
    assert(await surfaceGuide()>25,'Guide overlays pixels that were apple skin, not just the board');
    await set('#tilt',15);await set('#angle',25);await frames();assert(await surfaceGuide()>25,'Tilted guide also lies on apple skin');
    await shot('surface-guide');await set('#tilt',0);await set('#angle',0);await frames();
    const total=state.pieces.reduce((sum,p)=>sum+p.volume,0),before=await capture();assert(before.red>500&&before.green>20,'Apple skin and green guide/leaf pixels');
    await page.click('#cut');await settle();state=await read();assert.equal(state.pieces.length,6,'Only central apple is split');assert.equal(state.apples.filter(a=>a.intact).length,4);
    const after=await capture();assert(after.cream>before.cream+50,'Exposed flesh pixels');assert.notEqual(after.hash,before.hash);
    await set('#angle',90);await page.focus('#angle');await page.keyboard.press('Space');await settle();state=await read();assert(state.pieces.length>=8,'Repeated cutting creates additional pieces');
    assert(Math.abs(state.pieces.reduce((sum,p)=>sum+p.volume,0)-total)<total*.001,'Total volume across all apples');assert(state.pieces.every(p=>p.closed),'Closed fragments');
    assert(state.pieces.filter(p=>!state.apples.some(a=>a.id===p.id)).every(p=>Math.abs(p.bottom-.205)<.003),'Fragment support matches board');await shot('multi');
    await page.keyboard.press('r');assert.equal((await read()).pieces.length,5);
    // RMB, wheel and WASD must never slash an apple.
    let box=await(await page.$('#gpu-canvas')).boundingBox();
    await page.mouse.move(box.width*.4,box.height*.47);await page.mouse.down({button:'right'});await page.mouse.move(box.width*.5,box.height*.53,{steps:8});await page.mouse.up({button:'right'});await frames();
    state=await read();assert(cameraDistance(state.camera,initialCamera)>1,'RMB changes camera');assert.equal(state.cuts,0);
    const orbited=state.camera;await page.mouse.wheel({deltaY:180});await frames();assert(cameraDistance((await read()).camera,orbited)>.5,'Wheel zoom changes distance');
    const panned=(await read()).camera;await page.keyboard.down('w');
    await page.waitForFunction(before=>Math.hypot(...JSON.parse(window.render_game_to_text()).camera.position.map((v,i)=>v-before.position[i]))>.1,{polling:'raf'},panned);
    await page.keyboard.up('w');await frames();assert(cameraDistance((await read()).camera,panned)>.1,'WASD moves through space');
    const preResize=(await read()).camera;await page.setViewport({width:840,height:620,deviceScaleFactor:1});await frames();assert(cameraDistance((await read()).camera,preResize)<.001,'Resize preserves user camera');await page.setViewport({width:960,height:640,deviceScaleFactor:1});await frames();
    await page.click('#mode-camera');box=await(await page.$('#gpu-canvas')).boundingBox();const oldCamera=(await read()).camera;
    await page.mouse.move(box.width*.4,box.height*.5);await page.mouse.down();await page.mouse.move(box.width*.45,box.height*.5,{steps:6});await page.mouse.up();await frames();assert(cameraDistance((await read()).camera,oldCamera)>.1,'Camera mode primary drag');assert.equal((await read()).cuts,0);
    await page.click('#view-reset');await frames();assert(cameraDistance((await read()).camera,initialCamera)<.001,'Home camera');await page.click('#mode-held');
    // Trusted pointer: aim at two different apples using the rendered camera.
    state=await read();const first=state.apples[0];let point=await screen(first.center);await page.mouse.move(...point);await frames();state=await read();
    assert(Math.abs(state.settings.x-first.center[0])<.03&&Math.abs(state.settings.z-first.center[2])<.03,'Held sword follows cursor in board space');await shot('held');
    await page.mouse.down();await page.mouse.up();await settle();assert(!(await read()).apples[0].intact,'Held click cuts aimed apple');
    const second=(await read()).apples[3];point=await screen(second.center);await page.mouse.move(...point);await frames();await page.mouse.down();await page.mouse.up();await settle();state=await read();assert(!state.apples[3].intact&&state.pieces.length>=7,'Another apple cuts independently');
    await set('#offset',4);await set('#depth',1.3);await frames();
    const greenOn=(await capture()).green;await page.click('#guide');await frames();assert(!(await read()).guideVisible);const greenOff=(await capture()).green;assert(greenOn>greenOff+10,`Guide pixels disappear (${greenOn} -> ${greenOff})`);await page.click('#guide');await frames();
    // Planned stroke keeps camera still, including secondary-touch cancellation.
    await page.click('#mode-plan');state=await read();assert(!state.handVisible);const cameraBefore=state.camera;
    const target=state.apples[2].center,start=await screen([target[0],1.15,target[2]-1.3]),end=await screen([target[0],1.15,target[2]+1.3]);
    await page.mouse.move(...start);await page.mouse.down();
    await page.evaluate(()=>{const c=document.querySelector('#gpu-canvas');c.dispatchEvent(new PointerEvent('pointerdown',{pointerId:999,isPrimary:false,button:0,bubbles:true}));c.dispatchEvent(new PointerEvent('lostpointercapture',{pointerId:999,bubbles:true}));});
    assert.equal((await read()).dragging,'plan');await page.mouse.move(...end,{steps:12});await page.mouse.up();await settle();state=await read();assert(!state.apples[2].intact,'Planned swipe selects third apple');assert(cameraDistance(state.camera,cameraBefore)<.001,'Sword stroke cannot orbit camera');
    await page.click('#reset');await set('#angle',25);await set('#offset',.15);await set('#depth',0);await set('#tilt',15);await page.click('#cut');await settle();state=await read();assert(state.pieces.length>5,'Tilted guide produces cut');assert(state.board);
    if(backend==='BrowserWebGpu'){
        await page.click('#reset');await page.select('#compute','gpu');await set('#angle',25);await set('#offset',.17);await set('#tilt',15);
        await page.click('#cut');await settle();state=await read();assert.equal(state.lastCut.execution,'webgpu-compute');assert(state.lastCut.gpuMeshes>0&&state.pieces.length>5,'Explicit example compute mode consumes GPU output');await shot('compute');
        await page.click('#reset');await page.select('#compute','gpu-resident');await set('#angle',25);await set('#offset',.17);await set('#tilt',15);
        await page.click('#cut');await settle();state=await read();assert.equal(state.lastCut.execution,'webgpu-resident');assert(state.pieces.some(p=>p.resident)&&state.pieces.filter(p=>p.resident).every(p=>!p.cpuMaterialized),'Example renders and animates GPU buffers without CPU geometry');assert((await capture()).cream>80,'Resident caps actually render');await shot('resident');
        await page.select('#compute','auto');
    }
    await page.click('#reset');await set('#offset',4.3);await set('#depth',2.8);await page.click('#cut');await settle();assert.equal((await read()).pieces.length,5);assert.equal((await read()).cuts,0,'Miss does not score');
    await page.click('#reset');await page.click('#mode-held');await page.click('#view-reset');await frames();
    box=await(await page.$('#gpu-canvas')).boundingBox();await page.mouse.move(box.width*.5,box.height*.015);await page.mouse.down();await page.mouse.up();
    state=await read();assert(!state.busy&&state.cuts===0,'Off-board/sky click cannot slash the previous valid aim');
    return `PASS: ${backend} F3 HUD glyphs/on/off, surface guide on apple skin (straight/tilted), five apples, held click/planned swipe, independent fruit cuts, RMB/primary camera, wheel/WASD, resize/home, volume/caps/landing, reset/miss`;
}
