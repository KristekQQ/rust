import test from 'node:test';
import assert from 'node:assert/strict';
import {installNamedScoreboard,validateLeaderboard,loadLeaderboard,LEADERBOARD_STORAGE_KEY} from '../examples/neon-arena/scoreboard.js';
const flush=()=>new Promise(resolve=>setImmediate(resolve));
class Bridge extends EventTarget {
    state={scoreEntrySequence:0,scoreEntryPending:false};
    change(value){Object.assign(this.state,value);this.dispatchEvent(new Event('state'));}
}
test('new season accepts 1-3 initials and rejects anonymous, zero, unsafe or excessive entries',()=>{
    assert.equal(LEADERBOARD_STORAGE_KEY,'neon-leaderboard-v2');
    assert.deepEqual(validateLeaderboard([{score:2,initials:'A'},{score:3,initials:'ABC'}]),[{score:3,initials:'ABC'},{score:2,initials:'A'}]);
    for(const entries of [[3],[{score:0,initials:'ABC'}],[{score:2,initials:'ABCD'}],[{score:2,initials:'a'}],[{score:2,initials:'A\n'}],[{score:2**53,initials:'A'}],Array(11).fill({score:2,initials:'A'})])assert.throws(()=>validateLeaderboard(entries));
});
test('load uses versioned API',async()=>{
    assert.deepEqual(await loadLeaderboard(async url=>{assert.equal(url,'/__neon_leaderboard_v2');return {ok:true,json:async()=>[]};}),[]);
});
test('round end and pending preview do not submit; confirmation submits exactly once',async()=>{
    const bridge=new Bridge(),calls=[],restored=[];
    const dispose=installNamedScoreboard(bridge,{sessionId:'test',fetcher:async(url,options)=>{calls.push({url,body:JSON.parse(options.body)});return {ok:true,json:async()=>[{score:42,initials:'HZ'}]};},onScores:entries=>restored.push(entries)});
    bridge.dispatchEvent(new Event('roundend'));
    bridge.change({scoreEntryPending:true,lastScoreEntry:null});await flush();assert.equal(calls.length,0);
    bridge.change({scoreEntrySequence:1,scoreEntryPending:false,lastScoreEntry:{score:42,initials:'HZ'}});
    bridge.dispatchEvent(new Event('state'));await flush();assert.equal(calls.length,1);
    assert.deepEqual(calls[0],{url:'/__neon_leaderboard_v2',body:{score:42,initials:'HZ',requestId:'test:1'}});
    assert.equal(restored.length,1);dispose();
    bridge.change({scoreEntrySequence:2});await flush();assert.equal(calls.length,1);
});
test('late server replies cannot replace newer confirmations or initials in progress',async()=>{
    const bridge=new Bridge(),resolvers=[],restored=[];
    const dispose=installNamedScoreboard(bridge,{fetcher:()=>new Promise(resolve=>resolvers.push(resolve)),onScores:entries=>restored.push(entries)});
    bridge.change({scoreEntrySequence:1,lastScoreEntry:{score:1,initials:'A'}});await flush();
    bridge.change({scoreEntrySequence:2,lastScoreEntry:{score:2,initials:'B'}});
    resolvers[0]({ok:true,json:async()=>[{score:1,initials:'A'}]});await flush();
    assert.equal(restored.length,0);assert.equal(resolvers.length,2);
    bridge.change({scoreEntryPending:true});resolvers[1]({ok:true,json:async()=>[{score:2,initials:'B'}]});await flush();
    assert.equal(restored.length,0);dispose();
});
test('disposed scoreboard ignores an in-flight response',async()=>{
    const bridge=new Bridge();let reply,restored=false;
    const dispose=installNamedScoreboard(bridge,{fetcher:()=>new Promise(resolve=>{reply=resolve;}),onScores:()=>{restored=true;}});
    bridge.change({scoreEntrySequence:1,lastScoreEntry:{score:1,initials:'A'}});await flush();dispose();
    reply({ok:true,json:async()=>[]});await flush();assert.equal(restored,false);
});

function memoryStorage(){const values=new Map();return {getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value)};}
test('offline confirmation survives reload and replays before fetching server scores',async()=>{
    const storage=memoryStorage(),bridge=new Bridge(),calls=[];
    const dispose=installNamedScoreboard(bridge,{storage,sessionId:'offline',fetcher:async()=>{throw new Error('offline');}});
    bridge.change({scoreEntrySequence:1,lastScoreEntry:{score:99,initials:'HZ'}});await flush();dispose();
    const entries=await loadLeaderboard(async(url,options)=>{
        calls.push({url,method:options.method??'GET',body:options.body?JSON.parse(options.body):null});
        return {ok:true,json:async()=>[{score:99,initials:'HZ'}]};
    },storage);
    assert.deepEqual(calls.map(call=>call.method),['POST','GET']);
    assert.deepEqual(calls[0].body,{score:99,initials:'HZ',requestId:'offline:1'});
    assert.deepEqual(entries,[{score:99,initials:'HZ'}]);
    assert.equal(storage.getItem(`${LEADERBOARD_STORAGE_KEY}-pending`),'[]');
});
test('failed pending replay prevents accepting a server view missing offline scores',async()=>{
    const storage=memoryStorage();
    storage.setItem(`${LEADERBOARD_STORAGE_KEY}-pending`,JSON.stringify([{score:99,initials:'HZ',requestId:'offline:1'}]));
    let calls=0;
    await assert.rejects(loadLeaderboard(async()=>{calls++;return {ok:false,status:503};},storage),/503/);
    assert.equal(calls,1);assert.match(storage.getItem(`${LEADERBOARD_STORAGE_KEY}-pending`),/offline:1/);
});
test('a later successful submission cannot overwrite an earlier offline confirmation',async()=>{
    const storage=memoryStorage(),bridge=new Bridge();let calls=0,restored=false;
    const dispose=installNamedScoreboard(bridge,{storage,fetcher:async()=>{
        if(++calls===1)throw new Error('offline');
        return {ok:true,json:async()=>[{score:2,initials:'B'}]};
    },onScores:()=>{restored=true;}});
    bridge.change({scoreEntrySequence:1,lastScoreEntry:{score:1,initials:'A'}});await flush();
    bridge.change({scoreEntrySequence:2,lastScoreEntry:{score:2,initials:'B'}});await flush();
    assert.equal(calls,2);assert.equal(restored,false);dispose();
});
test('blocked browser storage never prevents named leaderboard setup',async()=>{
    const descriptor=Object.getOwnPropertyDescriptor(globalThis,'localStorage');
    Object.defineProperty(globalThis,'localStorage',{configurable:true,get(){throw new Error('blocked');}});
    try{
        const dispose=installNamedScoreboard(new Bridge());dispose();
        assert.deepEqual(await loadLeaderboard(async()=>({ok:true,json:async()=>[]})),[]);
    }finally{
        if(descriptor)Object.defineProperty(globalThis,'localStorage',descriptor);else delete globalThis.localStorage;
    }
});
