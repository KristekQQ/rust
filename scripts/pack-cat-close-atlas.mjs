// Keep 2048x2048 allocation: deduplicate walking poses to reserve 512px close artwork.
import {PNG} from 'pngjs';
import {readFileSync,writeFileSync} from 'node:fs';
const [currentPath,closePath,outPath,bandsPath]=process.argv.slice(2);
if(!bandsPath)throw Error('Usage: CURRENT_8x8 CLOSE_3x2 OUTPUT BANDS_JSON');
const current=PNG.sync.read(readFileSync(currentPath)),close=PNG.sync.read(readFileSync(closePath));
if(current.width!==2048||current.height!==2048||close.width!==1536||close.height!==1024)throw Error('Unexpected source dimensions');
const out=new PNG({width:2048,height:2048});
function copy(image,sx,sy,size,dx,dy){for(let y=0;y<size;y++)image.data.copy(out.data,((dy+y)*out.width+dx)*4,((sy+y)*image.width+sx)*4,((sy+y)*image.width+sx+size)*4);}
const small=[...Array.from({length:16},(_,i)=>i),...Array.from({length:4},(_,i)=>32+i),...Array.from({length:4},(_,i)=>40+i),...Array.from({length:4},(_,i)=>48+i),...Array.from({length:4},(_,i)=>56+i)];
small.forEach((frame,i)=>copy(current,(frame%8)*256,Math.floor(frame/8)*256,256,(i%8)*256,Math.floor(i/8)*256));
for(let i=0;i<6;i++)copy(close,(i%3)*512,Math.floor(i/3)*512,512,(i%4)*512,1024+Math.floor(i/4)*512);
const bands=[
 {firstFrame:0,frameCount:16,columns:8,origin:[0,0],tileSize:[.125,.125]},
 {firstFrame:16,frameCount:8,columns:4,origin:[0,.5],tileSize:[.25,.25],repeat:1},
 {firstFrame:24,frameCount:8,columns:4,origin:[0,.5],tileSize:[.25,.25],tileMap:[1,1,2,2,3,4,4,5]},
 ...[32,40,48,56].map((firstFrame,i)=>({firstFrame,frameCount:8,columns:8,origin:[0,0],tileSize:[.125,.125],firstTile:16+i*4,repeat:4}))
];
writeFileSync(outPath,PNG.sync.write(out));writeFileSync(bandsPath,JSON.stringify(bands,null,2)+'\n');
console.log(JSON.stringify({width:2048,height:2048,rgbaBytes:out.data.length,walkingPixels:256,closePixels:512,uniqueCloseFrames:6,bands:bands.length}));
