import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {DEFAULT_WAVE,planWave} from '../examples/neon-arena/wave-plan.js';
import {ARENA_STORAGE_KEY,validatePreset,makePreset} from '../examples/neon-arena/arena-presets.js';
import {createArenaTools} from '../examples/neon-arena/arena-tools.js';
const authored=JSON.parse(await readFile(new URL('../examples/neon-arena/wave-preset.json',import.meta.url),'utf8'));
const hooks=()=>({state:()=>({}),spawn:c=>({spawned:c.count}),start:()=>{},stop:()=>{},changed:()=>{}});
const memory=()=>{const data=new Map();return {getItem:k=>data.get(k)??null,setItem:(k,v)=>data.set(k,v),removeItem:k=>data.delete(k)};};
test('repository JSON records the accepted exact layout, size and motion',()=>{
    const preset=validatePreset(authored);assert.deepEqual(preset.wave,DEFAULT_WAVE);
    const recipes=planWave(preset.wave);assert.equal(recipes[0].scale,.09);assert.equal(recipes[0].gravity,.8);assert.equal(recipes[0].lifetime,2.6);
    const next=planWave({...preset.wave,curve:.1,slope:.08,width:.6,height:1.4,depth:-1.3,lift:.4,gravity:1,fruitScale:.05,lifetime:3});
    assert.equal(next[0].scale,.05);assert.equal(next[0].velocity[1],.4);assert.equal(next[0].gravity,1);assert.equal(next[0].lifetime,3);
    assert.notDeepEqual(next[4].position,recipes[4].position);
});
test('configure autosaves full JSON and a fresh tool restores without spawning',()=>{
    const storage=memory();const a=createArenaTools(hooks(),{preset:authored,storage});
    assert.equal(a.call('arena_configure',{count:6,curve:.1,width:.6}).saved,true);
    assert.equal(JSON.parse(storage.getItem(ARENA_STORAGE_KEY)).name,authored.name);
    const b=createArenaTools({...hooks(),spawn:()=>{throw new Error('must not spawn on load');}},{preset:authored,storage});
    assert.equal(b.config.count,6);assert.equal(b.config.curve,.1);assert.equal(b.call('arena_state').restored,true);
    assert.deepEqual(b.call('arena_export').wave,a.config);
    const copy=b.call('arena_export');copy.wave.count=1;assert.equal(b.config.count,6);
    b.call('arena_save');b.call('arena_reset');assert.equal(storage.getItem(ARENA_STORAGE_KEY),null);assert.deepEqual(b.config,authored.wave);
});
test('JSON import/export round-trips, rejects corrupt/versioned/partial presets without mutation',()=>{
    const storage=memory(),tools=createArenaTools(hooks(),{preset:authored,storage});
    const imported=makePreset({...DEFAULT_WAVE,count:8,slope:.1},'Osmička');
    tools.call('arena_import',{json:JSON.stringify(imported)});assert.equal(tools.call('arena_export').name,'Osmička');
    const before=storage.getItem(ARENA_STORAGE_KEY);
    for(const json of ['{',JSON.stringify({...imported,version:2}),JSON.stringify({...imported,wave:{count:2}}),JSON.stringify({...imported,wave:{...imported.wave,height:100}})]){
        assert.throws(()=>tools.call('arena_import',{json}));assert.equal(storage.getItem(ARENA_STORAGE_KEY),before);assert.deepEqual(tools.config,imported.wave);
    }
    storage.setItem(ARENA_STORAGE_KEY,'broken');assert.throws(()=>tools.call('arena_load'));assert.deepEqual(tools.config,imported.wave);
    const fallback=createArenaTools(hooks(),{preset:authored,storage});assert.deepEqual(fallback.config,authored.wave);assert.ok(fallback.call('arena_state').storageError);
    assert.equal(storage.getItem(ARENA_STORAGE_KEY),'broken');
});
test('quota or disabled storage reports failure, preserves imported config, and never claims saved',()=>{
    const denied={getItem:()=>null,setItem:()=>{throw new Error('QuotaExceededError');},removeItem:()=>{throw new Error('SecurityError');}};
    const tools=createArenaTools(hooks(),{preset:authored,storage:denied});
    const result=tools.call('arena_configure',{count:5});assert.equal(result.saved,false);assert.ok(result.warning);assert.equal(tools.config.count,5);
    assert.throws(()=>tools.call('arena_save'));assert.throws(()=>tools.call('arena_reset'));assert.equal(tools.config.count,5);
    assert.throws(()=>tools.call('arena_import',{json:JSON.stringify(authored)}));assert.equal(tools.config.count,5);
});

test('legacy saved wave config migrates to the minute program without losing geometry',()=>{
    const legacy=structuredClone(authored);delete legacy.wave.program;legacy.name='Stara predvolba';
    const migrated=validatePreset(legacy);assert.equal(migrated.wave.program,'minute');assert.equal(migrated.wave.curve,legacy.wave.curve);
});

test('old built-in tiny fruit upgrades without losing custom wave settings',()=>{
    const storage=memory(),old=structuredClone(authored);
    old.wave.fruitScale=.06;old.wave.count=7;old.wave.curve=.12;
    storage.setItem(ARENA_STORAGE_KEY,JSON.stringify(old));
    const tools=createArenaTools(hooks(),{preset:authored,storage});
    assert.equal(tools.config.fruitScale,.09);assert.equal(tools.config.count,7);assert.equal(tools.config.curve,.12);
    assert.equal(JSON.parse(storage.getItem(ARENA_STORAGE_KEY)).wave.fruitScale,.09);
    old.name='My deliberate small targets';storage.setItem(ARENA_STORAGE_KEY,JSON.stringify(old));
    assert.equal(createArenaTools(hooks(),{preset:authored,storage}).config.fruitScale,.06);
});
