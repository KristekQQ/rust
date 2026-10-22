/** Authored positions and timing only; Rust owns projectile simulation. */
import {DEFAULT_WAVE,FRUIT_KINDS,validateWave,planWave} from './wave-plan.js';
/** @typedef {import('./wave-plan.js').WaveConfig} WaveConfig */
/** @typedef {{position:[number,number,number],fruit?:string,scale?:number,velocity?:[number,number,number],lifetime?:number}} RoundTarget */
/** @typedef {{apexSeconds:number,gravity:number}} RoundLaunch */
/** @typedef {{at:number,label:string,wave:Partial<WaveConfig>,targets:RoundTarget[],launch?:RoundLaunch}} RoundCue */
/** @typedef {{version:1,name:string,durationSeconds:60,cues:RoundCue[]}} RoundPlan */

/** @param {unknown} input @param {string[]} keys */
function record(input,keys) {
    if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('Ocekavam objekt planu.');
    const value=/** @type {Record<string,unknown>} */(input);
    for(const key of Object.keys(value))if(!keys.includes(key))throw new Error(`Nezname pole: ${key}`);
    return value;
}
/** @param {unknown} input @returns {RoundCue} */
function validateCue(input) {
    const cue=record(input,['at','label','wave','targets','launch']);
    /** @type {RoundLaunch|undefined} */
    let launch;
    if(cue.launch!==undefined){
        const value=record(cue.launch,['apexSeconds','gravity']);
        for(const [key,min,max] of /** @type {[string,number,number][]} */([['apexSeconds',.38,.9],['gravity',2,10.5]])){
            const number=value[key];
            if(typeof number!=='number'||!Number.isFinite(number)||number<min||number>max)throw new Error(`Neplatny balisticky start: ${key}`);
        }
        launch=/** @type {RoundLaunch} */({apexSeconds:value.apexSeconds,gravity:value.gravity});
    }
    if(typeof cue.at!=='number'||!Number.isFinite(cue.at)||cue.at<0||cue.at>=60)throw new Error('Cas cue musi byt 0 az mene nez 60.');
    if(typeof cue.label!=='string'||!cue.label.trim())throw new Error('Cue potrebuje nazev.');
    if(!Array.isArray(cue.targets)||cue.targets.length<1||cue.targets.length>10)throw new Error('Cue potrebuje 1 az 10 cilu.');
    const targets=cue.targets.map(input=>{
        const target=record(input,['position','fruit','scale','velocity','lifetime']);
        if(!Array.isArray(target.position)||target.position.length!==3||target.position.some(v=>typeof v!=='number'||!Number.isFinite(v)))throw new Error('Pozice musi mit tri konecna cisla.');
        const [x,y,z]=target.position;
        if(x<-.8||x>.8||y<.95||y>1.7||z< -1.4||z>-.9)throw new Error('Cil je mimo dosah pred hracem.');
        if(target.fruit!==undefined&&(typeof target.fruit!=='string'||!FRUIT_KINDS.includes(target.fruit)))throw new Error('Nezname ovoce cile.');
        if(target.scale!==undefined&&(typeof target.scale!=='number'||!Number.isFinite(target.scale)||target.scale<.03||target.scale>.105))throw new Error('Meritko cile musi byt .03 az .105.');
        if(target.lifetime!==undefined&&(typeof target.lifetime!=='number'||!Number.isFinite(target.lifetime)||target.lifetime<1.2||target.lifetime>2.6))throw new Error('Zivotnost cile musi byt 1.2 az 2.6 s.');
        if(target.velocity!==undefined){
            if(!Array.isArray(target.velocity)||target.velocity.length!==3||target.velocity.some(v=>typeof v!=='number'||!Number.isFinite(v)))throw new Error('Rychlost musi mit tri konecna cisla.');
            const [vx,vy,vz]=target.velocity;
            if(Math.abs(vx)>.6||(!launch&&(vy<.1||vy>1))||Math.abs(vz)>.15)throw new Error('Rychlost cile je mimo povoleny rozsah.');
        }
        return /** @type {RoundTarget} */({position:[x,y,z],
            ...(target.fruit===undefined?{}:{fruit:target.fruit}),
            ...(target.scale===undefined?{}:{scale:target.scale}),
            ...(target.velocity===undefined?{}:{velocity:[...target.velocity]}),
            ...(target.lifetime===undefined?{}:{lifetime:target.lifetime})});
    });
    const patch=record(cue.wave,Object.keys(DEFAULT_WAVE));
    if(patch.count!==undefined&&patch.count!==targets.length)throw new Error('Pocet vlny neodpovida cilum.');
    validateWave({...patch,count:targets.length});
    if(launch)for(const target of targets)ballisticLaunch(target.position,target.velocity??[0,0,.015],launch);
    return {at:cue.at,label:cue.label,...(launch?{launch}:{}),wave:/** @type {Partial<WaveConfig>} */({...patch,count:targets.length}),targets};
}
/** Validate and copy a JSON round without retaining mutable caller data.
 * @param {unknown} input @returns {RoundPlan} */
export function validateRoundPlan(input) {
    const plan=record(input,['version','name','durationSeconds','cues']);
    if(plan.version!==1||plan.durationSeconds!==60)throw new Error('Podporovan je minutovy plan verze 1.');
    if(typeof plan.name!=='string'||!plan.name.trim())throw new Error('Plan potrebuje nazev.');
    if(!Array.isArray(plan.cues)||!plan.cues.length)throw new Error('Plan potrebuje cues.');
    const cues=plan.cues.map(validateCue);
    for(let i=1;i<cues.length;i++)if(cues[i].at<=cues[i-1].at)throw new Error('Casy cues musi byt serazene a jedinecne.');
    return {version:1,name:plan.name,durationSeconds:60,cues};
}
/** Convert an authored strike point into a launch recipe. For launch cues the
 * authored velocity y is ignored; x/z remain the horizontal launch velocity.
 * @param {readonly number[]} apex @param {readonly number[]} velocity @param {RoundLaunch} launch */
function ballisticLaunch(apex,velocity,launch){
    const t=launch.apexSeconds,g=launch.gravity;
    const position=/** @type {[number,number,number]} */([apex[0]-velocity[0]*t,apex[1]-.5*g*t*t,apex[2]-velocity[2]*t]);
    if(Math.abs(position[0])>1.1||position[1]<.08||position[2]<-1.6||position[2]>-.85)throw new Error('Balisticky start je mimo bezpecny prostor.');
    return {position,velocity:/** @type {[number,number,number]} */([velocity[0],g*t,velocity[2]]),gravity:g};
}
/** Keep Rust launch recipes, applying optional per-target size, motion and expiry.
 * @param {RoundCue} cue @param {WaveConfig} [baseWave] @param {number} [waveIndex] */
export function planCue(cue,baseWave=DEFAULT_WAVE,waveIndex=0) {
    const checked=validateCue(cue);
    const config=validateWave(checked.wave,validateWave(baseWave));
    // The minute choreography budgets original fruit for at most 2.6 seconds.
    config.lifetime=Math.min(config.lifetime,2.6);
    return planWave(config,waveIndex).map((recipe,i)=>{
        const target=checked.targets[i];
        return {...recipe,position:/** @type {[number,number,number]} */([...target.position]),
            fruit:target.fruit??recipe.fruit,scale:target.scale??recipe.scale,
            velocity:/** @type {[number,number,number]} */([...(target.velocity??recipe.velocity)]),
            lifetime:target.lifetime??recipe.lifetime,
            ...(checked.launch?ballisticLaunch(target.position,target.velocity??recipe.velocity,checked.launch):{})};
    });
}
