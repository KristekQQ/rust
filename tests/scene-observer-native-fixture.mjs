// Synthetic UDP source for repository acceptance only. Never a Quest result.
import dgram from 'node:dgram';
export async function startNativePoseFixture({port,token,clockOffsetMs=0}){
    const socket=dgram.createSocket('udp4'),secret=Buffer.from(token,'hex');
    await new Promise(resolve=>socket.bind(0,'127.0.0.1',resolve));
    socket.on('message',(bytes,from)=>{
        if(bytes.length!==28||bytes.toString('ascii',0,4)!=='NPP1')return;
        const reply=Buffer.alloc(36);reply.write('NPA1');secret.copy(reply,4);bytes.copy(reply,20,20,28);
        reply.writeBigUInt64LE(BigInt(Date.now()+clockOffsetMs)*1000n,28);socket.send(reply,from.port,from.address);
    });
    let sequence=0,paused=false;
    const send=()=>{
        if(paused)return;
        const packet=Buffer.alloc(120),t=++sequence/30;packet.write('NPO1');secret.copy(packet,4);
        packet.writeBigUInt64LE(100n,20);packet.writeBigUInt64LE(BigInt(sequence),28);packet.writeBigUInt64LE(BigInt(Date.now()+clockOffsetMs)*1000n,36);packet.writeUInt32LE(7,44);
        const poses=[[Math.sin(t)*.1,1.6,0,0,Math.sin(t)*.4,0],[-.4,1.2+Math.sin(t*3)*.2,-.4,.4,0,0],[.4,1.2+Math.cos(t*3)*.2,-.4,.6,0,0]];
        for(let i=0;i<3;i++)for(let j=0;j<6;j++)packet.writeFloatLE(poses[i][j],48+i*24+j*4);
        socket.send(packet,port,'127.0.0.1');
    };
    const timer=setInterval(send,1000/30);send();
    return {pause(value){paused=value;},close:async()=>{clearInterval(timer);await new Promise(resolve=>socket.close(resolve));}};
}
