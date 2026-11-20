// Authoring only: intact isolated cat poses, never donor face/mouth compositing.
import {PNG} from 'pngjs';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import path from 'node:path';
const [mode,...args]=process.argv.slice(2);
const read=file=>PNG.sync.read(readFileSync(file));
const write=(file,png)=>writeFileSync(file,PNG.sync.write(png));
const name=(kind,index)=>`${kind}-${String(index).padStart(2,'0')}.png`;
function crop(source,x,y,size){
 const out=new PNG({width:size,height:size});
 for(let row=0;row<size;row++)source.data.copy(out.data,row*size*4,((y+row)*source.width+x)*4,((y+row)*source.width+x+size)*4);
 // Remove low-alpha source debris only inside a transparent four-pixel guard.
 // No visible pixel, subject position, proportions or face is changed.
 for(let v=0;v<size;v++)for(let u=0;u<size;u++)if(Math.min(u,v,size-1-u,size-1-v)<4){
  const at=(v*size+u)*4;
  if(out.data[at+3]>=90)throw Error('Visible geometry enters guard gutter');
  out.data.fill(0,at,at+4);
 }
 return out;
}
if(mode==='extract'){
 const [sourcePath,directory]=args;if(!directory)throw Error('extract ORIGINAL_ATLAS FRAME_DIRECTORY');
 const source=read(sourcePath);if(source.width!==2048||source.height!==2048)throw Error('Expected original 2048px atlas');
 mkdirSync(directory,{recursive:true});
 for(let i=0;i<32;i++)write(path.join(directory,name('normal',i)),crop(source,i%8*256,Math.floor(i/8)*256,256));
 for(let i=0;i<6;i++)write(path.join(directory,name('close',i)),crop(source,i%4*512,1024+Math.floor(i/4)*512,512));
 const manifest={version:1,normal:Array.from({length:32},(_,i)=>name('normal',i)),close:Array.from({length:6},(_,i)=>name('close',i)),carry:[...[0,4,16,17,20,21,8].map(i=>({source:name('normal',i),size:256})),{source:name('close',0),size:256}]};
 writeFileSync(path.join(directory,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
 console.log(JSON.stringify({normalFrames:32,closeFrames:6,carryFrames:8,donorPatches:0}));
}else if(mode==='pack'){
 const [directory,outputPath,bandsPath,previewPath,closePreviewPath]=args;if(!closePreviewPath)throw Error('pack FRAME_DIRECTORY OUTPUT BANDS_JSON CARRY_PREVIEW CLOSE_PREVIEW');
 const manifest=JSON.parse(readFileSync(path.join(directory,'manifest.json')));
 const output=new PNG({width:2048,height:2304}),preview=new PNG({width:1024,height:512}),closePreview=new PNG({width:1536,height:1024});
 function place(source,destination,x,y){for(let row=0;row<source.height;row++)source.data.copy(destination.data,((y+row)*destination.width+x)*4,row*source.width*4,(row+1)*source.width*4);}
 function checked(kind,i,size){
  const item=manifest[kind][i],source=typeof item==='string'?item:item.source;
  const image=read(path.join(directory,source));
  if(image.width===size&&image.height===size)return image;
  if(kind!=='carry'||image.width!==512||image.height!==512||size!==256)throw Error(`Wrong standalone pose size: ${source}`);
  const small=new PNG({width:256,height:256});
  for(let y=0;y<256;y++)for(let x=0;x<256;x++){const at=(y*256+x)*4,from=(y*2*512+x*2)*4;image.data.copy(small.data,at,from,from+4);}
  return small;
 }
 for(let i=0;i<32;i++)place(checked('normal',i,256),output,i%8*256,Math.floor(i/8)*256);
 for(let i=0;i<6;i++){const image=checked('close',i,512);place(image,output,i%4*512,1024+Math.floor(i/4)*512);place(image,closePreview,i%3*512,Math.floor(i/3)*512);}
 for(let i=0;i<8;i++){const image=checked('carry',i,256);place(image,output,i*256,2048);place(image,preview,i%4*256,Math.floor(i/4)*256);}
 const oldBands=[
  {firstFrame:0,frameCount:16,columns:8,origin:[0,0],tileSize:[.125,.125]},
  {firstFrame:16,frameCount:8,columns:4,origin:[0,.5],tileSize:[.25,.25],repeat:1},
  {firstFrame:24,frameCount:8,columns:4,origin:[0,.5],tileSize:[.25,.25],tileMap:[1,1,2,2,3,4,4,5]},
  ...[32,40,48,56].map((firstFrame,i)=>({firstFrame,frameCount:8,columns:8,origin:[0,0],tileSize:[.125,.125],firstTile:16+i*4,repeat:4})),
 ];
 const bands=oldBands.map(band=>({...band,origin:[band.origin[0],band.origin[1]*8/9],tileSize:[band.tileSize[0],band.tileSize[1]*8/9]}));
 bands.push({firstFrame:64,frameCount:8,columns:8,origin:[0,8/9],tileSize:[.125,1/9]});
 write(outputPath,output);write(previewPath,preview);write(closePreviewPath,closePreview);writeFileSync(bandsPath,JSON.stringify(bands));
 console.log(JSON.stringify({width:2048,height:2304,bands:8,isolatedSourceFrames:38,donorPatches:0}));
}else throw Error('Expected extract or pack');
