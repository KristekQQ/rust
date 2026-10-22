import {DEFAULT_WAVE,validateWave} from './wave-plan.js';
export const ARENA_STORAGE_KEY='neon-arena.wave-preset.v1';
/** @typedef {{version:1,name:string,wave:import('./wave-plan.js').WaveConfig}} ArenaPreset */
/** @param {unknown} value @returns {ArenaPreset} */
export function validatePreset(value) {
    if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('Očekávám JSON předvolby.');
    const preset=/** @type {Record<string,unknown>} */(value);
    if(Object.keys(preset).some(key=>!['version','name','wave'].includes(key))||preset.version!==1)throw new Error('Nepodporovaná verze předvolby.');
    if(typeof preset.name!=='string'||!preset.name.trim()||preset.name.length>80)throw new Error('Předvolba potřebuje název do 80 znaků.');
    if(!preset.wave||typeof preset.wave!=='object'||Object.keys(DEFAULT_WAVE).filter(key=>key!=='program').some(key=>!Object.hasOwn(/** @type {object} */(preset.wave),key)))throw new Error('V předvolbě chybí nastavení vlny.');
    return {version:1,name:preset.name,wave:validateWave(preset.wave)};
}
/** @param {import('./wave-plan.js').WaveConfig} wave @param {string} [name] @returns {ArenaPreset} */
export function makePreset(wave,name='Moje ovocná aréna') {return {version:1,name,wave:{...wave}};}
/** @param {URL} url */
export async function fetchPreset(url) {
    const response=await fetch(url,{cache:'no-store'});
    if(!response.ok)throw new Error(`Nelze načíst předvolbu: HTTP ${response.status}`);
    return validatePreset(await response.json());
}
