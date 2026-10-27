// Browser-safe: logical positions are independent of this insertion permutation.
export function insertionOrder(count,order='spatial',seed=1337){
 if(!Number.isInteger(count)||count<1||count>65536||!['spatial','shuffled','alternating'].includes(order)||!Number.isInteger(seed)||seed<0||seed>0xffffffff)throw Error('Invalid insertion order');
 const ids=Array.from({length:count},(_,i)=>i);if(order==='alternating')return ids.map((_,i)=>i%2?count-1-Math.floor(i/2):i/2);
 if(order==='shuffled'){let state=seed>>>0;for(let i=count-1;i>0;i--){state=(Math.imul(state,1664525)+1013904223)>>>0;const j=Math.floor(state/4294967296*(i+1));[ids[i],ids[j]]=[ids[j],ids[i]];}}return ids;
}
