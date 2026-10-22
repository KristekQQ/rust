/** Authoring recipes only. Rust integrates all projectile motion and mesh cuts. */
export const FRUIT_KINDS = Object.freeze(['cherry','lime','lemon','orange','banana','plum','pear','grapes','watermelon']);
export const FORMATIONS = Object.freeze(['line','arc','cross','rain']);
export const DEFAULT_WAVE = Object.freeze({count:10,intervalMs:3000,formation:'arc',fruit:'mixed',automatic:true,program:'minute',fruitScale:.09,width:.74,height:1.28,depth:-1.12,curve:.2,slope:.16,lift:.6,gravity:.8,lifetime:2.6});
/** @typedef {{count:number,intervalMs:number,formation:string,fruit:string,automatic:boolean,program:'minute'|'practice',fruitScale:number,width:number,height:number,depth:number,curve:number,slope:number,lift:number,gravity:number,lifetime:number}} WaveConfig */
export const WAVE_RANGES = Object.freeze({fruitScale:[.03,.105],width:[.45,.9],height:[1.1,1.6],depth:[-1.4,-.9],curve:[0,.25],slope:[0,.2],lift:[.2,.8],gravity:[.5,1.6],lifetime:[1.5,3.5]});
/** @param {unknown} input @param {WaveConfig} [base] @returns {WaveConfig} */
export function validateWave(input,base=DEFAULT_WAVE) {
    if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('Očekávám nastavení vlny.');
    const patch=/** @type {Record<string,unknown>} */(input);
    for(const key of Object.keys(patch))if(!Object.hasOwn(DEFAULT_WAVE,key))throw new Error(`Neznámé nastavení: ${key}`);
    const value={...base,...patch};
    if(!Number.isInteger(value.count)||value.count<1||value.count>10)throw new Error('Počet musí být 1–10.');
    if(!Number.isInteger(value.intervalMs)||value.intervalMs<1800||value.intervalMs>6000)throw new Error('Pauza musí být 1800–6000 ms.');
    if(!FORMATIONS.includes(value.formation))throw new Error('Neznámá formace.');
    if(value.fruit!=='mixed'&&!FRUIT_KINDS.includes(value.fruit))throw new Error('Neznámé ovoce.');
    if(!['minute','practice'].includes(value.program))throw new Error('Neznámý herní program.');
    if(typeof value.automatic!=='boolean')throw new Error('Automatické vlny musí být true/false.');
    for(const [key,[min,max]] of Object.entries(WAVE_RANGES)){
        const number=/** @type {Record<string,unknown>} */(value)[key];
        if(typeof number!=='number'||!Number.isFinite(number)||number<min||number>max)throw new Error(`${key}: povolený rozsah ${min} až ${max}.`);
    }
    return value;
}
/** @param {WaveConfig} config @param {number} waveIndex */
export function planWave(config,waveIndex=0) {
    return Array.from({length:config.count},(_,i)=>{
        const row=Math.floor(i/5),column=i%5;
        const rowSize=Math.min(5,config.count-row*5);
        const t=rowSize===1?0:column/(rowSize-1)*2-1;
        let x=t*.68,y=1.15+row*.32,z=-1.12-row*.07;
        if(config.formation==='line'){x=(config.count===1?0:i/(config.count-1)*2-1)*.74;y=1.3;z=-1.12;}
        if(config.formation==='arc'){
            // One continuous reachable stroke. Four authored variants alternate
            // its slope and bend instead of repeating one horizontal swipe.
            const along=config.count===1?0:i/(config.count-1)*2-1;
            const direction=waveIndex%2===0?1:-1;
            const bend=Math.floor(waveIndex/2)%2===0?1:-1;
            x=along*.74;
            y=config.count===1?1.3:1.28+direction*config.slope*along+bend*config.curve*(1-along*along);
            z=-1.12+.04*(1-along*along);
        }
        if(config.formation==='cross'){z=-1.02-row*.25;y=1.32+(row===0?1:-1)*t*.24+(row===0?-.07:.07);}
        if(config.formation==='rain'){y=1.35+row*.3+(column%2)*.1;z=-1.05-row*.08;}
        const fruit=config.fruit==='mixed'?FRUIT_KINDS[(waveIndex+Math.floor(i/3))%FRUIT_KINDS.length]:config.fruit;
        // Apply user-authored spatial settings once; Rust owns motion afterwards.
        x*=config.width/.74;y+=config.height-1.28;z+=config.depth+1.12;
        return {fruit,scale:config.fruitScale,position:/** @type {import('../../js/engine.d.ts').Vector3} */([x,y,z]),
            velocity:/** @type {import('../../js/engine.d.ts').Vector3} */([0,config.formation==='rain'?config.lift*.2:config.lift,.015]),gravity:config.gravity,lifetime:config.lifetime};
    });
}
