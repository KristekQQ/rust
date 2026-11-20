// Deterministic packing of approved sprite cells; no generated animation frames.
import {PNG} from 'pngjs';
import {readFileSync,writeFileSync} from 'node:fs';
const [frontPath,topPath,outPath]=process.argv.slice(2);
if(!outPath)throw new Error('Usage: node scripts/pack-cat-view-atlas.mjs FRONT TOP OUTPUT');
const front=PNG.sync.read(readFileSync(frontPath)),top=PNG.sync.read(readFileSync(topPath));
const cell=320;
if(front.width!==front.height||top.width/3!==top.height/2)throw new Error('Unexpected atlas layout');
const out=new PNG({width:cell*4,height:cell*8});
for(let y=0;y<cell*4;y++)for(let x=0;x<cell*4;x++){
 const sx=Math.min(front.width-1,Math.floor((x+.5)*front.width/(cell*4)));
 const sy=Math.min(front.height-1,Math.floor((y+.5)*front.height/(cell*4)));
 const a=(sy*front.width+sx)*4,b=(y*out.width+x)*4;
 front.data.copy(out.data,b,a,a+4);
}
const poses=[0,0,0,0,0,0,0,0,1,1,2,2,3,4,4,5];
const sourceCell=top.width/3;
for(let i=0;i<16;i++){
 const pose=poses[i],ox=(i%4)*cell,oy=(4+Math.floor(i/4))*cell;
 for(let y=0;y<cell;y++)for(let x=0;x<cell;x++){
  const sx=(pose%3)*sourceCell+Math.min(sourceCell-1,Math.floor((x+.5)*sourceCell/cell));
  const sy=Math.floor(pose/3)*sourceCell+Math.min(sourceCell-1,Math.floor((y+.5)*sourceCell/cell));
  const a=(sy*top.width+sx)*4,b=((oy+y)*out.width+ox+x)*4;
  top.data.copy(out.data,b,a,a+4);
 }
}
writeFileSync(outPath,PNG.sync.write(out));
console.log(JSON.stringify({width:out.width,height:out.height,frontFrames:16,topPoseSequence:poses}));
