/** @typedef {import('../../js/engine.d.ts').Font3D} Font3D */
/** @typedef {import('../../js/engine.d.ts').Text3D} Text3D */
export const COLORS = /** @type {const} */ ({cyan:[.02,.8,1],magenta:[1,.035,.65]});

/** Explicit application recipe. Rust owns glyph caches, layout and child transforms.
 * @param {import('../../js/engine.d.ts').Scene} scene */
export async function createScene(scene) {
    const cynatar=await scene.loadFont3D(new URL('../../assets/fonts/cynatar/glyphs.json',import.meta.url));
    const pixel=scene.builtinFont3D('pixel');
    const fonts={cynatar,pixel};
    /** @type {keyof typeof fonts} */
    let selected='cynatar';
    /** @type {[Text3D,Text3D]} */
    let texts;
    /** @param {Font3D} font @param {string} text @param {import('../../js/engine.d.ts').Vector3} color
     * @param {number} emission */
    const build=(font,text,color,emission)=>{
        const headline=font.createText(text,{size:1.05,maxWidth:6.6,color,sideIntensity:.32,position:[0,1.7,0]})
            .setScale([-1,1,1]).setEmission(emission);
        try {
            const numbers=font.createText('0123456789',{size:.65,maxWidth:6.6,color,sideIntensity:.32,position:[0,.45,0]})
                .setScale([-1,1,1]).setEmission(emission);
            return /** @type {[Text3D,Text3D]} */([headline,numbers]);
        }catch(error){headline.remove();throw error;}
    };
    try{texts=build(cynatar,'CYNATAR',COLORS.cyan,4);}catch(error){cynatar.remove();pixel.remove();throw error;}
    return {
        fonts,
        get font(){return fonts[selected];},
        get selected(){return selected;},
        get objects(){return texts;},
        get text(){return texts[0].info.text;},
        /** @param {string} value */
        setText(value){texts[0].setText(value);},
        /** @param {keyof typeof COLORS} value */
        setColor(value){texts.forEach(text=>text.setOptions({color:COLORS[value]}));},
        /** @param {number} value */
        setEmission(value){texts.forEach(text=>text.setEmission(value));},
        /** @param {keyof typeof fonts} value @param {number} emission */
        setFont(value,emission){
            const next=build(fonts[value],texts[0].info.text,texts[0].info.color,emission);
            texts.forEach(text=>text.remove());texts=next;selected=value;
        },
        /** @param {string} char @param {keyof typeof COLORS} [tint] */
        createGlyph(char,tint='cyan') {return fonts[selected].createText(char,{align:'left',color:COLORS[tint],sideIntensity:.32}).setEmission(4);},
        dispose(){texts.forEach(text=>text.remove());cynatar.remove();pixel.remove();},
    };
}
