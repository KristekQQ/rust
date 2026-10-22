import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {validateRoundPlan,planCue} from '../examples/neon-arena/round-plan.js';
import {DEFAULT_WAVE,FRUIT_KINDS,planWave} from '../examples/neon-arena/wave-plan.js';
const source=JSON.parse(readFileSync(new URL('../examples/neon-arena/minute-scene.json',import.meta.url),'utf8'));
const pristine=structuredClone(source);
const plan=validateRoundPlan(source);
const copy=()=>structuredClone(source);
const values={cherry:10,lime:15,lemon:20,orange:25,banana:40,plum:50,pear:60,grapes:100,watermelon:120};

// Mirrors ComboScore::hit_event in src/xr_round.rs: only a completed triple
// pays once, a mismatch replaces the unfinished kind, and misses do nothing.
function scoreHits(hits){
    let kind='',streak=0,points=0,score=0,triples=0;
    for(const hit of hits){
        if(!hit)continue;
        const fruit=typeof hit==='string'?hit:hit.fruit,wild=typeof hit==='object'&&hit.wild;
        if(!wild){if(kind&&kind!==fruit)streak=0;kind=fruit;points=values[fruit];}
        else if(!kind)points=values[fruit];
        if(++streak===3){score+=points;triples++;kind='';streak=0;points=0;}
    }
    return {score,triples,kind,streak};
}

test('six score challenges provide choices, delayed completion, precision, timing, speed and a finale',()=>{
    assert.equal(plan.durationSeconds,60);assert.ok(plan.cues.length>=22&&plan.cues.length<=28);
    const labels=['VOLBA','DOKONCI','PRESNOST','NACASOVANI','SERIE','FINALE'];
    for(const [i,label] of labels.entries()){
        const cues=plan.cues.filter(c=>c.at>=i*10&&c.at<(i+1)*10);
        assert.equal(cues.length,4,label);assert.ok(cues.every(c=>c.label===label));
    }
    for(const c of plan.cues.filter(c=>['VOLBA','PRESNOST','SERIE'].includes(c.label))){
        const valuable=c.targets.filter(t=>values[t.fruit]>=60),cheap=c.targets.filter(t=>values[t.fruit]<=25);
        assert.equal(valuable.length,3);assert.equal(cheap.length,3);
        assert.equal(new Set(valuable.map(t=>t.fruit)).size,1);
        assert.equal(new Set(cheap.map(t=>t.fruit)).size,1);
        assert.notDeepEqual(valuable.map(t=>t.position),cheap.map(t=>t.position));
    }
    const delayed=plan.cues.filter(c=>c.label==='DOKONCI');
    for(let i=0;i<delayed.length;i+=2){
        const first=delayed[i],third=delayed[i+1],fruit=first.targets.find(t=>values[t.fruit]>=60).fruit;
        assert.equal(first.targets.filter(t=>t.fruit===fruit).length,2);
        assert.equal(third.targets.filter(t=>t.fruit===fruit).length,1);
        assert.ok(third.at-first.at>=1.5&&third.at-first.at<=1.7);
        assert.ok(third.targets.filter(t=>t.fruit!==fruit).length>=3);
    }
    for(const c of plan.cues.filter(c=>c.label==='PRESNOST')){
        assert.ok(c.targets.filter(t=>values[t.fruit]>=60).every(t=>t.scale>=.075&&t.scale<=.08&&t.lifetime>=1.6&&t.lifetime<=2.2));
        assert.ok(c.targets.filter(t=>values[t.fruit]<=25).every(t=>t.scale>=.08));
    }
    for(const c of plan.cues.filter(c=>c.label==='NACASOVANI')){
        const high=c.targets.filter(t=>values[t.fruit]>=60);
        const xs=planCue(c).filter(t=>values[t.fruit]>=60).map(t=>t.position[0]+t.velocity[0]*c.launch.apexSeconds);
        assert.ok(Math.max(...xs)-Math.min(...xs)<.001,'valuable fruit align at the real ballistic apex');
        assert.ok(high.some(t=>t.velocity[0]===.25)&&high.some(t=>t.velocity[0]===-.25));
        const ys=high.map(t=>t.position[1]).sort();assert.ok(ys[1]-ys[0]>=.2&&ys[2]-ys[1]>=.2);
    }
    assert.ok(plan.cues.filter(c=>c.label==='SERIE').every(c=>c.wave.lifetime===1.8));
    assert.equal(plan.cues.at(-1).targets.length,10);
});

test('selecting valuable triples beats naive sweeping under the existing scoring rules',()=>{
    assert.equal(scoreHits(['grapes','grapes',null,'grapes']).score,100);
    assert.equal(scoreHits(['grapes','grapes','cherry','grapes']).score,0);
    assert.equal(scoreHits(['grapes',{fruit:'watermelon',wild:true},'grapes']).score,100);
    assert.equal(scoreHits([{fruit:'watermelon',wild:true},{fruit:'watermelon',wild:true},{fruit:'watermelon',wild:true}]).score,120);
    const naive=scoreHits(plan.cues.flatMap(c=>c.targets.map(t=>t.fruit)));
    const selective=scoreHits(plan.cues.flatMap(c=>c.targets.filter(t=>values[t.fruit]>=60).sort((a,b)=>values[b.fruit]-values[a.fruit]).map(t=>t.fruit)));
    const beginner=scoreHits(plan.cues.flatMap(c=>c.targets.filter(t=>values[t.fruit]<=25).map(t=>t.fruit)));
    assert.equal(naive.score,110);assert.equal(selective.score,2440);
    assert.ok(beginner.score>=250,'large cheap triples remain useful');
    assert.ok(selective.score>=2000&&selective.score>=naive.score*4,`${JSON.stringify({naive,selective,beginner})}`);
    // Also compare a wide left-to-right cut that meets both lanes at each x.
    const spatial=scoreHits(plan.cues.flatMap(c=>[...c.targets].sort((a,b)=>a.position[0]-b.position[0]||a.position[1]-b.position[1]).map(t=>t.fruit)));
    assert.ok(selective.score>spatial.score,JSON.stringify({selective,spatial}));
});

test('authored fruit stays reachable and separated, with default size on most targets',()=>{
    let defaults=0,total=0;
    for(const [i,cue] of plan.cues.entries()){
        const targets=planCue(cue,DEFAULT_WAVE,i);
        assert.equal(cue.wave.count,targets.length);
        for(const target of targets){
            const [x,y,z]=target.position;
            assert.ok(target.position.every(Number.isFinite));
            assert.ok(Math.abs(x)<=1.1&&y>=.08&&z>=-1.6&&z<=-.85);
            const authored=cue.targets[targets.indexOf(target)],t=cue.launch.apexSeconds;
            assert.equal(target.gravity,cue.launch.gravity);assert.ok(Math.abs(target.velocity[1]-target.gravity*t)<1e-10);
            const apex=target.position.map((v,k)=>v+target.velocity[k]*t-(k===1?.5*target.gravity*t*t:0));
            apex.forEach((v,k)=>assert.ok(Math.abs(v-authored.position[k])<1e-10));
            assert.ok(target.lifetime>=1.4&&target.lifetime<=2.2);
            assert.ok(FRUIT_KINDS.includes(target.fruit));assert.ok(cue.targets.every(t=>t.fruit));
            total++;if(target.scale===DEFAULT_WAVE.fruitScale)defaults++;
        }
        for(let a=0;a<targets.length;a++)for(let b=0;b<a;b++){
            assert.ok(Math.hypot(...targets[a].position.map((v,k)=>v-targets[b].position[k]))>=Math.max(.14,targets[a].scale+targets[b].scale),`${i}: fruit ${a}/${b} overlap`);
        }
    }
    assert.ok(defaults/total>.75);
});

test('no cuts admit every cue under the ten-original cap and clear the finale before 60 s',()=>{
    for(const lifetime of [1.5,2.6,3.5]){
        const scheduled=plan.cues.map((cue,i)=>({at:cue.at,targets:planCue(cue,{...DEFAULT_WAVE,lifetime},i)}));
        let peak=0,admitted=0;
        for(const cue of scheduled){
            const active=scheduled.filter(c=>c.at<=cue.at).flatMap(c=>c.targets.map(target=>({expires:c.at+target.lifetime}))).filter(t=>t.expires>cue.at+1e-8);
            peak=Math.max(peak,active.length);
            assert.ok(active.length<=10,`lifetime ${lifetime}, at ${cue.at}: ${active.length}`);
            admitted+=cue.targets.length;assert.ok(cue.targets.every(t=>cue.at+t.lifetime<60));
        }
        assert.equal(peak,10);assert.equal(admitted,plan.cues.reduce((sum,c)=>sum+c.targets.length,0));
    }
});

test('target overrides copy and replace recipe scale, velocity and lifetime without mutating inputs',()=>{
    const cue=structuredClone(plan.cues[0]);delete cue.launch;cue.targets.forEach(t=>t.velocity[1]=.6);
    cue.targets[1]={...cue.targets[1],fruit:'watermelon',scale:.04,velocity:[.4,.7,-.1],lifetime:1.2};
    const base={...DEFAULT_WAVE,fruitScale:.08,lift:.7,gravity:1.1,lifetime:3,fruit:'pear'};
    const expected=planWave({...base,...cue.wave,count:cue.targets.length},4),before=structuredClone(cue);
    const recipes=planCue(cue,base,4);
    for(let i=0;i<recipes.length;i++){
        const target=cue.targets[i];
        assert.deepEqual(recipes[i],{...expected[i],position:target.position,fruit:target.fruit??expected[i].fruit,scale:target.scale??expected[i].scale,velocity:target.velocity??expected[i].velocity,lifetime:target.lifetime??expected[i].lifetime});
    }
    recipes[0].position[0]=.7;recipes[1].velocity[0]=-.4;
    assert.deepEqual(cue,before);
    const validated=validateRoundPlan({ ...source,cues:[cue] });
    validated.cues[0].targets[1].velocity[0]=-.3;assert.deepEqual(cue,before);assert.deepEqual(source,pristine);
});

test('merged wave lifetime retains the cap unless a bounded target explicitly overrides it',()=>{
    const cue=structuredClone(plan.cues[0]);
    for(const [baseLifetime,cueLifetime,expected] of [[3.5,undefined,2.6],[1.5,undefined,1.5],[1.5,3.5,2.6],[3.5,1.8,1.8]]){
        if(cueLifetime===undefined)delete cue.wave.lifetime;else cue.wave.lifetime=cueLifetime;
        assert.ok(planCue(cue,{...DEFAULT_WAVE,lifetime:baseLifetime}).every(t=>t.lifetime===expected));
    }
    cue.targets[0].lifetime=2.6;cue.wave.lifetime=1.5;
    assert.equal(planCue(cue)[0].lifetime,2.6);
});

test('round validation rejects malformed authoring and every target override boundary',()=>{
    const mutations=[
        p=>p.version=2,p=>p.durationSeconds=59,p=>p.name='',p=>p.cues=[],p=>p.extra=true,
        p=>p.cues[0].at=-1,p=>p.cues[0].at=60,p=>p.cues[0].at=NaN,
        p=>p.cues[1].at=p.cues[0].at,p=>p.cues.reverse(),p=>p.cues[0].label='',
        p=>p.cues[0].targets=[],p=>p.cues[0].wave.count=10,p=>p.cues[0].wave.fruit='bomb',
        p=>p.cues[0].wave.lifetime=Infinity,p=>p.cues[0].wave.extra=1,
        p=>p.cues[0].targets[0].fruit='bomb',p=>p.cues[0].targets[0].position=[0,1],
        p=>p.cues[0].targets[0].position=[0,NaN,-1],p=>p.cues[0].targets[0].position=[.81,1,-1],
        p=>p.cues[0].targets[0].position=[0,.94,-1],p=>p.cues[0].targets[0].position=[0,1,-.89],
        p=>p.cues[0].targets[0].position=[0,1,-1.41],p=>p.cues[0].targets[0].extra=1,
    ];
    for(const [key,invalid] of Object.entries({scale:[.029,.106,NaN,'0.06'],lifetime:[1.19,2.61,Infinity,'2'],velocity:[[.61,.5,0],[-.61,.5,0],[0,.5,.151],[0,.5,-.151],[0,NaN,0],[0,.5],null]})){
        for(const value of invalid)mutations.push(p=>p.cues[0].targets[0][key]=value);
    }
    for(const change of mutations){const bad=copy();change(bad);assert.throws(()=>validateRoundPlan(bad));}
    for(const bad of [null,[],{},'plan'])assert.throws(()=>validateRoundPlan(bad));
    const valid=copy();delete valid.cues[0].wave.count;delete valid.cues[0].launch;valid.cues[0].targets.forEach(t=>t.velocity[1]=.6);
    Object.assign(valid.cues[0].targets[0],{scale:.03,lifetime:1.2,velocity:[-.6,.1,-.15]});
    Object.assign(valid.cues[0].targets[1],{scale:.105,lifetime:2.6,velocity:[.6,1,.15]});
    assert.equal(validateRoundPlan(valid).cues[0].wave.count,6);
});


test('opening changes stroke direction, hook and separated staggered fan immediately',()=>{
    assert.equal(plan.cues.length,24);assert.equal(plan.cues.reduce((n,c)=>n+c.targets.length,0),140);
    const group=(i,high)=>plan.cues[i].targets.filter(t=>(values[t.fruit]>=60)===high).map(t=>t.position);
    for(const high of [false,true]){
        const up=group(0,high),down=group(1,high),hook=group(2,high);
        assert.ok(up[2][1]>up[0][1]);assert.ok(down[2][1]<down[0][1]);
        assert.ok((hook[1][0]-hook[0][0])*(hook[2][0]-hook[1][0])<0,'hook turns horizontally');
    }
    assert.ok(Math.max(...group(3,true).map(p=>p[0]))<Math.min(...group(3,false).map(p=>p[0])));
    const shapes=plan.cues.map(c=>JSON.stringify(c.targets.map(t=>t.position)));
    assert.equal(new Set(shapes).size,24,'no repeated horizontal two-row grids');
});

test('ballistic launch validates strict fields, copies settings and ignores only authored vertical velocity',()=>{
    for(const launch of [null,[],{}, {apexSeconds:.42,gravity:9.81,extra:1},
        ...[.379,.901,NaN,'0.42'].map(apexSeconds=>({apexSeconds,gravity:9.81})),
        ...[1.99,10.51,Infinity,'9.81'].map(gravity=>({apexSeconds:.42,gravity}))]){
        const bad=copy();bad.cues[0].launch=launch;assert.throws(()=>validateRoundPlan(bad));
    }
    const bad=copy();bad.cues[0].launch={apexSeconds:.46,gravity:10.5};bad.cues[0].targets[0].position[1]=.95;
    assert.throws(()=>validateRoundPlan(bad),/start/);
    const shallow=copy();shallow.cues[0].targets[0].position[2]=-.9;shallow.cues[0].targets[0].velocity=[0,0,-.15];
    assert.throws(()=>validateRoundPlan(shallow),/start/);
    const cue=structuredClone(plan.cues[0]);cue.targets[0].velocity=[.2,-999,-.1];
    const before=structuredClone(cue),recipes=planCue(cue),validated=validateRoundPlan({...source,cues:[cue]});
    assert.deepEqual(recipes[0].velocity,[.2,cue.launch.gravity*cue.launch.apexSeconds,-.1]);
    validated.cues[0].launch.gravity=8;validated.cues[0].targets[0].velocity[0]=0;
    assert.deepEqual(cue,before);assert.deepEqual(source,pristine);
    delete cue.launch;assert.throws(()=>planCue(cue),/Rychlost/);
    for(const vy of [.09,1.01]){cue.targets[0].velocity=[0,vy,0];assert.throws(()=>planCue(cue));}
    for(const launch of [{apexSeconds:.38,gravity:8},{apexSeconds:.46,gravity:10.5}]){
        const c=structuredClone(plan.cues[0]);c.launch=launch;c.targets.forEach(t=>t.position[1]=1.5);
        assert.equal(planCue(c)[0].gravity,launch.gravity);
    }
});

// Preserve the spatial show while giving players twice the physical reaction time.
test('arcade flight doubles the strike window without changing the throw path',()=>{
    for(const cue of plan.cues){
        assert.equal(cue.launch.apexSeconds,.84);
        assert.equal(cue.launch.gravity,2.4525);
        for(const recipe of planCue(cue)){
            const within=2*Math.sqrt(2*.15/recipe.gravity);
            assert.ok(within>.69,'at least 690 ms within 15 cm of the apex');
        }
    }
});
