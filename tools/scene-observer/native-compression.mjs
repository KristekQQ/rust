import {inflateSync} from 'node:zlib';

// Optional lossless wrapper around one complete NDP1 packet. Never advance the
// delta base until decompression and ordinary scene validation have succeeded.
export function unwrapNativePacket(payload, maxBytes) {
  if (payload.length < 4 || Buffer.from(payload.subarray(0,4)).toString('latin1') !== 'NZP1') return payload;
  if (payload.length < 9 || payload.length > maxBytes + 24) throw Error('Compressed scene envelope budget');
  const length = new DataView(payload.buffer,payload.byteOffset,payload.byteLength).getUint32(4,true);
  if (length < 24 || length > maxBytes + 24) throw Error('Compressed scene decoded budget');
  const input = payload.subarray(8);
  const result = inflateSync(input,{maxOutputLength:length,info:true});
  if (result.buffer.length !== length || result.engine.bytesWritten !== input.length ||
      result.buffer.toString('latin1',0,4) !== 'NDP1') throw Error('Invalid compressed scene payload');
  return new Uint8Array(result.buffer.buffer,result.buffer.byteOffset,result.buffer.byteLength);
}
