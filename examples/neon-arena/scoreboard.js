/**
 * Save the final Rust-owned round score to the notebook server.
 * @param {EventTarget & {readonly roundState: {finished: boolean, score: number}|null}} xr
 * @param {{fetcher?: typeof globalThis.fetch, onStatus?: (text: string) => void, onScores?: (scores: number[]) => void}} [options]
 */
export function installScoreboard(xr, {fetcher = globalThis.fetch, onStatus = () => {}, onScores} = {}) {
    let disposed=false;
    const finished = () => {
        const round = xr.roundState;
        if (!round?.finished || !Number.isSafeInteger(round.score) || round.score < 0) return;
        onStatus('Ukládám skóre…');
        void Promise.resolve().then(() => fetcher('/__neon_scores', {
            method: 'POST', headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({score: round.score}),
        })).then(async response => {
            if (disposed) return;
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            if (onScores) {
                const scores=validateScores(await response.json());
                if (disposed) return;
                onScores(scores);
            }
            onStatus('Skóre uloženo na notebooku.');
        }).catch(() => {if(!disposed)onStatus('Skóre se nepodařilo uložit na notebooku.');});
    };
    xr.addEventListener('roundend', finished);
    return () => {disposed=true;xr.removeEventListener('roundend', finished);};
}

/** @param {unknown} scores @returns {number[]} */
function validateScores(scores) {
    if (!Array.isArray(scores) || !scores.every(score => Number.isSafeInteger(score) && score >= 0)) throw new Error('Invalid scoreboard');
    return scores.sort((a,b)=>b-a);
}

/** @param {typeof globalThis.fetch} [fetcher] */
export async function loadScoreboard(fetcher = globalThis.fetch) {
    const response = await fetcher('/__neon_scores', {cache: 'no-store'});
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return validateScores(await response.json());
}

// New season: old anonymous scores intentionally remain in their original store.
export const LEADERBOARD_STORAGE_KEY = 'neon-leaderboard-v2';
const PENDING_STORAGE_KEY = `${LEADERBOARD_STORAGE_KEY}-pending`;
/** @typedef {{score:number,initials:string}} LeaderboardEntry */
/** @typedef {LeaderboardEntry & {requestId:string}} ScoreReceipt */
const LEADERBOARD_URL = '/__neon_leaderboard_v2';
function availableStorage(){try{return globalThis.localStorage;}catch{return undefined;}}

/** @param {unknown} entries @returns {{score:number,initials:string}[]} */
export function validateLeaderboard(entries) {
    if (!Array.isArray(entries) || entries.length > 10 || !entries.every(entry =>
        entry && Number.isSafeInteger(entry.score) && entry.score > 0 && typeof entry.initials==='string'
        && entry.initials.length>=1 && entry.initials.length<=3 && !/[^A-Z]/.test(entry.initials))) {
        throw new Error('Invalid leaderboard');
    }
    return entries.map(({score,initials})=>({score,initials})).sort((a,b)=>b.score-a.score);
}

/** @param {Storage|undefined} storage @returns {ScoreReceipt[]} */
function pendingReceipts(storage) {
    try {
        const receipts=JSON.parse(storage?.getItem(PENDING_STORAGE_KEY)??'[]');
        if(!Array.isArray(receipts))return [];
        return receipts.filter(receipt=>{
            try{return typeof receipt.requestId==='string'&&/^[A-Za-z0-9:._-]{1,128}$/.test(receipt.requestId)
                &&receipt.requestId.trim()===receipt.requestId&&validateLeaderboard([receipt]).length===1;}
            catch{return false;}
        });
    }catch{return [];}
}
/** @param {Storage|undefined} storage @param {ScoreReceipt[]} receipts */
function savePending(storage,receipts) {
    try{storage?.setItem(PENDING_STORAGE_KEY,JSON.stringify(receipts));}catch{}
}
/** @param {Storage|undefined} storage @param {string} requestId */
function acknowledge(storage,requestId) {
    savePending(storage,pendingReceipts(storage).filter(receipt=>receipt.requestId!==requestId));
}
/** @param {typeof globalThis.fetch} fetcher @param {ScoreReceipt} receipt */
async function submitEntry(fetcher,receipt) {
    const response=await fetcher(LEADERBOARD_URL,{
        method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(receipt),
    });
    if(!response.ok)throw new Error(`HTTP ${response.status}`);
    return validateLeaderboard(await response.json());
}
/** Retry durable confirmations before accepting the server's leaderboard.
 * @param {typeof globalThis.fetch} [fetcher] @param {Storage|undefined} [storage]
 */
export async function loadLeaderboard(fetcher = globalThis.fetch,storage=availableStorage()) {
    for(const receipt of pendingReceipts(storage)) {
        await submitEntry(fetcher,receipt);
        acknowledge(storage,receipt.requestId);
    }
    const response = await fetcher(LEADERBOARD_URL, {cache:'no-store'});
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return validateLeaderboard(await response.json());
}

/** Persist only a Rust-confirmed initials entry, never the round-end preview.
 * @param {EventTarget & {state: {scoreEntrySequence?:number,scoreEntryPending?:boolean,lastScoreEntry?:LeaderboardEntry|null}|null}} bridge
 * @param {{fetcher?:typeof globalThis.fetch,onScores?:(entries:LeaderboardEntry[])=>void,onStatus?:(message:string)=>void,sessionId?:string,storage?:Storage}} [options]
 */
export function installNamedScoreboard(bridge, {fetcher=globalThis.fetch,onScores=()=>{},onStatus=()=>{},storage=availableStorage(),
    sessionId=globalThis.crypto?.randomUUID?.()??`${Date.now()}-${Math.random().toString(36).slice(2)}`}={}) {
    let disposed=false, sequence=bridge.state?.scoreEntrySequence??0, queue=Promise.resolve();
    const changed=()=>{
        const state=bridge.state, next=state?.scoreEntrySequence??0;
        if(next===sequence||!state)return;
        sequence=next;
        const entry=validateLeaderboard([state.lastScoreEntry])[0];
        const receipt={...entry,requestId:`${sessionId}:${next}`};
        savePending(storage,[...pendingReceipts(storage),receipt]);
        onStatus('Ukládám skóre…');
        queue=queue.then(async()=>{
            if(disposed)return;
            const entries=await submitEntry(fetcher,receipt);
            acknowledge(storage,receipt.requestId);
            if(disposed)return;
            // A response must not replace a newer local confirmation or open initials form.
            if((bridge.state?.scoreEntrySequence??0)===next&&!bridge.state?.scoreEntryPending&&pendingReceipts(storage).length===0)onScores(entries);
            onStatus('Skóre uloženo na notebooku.');
        }).catch(()=>{if(!disposed)onStatus('Skóre je uložené v prohlížeči; odeslání zopakujeme při příštím načtení.');});
    };
    bridge.addEventListener('state',changed);
    return ()=>{disposed=true;bridge.removeEventListener('state',changed);};
}
