import {DEFAULT_WAVE,validateWave} from './wave-plan.js';
import {ARENA_STORAGE_KEY,validatePreset,makePreset} from './arena-presets.js';
/** @typedef {import('./wave-plan.js').WaveConfig} WaveConfig */
/** @typedef {{getItem:(key:string)=>string|null,setItem:(key:string,value:string)=>void,removeItem:(key:string)=>void}} PresetStorage */
/** @param {{state:()=>object,spawn:(config:WaveConfig)=>object,preview?:(seconds:number|undefined)=>object,bank?:(hand:'left'|'right')=>object,start:()=>void,stop:()=>void,changed:(previous:WaveConfig)=>void}} hooks
 * @param {{preset?:import('./arena-presets.js').ArenaPreset,storage?:PresetStorage|null}} [options] */
export function createArenaTools(hooks,options={}) {
    /** @type {WaveConfig} */
    let config={...(options.preset?.wave??DEFAULT_WAVE)};
    const defaults=validatePreset(options.preset??makePreset(DEFAULT_WAVE));
    const storage=options.storage??null;
    let presetName=defaults.name,storageError='';
    let restored=false,migratedSize=false;
    try{
        const raw=storage?.getItem(ARENA_STORAGE_KEY);
        if(raw){const saved=validatePreset(JSON.parse(raw));config=saved.wave;presetName=saved.name;restored=true;
            // Upgrade only the old built-in size; retain custom named presets and every other setting.
            if(saved.name===defaults.name&&saved.wave.fruitScale===.06&&defaults.wave.fruitScale===.09){
                config={...config,fruitScale:.09};migratedSize=true;
            }
        }
    }catch(error){storageError=`Uložená předvolba se nenačetla: ${String(error)}`;}
    const persist=(/** @type {WaveConfig} */ wave,/** @type {string} */ title)=>{
        try{
            if(!storage)throw new Error('localStorage není dostupné.');
            storage.setItem(ARENA_STORAGE_KEY,JSON.stringify(makePreset(wave,title)));
            storageError='';return true;
        }catch(error){storageError=String(error);return false;}
    };
    if(migratedSize)persist(config,presetName);
    const apply=(/** @type {WaveConfig} */ next,/** @type {string} */ title=presetName)=>{
        const previous=config;config=next;presetName=title;hooks.changed(previous);
    };
    return Object.freeze({
        get config(){return {...config};},
        /** @param {string} name @param {unknown} [args] */
        call(name,args={}) {
            switch(name){
            case 'arena_state':return {...hooks.state(),config:{...config},presetName,restored,storageError};
            case 'arena_configure':{
                const next=validateWave(args,config),saved=persist(next,presetName);apply(next);
                return {config:{...config},saved,...(saved?{}:{warning:storageError})};
            }
            case 'arena_save':{
                if(!persist(config,presetName))throw new Error(storageError);
                return {saved:true,preset:makePreset(config,presetName)};
            }
            case 'arena_export':return makePreset(config,presetName);
            case 'arena_load':{
                if(!storage)throw new Error('localStorage není dostupné.');
                const raw=storage.getItem(ARENA_STORAGE_KEY);
                if(!raw)throw new Error('Zatím není uložená předvolba.');
                const saved=validatePreset(JSON.parse(raw));apply(saved.wave,saved.name);restored=true;storageError='';
                return {loaded:true,preset:makePreset(config,presetName)};
            }
            case 'arena_reset':{
                if(!storage)throw new Error('localStorage není dostupné.');
                storage.removeItem(ARENA_STORAGE_KEY);apply({...defaults.wave},defaults.name);restored=false;storageError='';
                return {reset:true,preset:makePreset(config,presetName)};
            }
            case 'arena_import':{
                const json=/** @type {{json?:unknown}} */(args)?.json;
                if(typeof json!=='string'||json.length>24576)throw new Error('Očekávám JSON do 24 KiB.');
                const imported=validatePreset(JSON.parse(json));
                if(!persist(imported.wave,imported.name))throw new Error(storageError);
                apply(imported.wave,imported.name);restored=true;
                return {imported:true,preset:makePreset(config,presetName)};
            }
            case 'arena_preview':{
                const seconds=/** @type {{seconds?:unknown}} */(args)?.seconds;
                if(seconds!==undefined&&(typeof seconds!=='number'||!Number.isFinite(seconds)||seconds<0||seconds>=60))throw new Error('Čas náhledu musí být 0 až méně než 60 s.');
                if(!hooks.preview)throw new Error('Náhled scény není dostupný.');
                return hooks.preview(/** @type {number|undefined} */(seconds));
            }
            case 'arena_bank':{
                if(!args||typeof args!=='object'||Array.isArray(args)||Object.keys(args).some(key=>key!=='hand'))throw new Error('Očekávám jen hand: left nebo right.');
                const hand=/** @type {{hand?:unknown}} */(args).hand;
                if(hand!=='left'&&hand!=='right')throw new Error('Vyber left nebo right.');
                if(!hooks.bank)throw new Error('Zásobníky nejsou dostupné.');
                return hooks.bank(hand);
            }
            case 'arena_spawn':return hooks.spawn(validateWave(args,config));
            case 'arena_start':hooks.start();return {...hooks.state(),config:{...config}};
            case 'arena_stop':hooks.stop();return {...hooks.state(),config:{...config}};
            default:throw new Error(`Neznámý nástroj: ${name}`);
            }
        }
    });
}
/** Human panel and MCP invoke exactly the same command dispatcher.
 * @param {ReturnType<typeof createArenaTools>} tools */
export function installArenaPanel(tools) {
    const root=document.getElementById('arena-tools');
    if(!root)return ()=>{};
    const output=root.querySelector('output');
    const message=(/** @type {string} */ text)=>{if(output)output.textContent=text;};
    const read=()=>({count:Number(/** @type {HTMLInputElement} */(root.querySelector('[name=count]')).value),
        intervalMs:Number(/** @type {HTMLInputElement} */(root.querySelector('[name=intervalMs]')).value),
        formation:/** @type {HTMLSelectElement} */(root.querySelector('[name=formation]')).value,
        fruit:/** @type {HTMLSelectElement} */(root.querySelector('[name=fruit]')).value,
        automatic:/** @type {HTMLInputElement} */(root.querySelector('[name=automatic]')).checked,
        program:/** @type {HTMLSelectElement|null} */(root.querySelector('[name=program]'))?.value??tools.config.program,
        ...Object.fromEntries(['fruitScale','width','height','depth','curve','slope','lift','gravity','lifetime'].flatMap(key=>{
            const input=root.querySelector(`[name=${key}]`);return input instanceof HTMLInputElement?[[key,Number(input.value)]]:[];
        }))});
    const sync=()=>{
        for(const [key,value] of Object.entries(tools.config)){
            const input=root.querySelector(`[name=${key}]`);
            if(input instanceof HTMLInputElement&&input.type==='checkbox')input.checked=Boolean(value);
            else if(input instanceof HTMLInputElement||input instanceof HTMLSelectElement)input.value=String(value);
        }
        const count=root.querySelector('#wave-count');if(count)count.textContent=String(tools.config.count);
        const hint=root.querySelector('#program-hint');if(hint)hint.textContent=tools.config.program==='minute'?'60 sekund: střídavé nálety zespodu, trojice a finále. Zpomalený arkádový let dává čas na reakci a řez u vrcholu dráhy. Nastavení letu dole patří volným vlnám. Zásobníky drží dva druhy; trigger uloží sérii a uvolní místo.':'60 sekund volných vln podle nastaveného počtu, formace a tempa.';
    };
    root.addEventListener('change',event=>{const key=event.target instanceof Element?event.target.getAttribute('name'):null;if(!key||!Object.hasOwn(DEFAULT_WAVE,key))return;try{tools.call('arena_configure',read());sync();const state=tools.call('arena_state');message('storageError' in state&&state.storageError?`Nastavení platí, ale není uložené: ${state.storageError}`:'Nastavení uloženo v tomto prohlížeči.');}catch(error){message(String(error));sync();}});
    root.addEventListener('click',event=>{
        const button=event.target instanceof Element?event.target.closest('button[data-tool]'):null;
        if(!(button instanceof HTMLButtonElement))return;
        try{const tool=button.dataset.tool??'';const result=tools.call(tool,button.dataset.hand?{hand:button.dataset.hand}:{});
            if(tool==='arena_export'){
                const url=URL.createObjectURL(new Blob([JSON.stringify(result,null,2)+'\n'],{type:'application/json'}));
                const link=document.createElement('a');link.href=url;link.download='arena-preset.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
                message('JSON předvolby stažen.');return;
            }
            if(['arena_save','arena_load','arena_reset'].includes(tool)){message(tool==='arena_save'?'Předvolba uložena.':tool==='arena_load'?'Uložená předvolba načtena.':'Obnovena projektová předvolba.');return;}
            if(tool==='arena_bank'){message('banked' in result&&result.banked?'Zásobník uvolněn.':'Zásobník je prázdný.');return;}
            if(tool==='arena_preview'){message('label' in result?`Náhled: ${result.label}`:'Náhled scény.');return;}
            message(button.dataset.tool==='arena_spawn'?('spawned' in result?(result.spawned===0?'Předchozí ovoce je ještě ve hře.':`Posláno ${result.spawned} plodů.`):'Vlna odeslána.'):button.dataset.tool==='arena_stop'?'Další vlny zastaveny.':'Nové kolo spuštěno.');}catch(error){message(error instanceof Error?error.message:String(error));}
    });
    const importInput=root.querySelector('[data-preset-import]');
    if(importInput instanceof HTMLInputElement)importInput.addEventListener('change',async()=>{
        try{
            const file=importInput.files?.[0];if(!file)return;
            if(file.size>24576)throw new Error('JSON je příliš velký.');
            tools.call('arena_import',{json:await file.text()});message('JSON načten a uložen.');
        }catch(error){message(String(error));}finally{importInput.value='';}
    });
    sync();const initial=tools.call('arena_state');
    if('storageError' in initial&&initial.storageError)message(String(initial.storageError));
    else if('restored' in initial&&initial.restored)message('Načtena uložená předvolba.');
    return sync;
}
/** Explicit opt-in, same-origin bridge. Token remains only in page memory.
 * @param {ReturnType<typeof createArenaTools>} tools */
export function installArenaBridge(tools) {
    const button=document.getElementById('arena-connect'),input=document.getElementById('arena-token'),output=document.getElementById('arena-connection');
    let stopped=false,token='',timer=0;
    const session=crypto.randomUUID();
    const controller=new AbortController();
    const show=(/** @type {string} */ text)=>{if(output)output.textContent=text;};
    const request=async(/** @type {string} */ path,/** @type {object|undefined} */ body=undefined)=>{
        const response=await fetch(path,{method:body?'POST':'GET',headers:{Authorization:`Bearer ${token}`,...(body?{'Content-Type':'application/json'}:{})},body:body?JSON.stringify(body):undefined,signal:controller.signal});
        if(!response.ok)throw new Error(`MCP bridge: HTTP ${response.status}`);
        return response.status===204?{}:response.json();
    };
    const poll=async()=>{
        try{
            const {commands=[]}=await request(`/__arena_tools/poll?session=${encodeURIComponent(session)}`);
            for(const command of commands){
                let result,error;
                try{
                    if(!Number.isFinite(command.expiresAt)||Date.now()>=command.expiresAt)throw new Error('Příkaz vypršel; nebyl proveden.');
                    result=tools.call(command.name,command.arguments??{});
                }catch(reason){error=reason instanceof Error?reason.message:String(reason);}
                await request('/__arena_tools/result',{session,id:command.id,result,error});
            }
            show(`MCP připojeno · ${session.slice(0,8)}`);
        }catch(error){if(!stopped)show(String(error));}
        finally{if(!stopped)timer=window.setTimeout(poll,250);}
    };
    if(button instanceof HTMLButtonElement)button.onclick=()=>{
        if(token)return;
        if(!(input instanceof HTMLInputElement)||!input.value.trim()){show('Zadej token spuštěného MCP serveru.');return;}
        token=input.value.trim();input.value='';button.disabled=true;void poll();
    };
    return ()=>{stopped=true;clearTimeout(timer);controller.abort();};
}
