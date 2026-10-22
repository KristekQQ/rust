import {capturePixels,assertChanged,assertForeground} from '../../tests/pixels.js';
/** @param {import('../../js/engine.d.ts').Engine} engine
 * @param {HTMLCanvasElement} canvas
 * @param {ReturnType<typeof import('./scene.js').createScene> extends Promise<infer P> ? P : never} preview
 * @param {{strength:number,radius:number}} glow */
export async function verifyFont(engine,canvas,preview,glow) {
    const original=engine.cameraViewState();
    /** @type {import('../../js/engine.d.ts').Text3D|null} */
    let probe=null;
    /** @type {import('../../js/engine.d.ts').Font3D|null} */
    let temporaryFont=null;
    const assert=(/** @type {unknown} */ value,/** @type {string} */ message)=>{if(!value)throw new Error(message);};
    const frames=async()=>{for(let i=0;i<8;i++)await new Promise(requestAnimationFrame);};
    try{
        const text=document.querySelector('#text'),color=document.querySelector('#color'),selector=document.querySelector('#font');
        if(!(text instanceof HTMLInputElement)||!(color instanceof HTMLSelectElement)||!(selector instanceof HTMLSelectElement))throw new Error('Chybí ovládání fontů.');
        const oldText=text.value,oldRendered=preview.text,oldColor=color.value,oldFont=preview.selected;
        const oldError=text.validity.customError?text.validationMessage:'';
        try{
            text.value='MODELAR';text.dispatchEvent(new Event('input',{bubbles:true}));
            assert(preview.text==='MODELAR'&&text.validity.valid,'Editace Rust textu nefunguje.');
            text.value='Č';text.dispatchEvent(new Event('input',{bubbles:true}));
            assert(!text.validity.valid&&preview.text==='MODELAR','Chybějící znak nebyl atomicky odmítnutý.');
            color.value='magenta';color.dispatchEvent(new Event('change',{bubbles:true}));
            assert(preview.objects[0].info.color[0]===1,'Přepínání barvy nefunguje.');
            selector.value='pixel';selector.dispatchEvent(new Event('change',{bubbles:true}));
            assert(preview.font.info.name==='pixel'&&preview.text==='MODELAR','Vestavěný pixel font nefunguje.');
            await frames(); // worldToScreen reports the most recently rendered camera.
            const half=preview.objects[0].info.width/2;
            for(const x of [-half,half]){
                const p=engine.worldToScreen([x,1.7,0]);
                assert(p&&p[0]>.01&&p[0]<.99,'Širší pixelový text se nevejde do náhledu.');
            }
        }finally{
            // Restore the original font first; the original text might be lowercase.
            preview.setFont(oldFont,4);preview.setText(oldRendered);
            preview.setColor(oldColor==='magenta'?'magenta':'cyan');
            selector.value=oldFont;text.value=oldText;text.setCustomValidity(oldError);color.value=oldColor;
        }
        // Import has already validated the entire asset in Rust, including all185 glyphs.
        const font=preview.fonts.cynatar;
        assert(font.info.characters.length===185,'Neúplný font Cynatar.');
        const data=await (await fetch('../../assets/fonts/cynatar/glyphs.json')).json();
        const glyph=data.glyphs.O;
        const xs=glyph.positions.filter((/** @type {number} */ _,/** @type {number} */ i)=>i%3===0);
        const ys=glyph.positions.filter((/** @type {number} */ _,/** @type {number} */ i)=>i%3===1);
        const cx=(Math.min(...xs)+Math.max(...xs))/2,cy=(Math.min(...ys)+Math.max(...ys))/2;
        const position=/** @type {const} */([12+cx*2,1.5-cy*2,0]);
        probe=font.createText('O',{align:'left',color:[.02,.8,1],sideIntensity:.32})
            .setEmission(4).setScale([-2,2,2]).setPosition(position);
        assert(Math.abs(probe.info.depth-.13)<1e-5,'Nesprávná 3D hloubka.');
        engine.setOrbitView([12,1.5,3],[12,1.5,0]);engine.setNeonGlow(glow);
        const on=await capturePixels(canvas);assertForeground(on,'Cynatar O',100);
        engine.setNeonGlow({strength:0});const off=await capturePixels(canvas);
        assertChanged(off,on,'Geometrická neonová záře',80);
        const hole=engine.worldToScreen([12,1.5,.13]);
        if(!hole)throw new Error('Otvor O je mimo záběr.');
        const hx=Math.round(hole[0]*off.width),hy=Math.round(hole[1]*off.height);
        let holePeak=0,cyan=0;
        for(let y=-2;y<=2;y++)for(let x=-2;x<=2;x++){
            const i=((hy+y)*off.width+hx+x)*4;holePeak=Math.max(holePeak,off.data[i],off.data[i+1],off.data[i+2]);
        }
        assert(holePeak<60,`Otvor O je vyplněný (${holePeak}).`);
        for(let i=0;i<off.data.length;i+=4)if(off.data[i+1]>off.data[i]+35&&off.data[i+2]>off.data[i]+35)cyan++;
        assert(cyan>80,'Chybí azurová barva.');
        engine.setOrbitView([15,1.5,.05],[12,1.5,0]);const side=await capturePixels(canvas);let sideCyan=0;
        for(let i=0;i<side.data.length;i+=4)if(side.data[i+1]>side.data[i]+35&&side.data[i+2]>side.data[i]+35)sideCyan++;
        assert(sideCyan>80,'Bok písmene nesvítí azurově.');
        engine.setOrbitView([13.9,1.8,2.8],[12,1.5,0]);
        const oblique=await capturePixels(canvas);assertChanged(off,oblique,'Skutečná perspektiva',100);
        probe.setOptions({color:[1,.035,.65]});
        const magenta=await capturePixels(canvas);let colored=0;
        for(let i=0;i<magenta.data.length;i+=4)if(magenta.data[i]>magenta.data[i+1]+35&&magenta.data[i+2]>magenta.data[i+1]+35)colored++;
        assert(colored>80,'Chybí purpurová barva.');
        assertChanged(oblique,magenta,'Změna stylu na Rust text rootu',80);
        probe.setRotation([0,.35,0]).setScale([-1.4,2.3,.8]);
        assertChanged(magenta,await capturePixels(canvas),'Kořenová rotace a neuniformní měřítko',100);
        probe.remove();probe=null;

        // Isolate a run of repeated letters: one visible glyph mesh, 32 GPU instances.
        const pixel=preview.fonts.pixel;
        probe=pixel.createText('A'.repeat(32),{size:.16,maxWidth:2,color:[.02,.8,1],position:[12,1.25,0]})
            .setScale([-1,1,1]).setEmission(4);
        engine.setOrbitView([12,1.5,3],[12,1.5,0]);await frames();
        const before=engine.debugStats().font3d,render=engine.renderer_stats();
        assert(probe.info.glyphCount===32&&before.instanceGeometryBytes===0,'Glyph instance owns duplicate GPU geometry.');
        assert(render.visible>=20&&render.drawCalls===1,`Očekáváno instancování do jednoho draw: ${JSON.stringify(render)}`);
        probe.setText('A'.repeat(32));await frames();
        const after=engine.debugStats().font3d;
        assert(before.runtime.layoutRebuilds===after.runtime.layoutRebuilds&&before.runtime.syncUpdates===after.runtime.syncUpdates,
            'Nezměněný text znovu počítá layout nebo transformace.');
        assert(before.geometryBytes===after.geometryBytes,'Nezměněný text znovu alokuje GPU geometrii.');
        document.querySelector('#status')?.setAttribute('data-instancing','32 glyphů / 1 draw');
        probe.remove();probe=null;

        // Released font handles retain existing labels; their last label frees the cache.
        const count=engine.debugStats().font3d.runtime.fonts;
        temporaryFont=engine.scene.registerFont3D({...data,glyphs:{O:glyph}});
        probe=temporaryFont.createText('O',{position:[30,0,0]});temporaryFont.remove();
        let rejected=false;try{temporaryFont.createText('O');}catch{rejected=true;}
        assert(rejected,'Odstraněný font vytváří nové texty.');
        probe.setText('OO');probe.remove();probe=null;temporaryFont=null;
        assert(engine.debugStats().font3d.runtime.fonts===count,'Poslední text neuvolnil font.');
        engine.setNeonGlow(glow);
        const distance=Math.hypot(original.position[0],original.position[2]),angle=25*Math.PI/180;
        engine.setOrbitView([distance*Math.sin(angle),1.8,distance*Math.cos(angle)],[0,1.5,0]);
        const frame=await capturePixels(canvas);
        let evidence=document.querySelector('#font-evidence');
        if(!(evidence instanceof HTMLCanvasElement)){evidence=document.createElement('canvas');evidence.id='font-evidence';document.body.append(evidence);}
        if(evidence instanceof HTMLCanvasElement){evidence.style.display='none';evidence.width=frame.width;evidence.height=frame.height;evidence.getContext('2d')?.putImageData(frame,0,0);}
    }finally{
        probe?.remove();if(temporaryFont?.info.registered)temporaryFont.remove();engine.setNeonGlow(glow);
        if(original.orbitTarget)engine.setOrbitView(original.position,original.orbitTarget);
    }
}
