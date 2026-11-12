import dgram from 'node:dgram';
import {randomBytes,timingSafeEqual} from 'node:crypto';

// NPO1: token16, boot/session u64, seq u64, sample epoch us u64, tracked bits u32,
// head/left/right each position3 + Euler [x,y,z]3, all little endian; 120 bytes.
export function decodeNativePose(bytes,token){
    if(bytes.length!==120||bytes.toString('ascii',0,4)!=='NPO1'||!timingSafeEqual(bytes.subarray(4,20),token))throw Error('Invalid native pose packet');
    const session=bytes.readBigUInt64LE(20),sequence=bytes.readBigUInt64LE(28),sampledAt=Number(bytes.readBigUInt64LE(36))/1000,flags=bytes.readUInt32LE(44);
    if(!session||!sequence||flags>7||!Number.isFinite(sampledAt)||sampledAt<=0)throw Error('Invalid native pose header');
    const poses=[];
    for(let i=0;i<3;i++){
        const values=Array.from({length:6},(_,j)=>bytes.readFloatLE(48+i*24+j*4));
        if(!values.every(Number.isFinite)||values.slice(0,3).some(v=>Math.abs(v)>100))throw Error('Invalid native pose transform');
        poses.push(flags&(1<<i)?{position:values.slice(0,3),rotation:values.slice(3)}:null);
    }
    return {session,sequence,sampledAt,poses};
}
export async function createNativePoseBridge({port=8792,host='0.0.0.0',token:configuredToken,onFrame=()=>{}}={}){
    const token=configuredToken?Buffer.from(configuredToken,'hex'):randomBytes(16);
    if(token.length!==16)throw Error('Native observer token must be 16 bytes');
    const socket=dgram.createSocket('udp4');
    let latestRaw=null,latest=null,remote=null,session=0n,sequence=0n,offset=null,uncertainty=Infinity,calibratedAt=0,closed=false;
    const pending=new Map();
    const counters={accepted:0,rejected:0,reordered:0,clockSamples:0};
    const emit=()=>{
        if(!latestRaw||offset===null)return;
        const [head,left,right]=latestRaw.poses;
        latest={session:'native-'+latestRaw.session,sequence:latestRaw.sequence.toString(),mode:'live',sampledAt:latestRaw.sampledAt-offset,
            clockUncertaintyMs:uncertainty,state:{head,left,right,swords:Number(!!left)+Number(!!right)},native:true};
        onFrame(latest);
    };
    const ping=()=>{
        if(!remote||closed)return;
        const now=Date.now(),stamp=BigInt(now)*1000n,packet=Buffer.alloc(28);
        packet.write('NPP1');token.copy(packet,4);packet.writeBigUInt64LE(stamp,20);
        pending.set(stamp.toString(),now);while(pending.size>16)pending.delete(pending.keys().next().value);
        socket.send(packet,remote.port,remote.address,()=>{});
    };
    socket.on('error',()=>{counters.rejected++;});
    socket.on('message',(bytes,from)=>{
        try{
            if(bytes.length===36&&bytes.toString('ascii',0,4)==='NPA1'){
                if(!remote||remote.address!==from.address||remote.port!==from.port||!timingSafeEqual(bytes.subarray(4,20),token))throw Error('Invalid native clock response');
                const key=bytes.readBigUInt64LE(20).toString(),sent=pending.get(key);if(sent===undefined)return;pending.delete(key);
                const now=Date.now(),rtt=now-sent;if(rtt<0||rtt>1000)return;
                if(offset===null||rtt/2+.5<=uncertainty||now-calibratedAt>15000){
                    const deviceAt=Number(bytes.readBigUInt64LE(28))/1000;
                    offset=deviceAt-(sent+now)/2;uncertainty=rtt/2+.5;calibratedAt=now;counters.clockSamples++;emit();
                }
                return;
            }
            const frame=decodeNativePose(bytes,token);
            if(frame.session<session||(frame.session===session&&frame.sequence<=sequence)){counters.reordered++;return;}
            if(frame.session!==session){session=frame.session;sequence=0n;offset=null;uncertainty=Infinity;pending.clear();}
            sequence=frame.sequence;latestRaw=frame;remote={address:from.address,port:from.port};counters.accepted++;
            if(offset===null)ping();else emit();
        }catch{counters.rejected++;}
    });
    await new Promise((resolve,reject)=>{socket.once('error',reject);socket.bind(port,host,resolve);});
    const timer=setInterval(ping,5000);timer.unref();
    return {normalizeTimestamp:deviceAt=>offset===null?null:{sampledAt:deviceAt-offset,clockUncertaintyMs:uncertainty},get latest(){return latest;},get port(){return socket.address().port;},token:token.toString('hex'),
        status:()=>({port:socket.address().port,connected:!!latest&&Date.now()-latest.sampledAt<1000,session:session.toString(),sequence:sequence.toString(),uncertaintyMs:Number.isFinite(uncertainty)?uncertainty:null,...counters}),
        close:async()=>{closed=true;clearInterval(timer);await new Promise(resolve=>socket.close(resolve));}};
}
