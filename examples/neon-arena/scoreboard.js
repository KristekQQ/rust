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
