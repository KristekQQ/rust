// Authoring-only packing. Five walking directions share one runtime texture.
import {PNG} from 'pngjs';
import {readFileSync,writeFileSync} from 'node:fs';
const [existingPath,directionsPath,outPath]=process.argv.slice(2);
if(!outPath)throw Error('Usage: node scripts/pack-cat-direction-atlas.mjs EXISTING_4x8 DIRECTIONS_4x4 OUTPUT');
const old=PNG.sync.read(readFileSync(existingPath)),directions=PNG.sync.read(readFileSync(directionsPath));
const cell=256,out=new PNG({width:2048,height:2048});
function copyCell(image,cols,rows,index,dest){
 const sw=image.width/cols,sh=image.height/rows,sx=(index%cols)*sw,sy=Math.floor(index/cols)*sh;
 const dx=(dest%8)*cell,dy=Math.floor(dest/8)*cell;
 for(let y=0;y<cell;y++)for(let x=0;x<cell;x++){
  const ix=Math.min(image.width-1,Math.floor(sx+(x+.5)*sw/cell)),iy=Math.min(image.height-1,Math.floor(sy+(y+.5)*sh/cell));
  const a=(iy*image.width+ix)*4,b=((dy+y)*out.width+dx+x)*4;
  image.data.copy(out.data,b,a,a+4);
 }
}
for(let i=0;i<32;i++)copyCell(old,4,8,i,i);
// Four genuine gait poses repeat twice in each eight-frame semantic cycle.
for(let band=0;band<4;band++)for(let frame=0;frame<8;frame++)copyCell(directions,4,4,band*4+frame%4,32+band*8+frame);
writeFileSync(outPath,PNG.sync.write(out));
console.log(JSON.stringify({width:out.width,height:out.height,cells:64,rgbaBytes:out.data.length,walkingDirections:5,uniqueNewGaitPosesPerDirection:4}));
