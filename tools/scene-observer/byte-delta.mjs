// Exact binary transport patches; scene interpretation remains in Rust/NWR.
export const BYTE_LIMIT=32*1024*1024+65540;
const MAGIC=[78,68,80,49],HEADER=24,BLOCK=64;
const hasMagic=b=>b.length>=4&&MAGIC.every((v,i)=>b[i]===v);
export const isByteDelta=hasMagic;
const u32=(v,o,n)=>v.setUint32(o,n,true);
export function encodeByteDelta(previous,bytes){
 if(!(bytes instanceof Uint8Array)||bytes.length>BYTE_LIMIT)throw Error('Delta target budget');
 const sequence=(previous?.sequence??0)+1;if(sequence>0xffffffff)throw Error('Delta sequence exhausted');
 const base=previous?.bytes,runs=[];let patchSize=HEADER;
 if(base)for(let offset=0;offset<bytes.length;offset+=BLOCK){
  const end=Math.min(offset+BLOCK,bytes.length);let changed=end>base.length;
  for(let p=offset;!changed&&p<end;p++)changed=bytes[p]!==base[p];
  if(!changed)continue;
  const last=runs.at(-1);
  if(last&&last[0]+last[1]===offset)last[1]=end-last[0];else{runs.push([offset,end-offset]);patchSize+=8;}
  patchSize+=end-offset;
 }
 const patch=!!base&&patchSize<HEADER+bytes.length,size=patch?patchSize:HEADER+bytes.length;
 const payload=new Uint8Array(size),v=new DataView(payload.buffer);payload.set(MAGIC);payload[4]=Number(patch);
 u32(v,8,sequence);u32(v,12,patch?previous.sequence:0);u32(v,16,bytes.length);u32(v,20,patch?runs.length:0);
 if(patch){let at=HEADER;for(const [offset,n]of runs){u32(v,at,offset);u32(v,at+4,n);payload.set(bytes.subarray(offset,offset+n),at+8);at+=8+n;}}
 else payload.set(bytes,HEADER);
 return {sequence,payload,patch};
}
export function decodeByteDelta(previous,payload,{maxBytes=BYTE_LIMIT}={}){
 if(!(payload instanceof Uint8Array)||!hasMagic(payload)||payload.length<HEADER||payload.length>maxBytes+HEADER)throw Error('Invalid delta envelope');
 const v=new DataView(payload.buffer,payload.byteOffset,payload.byteLength),mode=payload[4],sequence=v.getUint32(8,true),baseSeq=v.getUint32(12,true),length=v.getUint32(16,true),count=v.getUint32(20,true);
 if(payload[5]||payload[6]||payload[7]||mode>1||!sequence||length>maxBytes||sequence!==(previous?.sequence??0)+1)throw Error('Invalid delta header/sequence');
 if(mode===0){if(baseSeq||count||payload.length!==HEADER+length)throw Error('Invalid full delta frame');return {sequence,bytes:payload.slice(HEADER)};}
 if(!previous||baseSeq!==previous.sequence||count>Math.ceil(length/BLOCK))throw Error('Invalid delta base/runs');
 const bytes=new Uint8Array(length);bytes.set(previous.bytes.subarray(0,length));let at=HEADER,lastEnd=0;
 for(let i=0;i<count;i++){
  if(at+8>payload.length)throw Error('Truncated delta run');
  const offset=v.getUint32(at,true),n=v.getUint32(at+4,true);at+=8;
  if(!n||offset<lastEnd||offset+n>length||at+n>payload.length)throw Error('Invalid delta range');
  bytes.set(payload.subarray(at,at+n),offset);at+=n;lastEnd=offset+n;
 }
 if(at!==payload.length)throw Error('Trailing delta bytes');
 return {sequence,bytes};
}
