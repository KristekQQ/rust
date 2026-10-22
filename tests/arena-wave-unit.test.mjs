import test from 'node:test';
import assert from 'node:assert/strict';
import {DEFAULT_WAVE,FORMATIONS,planWave,validateWave} from '../examples/neon-arena/wave-plan.js';
import {createArenaTools} from '../examples/neon-arena/arena-tools.js';
test('ten nonoverlapping targets stay in the forward reachable envelope during their rise',()=>{
    for(const formation of FORMATIONS){
        const plan=planWave({...DEFAULT_WAVE,formation});assert.equal(plan.length,10);
        for(const target of plan)for(const t of [0,.5,1]){
            const [x,y,z]=target.position,vy=target.velocity[1];
            assert.ok(Math.abs(x)<=(['line','arc'].includes(formation)?.75:.7)&&z<-.9&&z>=-1.3);
            assert.ok(y+vy*t-.5*target.gravity*t*t>=.9&&y+vy*t-.5*target.gravity*t*t<=2);
        }
        for(let i=0;i<plan.length;i++)for(let j=0;j<i;j++){
            const distance=Math.hypot(...plan[i].position.map((v,k)=>v-plan[j].position[k]));
            assert.ok(distance>(['line','arc'].includes(formation)?.16:.18),`${formation} targets overlap: ${distance}`);
        }
    }
});
test('validation rejects malformed commands atomically and API uses common hooks',()=>{
    const calls=[];const tools=createArenaTools({state:()=>({live:0}),spawn:c=>{calls.push(c);return {spawned:c.count};},start:()=>calls.push('start'),stop:()=>calls.push('stop'),changed:()=>calls.push('change')});
    for(const bad of [{count:11},{count:NaN},{count:2.2},{intervalMs:10},{formation:'behind'},{fruit:'bomb'},{automatic:1},{program:'invalid'},{__proto__:null,extra:1},null,[]])assert.throws(()=>tools.call('arena_configure',bad));
    assert.deepEqual(tools.config,DEFAULT_WAVE);assert.equal(calls.length,0);
    tools.call('arena_configure',{count:6});assert.equal(tools.config.count,6);
    assert.equal(tools.call('arena_spawn').spawned,6);
    tools.call('arena_start');tools.call('arena_stop');assert.equal(calls.at(-1),'stop');
    assert.throws(()=>tools.call('unknown'));
    const snapshot=tools.config;snapshot.count=1;assert.equal(tools.config.count,6);
});
test('mix supplies contiguous matching groups and changes fruit across waves',()=>{
    const a=planWave(DEFAULT_WAVE,0),b=planWave(DEFAULT_WAVE,1);
    assert.equal(a[0].fruit,a[2].fruit);assert.notEqual(a[2].fruit,a[3].fruit);assert.notEqual(a[0].fruit,b[0].fruit);
    assert.equal(planWave(validateWave({count:1})).length,1);
});
test('browser bridge ignores expired commands and executes a fresh MCP command once',async t=>{
    const {installArenaBridge}=await import('../examples/neon-arena/arena-tools.js');
    const originals=Object.fromEntries(['document','window','HTMLButtonElement','HTMLInputElement','fetch'].map(key=>[key,globalThis[key]]));
    class Button {disabled=false;onclick=null;}
    class Input {value='test-token';}
    const button=new Button(),input=new Input(),status={textContent:''},responses=[];
    let finish;const done=new Promise(resolve=>{finish=resolve;});
    Object.assign(globalThis,{HTMLButtonElement:Button,HTMLInputElement:Input,
        document:{getElementById:id=>({'arena-connect':button,'arena-token':input,'arena-connection':status}[id])},
        window:{setTimeout:()=>1},
        fetch:async(url,options)=>{
            assert.equal(options.headers.Authorization,'Bearer test-token');
            if(String(url).includes('/poll?'))return {ok:true,status:200,json:async()=>({commands:[
                {id:'expired',name:'arena_spawn',arguments:{},expiresAt:Date.now()-100},
                {id:'fresh',name:'arena_spawn',arguments:{},expiresAt:Date.now()+5000}]})};
            responses.push(JSON.parse(options.body));if(responses.length===2)finish();
            return {ok:true,status:204};
        }});
    let spawned=0;
    const tools=createArenaTools({state:()=>({}),spawn:c=>{spawned++;return {spawned:c.count};},start:()=>{},stop:()=>{},changed:()=>{}});
    const disconnect=installArenaBridge(tools);
    t.after(()=>{disconnect();for(const [key,value] of Object.entries(originals))value===undefined?delete globalThis[key]:globalThis[key]=value;});
    button.onclick();await done;
    assert.equal(spawned,1);assert.equal(responses[0].id,'expired');assert.ok(responses[0].error);
    assert.equal(responses[1].result.spawned,10);assert.equal(input.value,'');
});

test('line puts all ten smaller fruit in one straight horizontal row',()=>{
    const plan=planWave({...DEFAULT_WAVE,formation:'line'});
    assert.equal(new Set(plan.map(target=>target.position[1])).size,1);
    assert.equal(new Set(plan.map(target=>target.position[2])).size,1);
    assert.ok(plan.every((target,i)=>i===0||target.position[0]>plan[i-1].position[0]));
});

test('curved strokes alternate slope and bend across the round without demanding a straight swipe',()=>{
    const plans=Array.from({length:4},(_,wave)=>planWave({...DEFAULT_WAVE,formation:'arc'},wave));
    for(const plan of plans){
        const first=plan[0].position,last=plan.at(-1).position;
        const offChord=plan.slice(1,-1).map(target=>{const t=(target.position[0]-first[0])/(last[0]-first[0]);return target.position[1]-(first[1]+t*(last[1]-first[1]));});
        assert.ok(Math.max(...offChord.map(Math.abs))>.19);
        assert.ok(plan.every(target=>target.position[1]>1&&target.position[1]<1.6));
    }
    assert.ok(plans[0].at(-1).position[1]>plans[0][0].position[1]);
    assert.ok(plans[1].at(-1).position[1]<plans[1][0].position[1]);
    assert.ok(plans[0][4].position[1]>plans[2][4].position[1]+.3);
});

test('scene preview tool validates time before invoking its hook',()=>{
    const requested=[];const tools=createArenaTools({state:()=>({}),spawn:()=>({}),start:()=>{},stop:()=>{},changed:()=>{},preview:seconds=>{requested.push(seconds);return {label:'FINALE'};}});
    assert.equal(tools.call('arena_preview',{seconds:55}).label,'FINALE');
    tools.call('arena_preview');assert.deepEqual(requested,[55,undefined]);
    for(const seconds of [-1,60,Infinity,'10',null])assert.throws(()=>tools.call('arena_preview',{seconds}));
    assert.equal(requested.length,2);
});

test('chain banking tool validates the stock before forwarding to the native hook',()=>{
    const received=[];const tools=createArenaTools({state:()=>({}),spawn:()=>({}),start:()=>{},stop:()=>{},changed:()=>{},bank:hand=>{received.push(hand);return{banked:true};}});
    for(const args of [{},{hand:'third'},{hand:1},{hand:'left',score:100},null,[]])assert.throws(()=>tools.call('arena_bank',args));
    assert.deepEqual(received,[]);assert.deepEqual(tools.call('arena_bank',{hand:'right'}),{banked:true});assert.deepEqual(received,['right']);
});
