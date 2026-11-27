// Seconds are relative to the first audible sample. The last passage leads back
// into the opening so a longer DJ mix can keep playing for as many laps as needed.
const concertCues=[
 [0,2,'POLÁRNÍ ZÁŘE'],[20,5,'LOM'],[39,4,'CHROM'],[58,5,'TAVENÍ'],
 [77,6,'ŽIVÝ ÚTES'],[97,5,'VÍR'],[163,3,'LASERY'],[182,5,'LASTURA'],
 [201,1,'DUHOVÝ TUNEL']
];
function concertCue(time){
 const t=time%221;
 const index=concertCues.findLastIndex(cue=>cue[0]<=t);
 const cue=concertCues[index],next=concertCues[(index+1)%concertCues.length];
 const end=next[0]||221,fade=3;
 const blend=Math.max(0,1-(end-t)/fade);
 const external=cue[1]===5?cue:blend>0 && next[1]===5?next:null;
 return {from:cue[1],to:next[1],label:cue[2],blend,external:external?.[2],
  age:external?t-external[0]+fade:0};
}
// Plan one pass over the decoded song, using its own transients and phrase energy.
function createMusicConcert(groove,duration){
 const lengths=[22,20,20,20,20,17,60,20,22,19];
 const scale=duration/240,starts=[0];
 let elapsed=0;
 for(const length of lengths.slice(0,-1)){
  elapsed+=length*scale;
  let start=elapsed,highest=-Infinity;
  const radius=1.7*scale;
  for(let n=-32;groove && n<=32;n++){
   const t=elapsed+radius*n/32,g=groove(t);
   const score=g.impact+.48*g.kick+.22*g.snare-.12*Math.abs(n)/32;
   if(score>highest){highest=score;start=t;}
  }
  starts.push(start);
 }
 const slots=starts.map((start,i)=>{
  const end=starts[i+1]??duration;
  let drive=0;
  for(let j=0;groove && j<72;j++){
   const g=groove(start+(end-start)*(j+.5)/72);
   drive+=2*g.energy+.50*g.kick+.29*g.snare+.13*g.hat;
  }
  return {start,end,drive,fade:Math.min(3.3,(end-start)*.18)};
 });
 function assign(indices,scenes){
  indices.sort((a,b)=>slots[a].drive-slots[b].drive);
  indices.forEach((i,n)=>{[slots[i].id,slots[i].label]=scenes[n];});
 }
 assign([0],[[3,'LASERY']]);
 assign([6],[[5,'VÍR']]);
 assign([1,3,8],[[5,'LASTURA'],[5,'TAVENÍ'],[5,'LOM']]);
 assign([2,4,5,7,9],[[2,'POLÁRNÍ ZÁŘE'],[7,'PERLEŤOVÝ FRAKTÁL'],[4,'CHROM'],[6,'ŽIVÝ ÚTES'],[1,'DUHOVÝ TUNEL']]);
 return time=>{
  const t=time%duration,index=slots.findLastIndex(s=>s.start<=t);
  const cur=slots[index],next=slots[(index+1)%slots.length];
  const blend=Math.max(0,1-(cur.end-t)/cur.fade);
  const ext=cur.id===5?cur:blend>0 && next.id===5?next:null;
  let age=0;
  if(ext){
   const ix=cur===ext?index:(index+1)%slots.length;
   const preroll=slots[(ix+slots.length-1)%slots.length].fade;
   const fraction=Math.max(0,(t-ext.start+preroll)/(ext.end-ext.start+preroll));
   // Keep the physical eye entry, the teeth and the full crystal ritual together.
   age=fraction*(ext.label==='VÍR'?71:ext.label==='LASTURA'?30:ext.end-ext.start+preroll);
  }
  const label=cur.label==='LASTURA' && age<15?'VÝBOJ':cur.label;
  return {from:cur.id,to:next.id,label,blend,external:ext?.label,age,
   arrival:index===0 && time<duration?4:Math.min(4,(t-cur.start)*2)};
 };
}
// A continuous round trip. Pick each takeoff and arrival on a kick in the track.
function createPassageClock(groove){
 const scenes=[
  {id:4,label:'CHROM',hold:16,flight:2.6},
  {id:1,label:'DUHOVÝ TUNEL',hold:6,flight:2.6},
  {id:5,label:'VÍR',hold:34,flight:1.8},
  {id:5,label:'ČELISTI / LOM',hold:6,flight:2.8}
 ];
 const legs=[];
 let end=0;
 return time=>{
  while(end<=time){
   const index=legs.length%scenes.length,start=end,scene=scenes[index];
   const depart=nextMusicAccent(groove,start+scene.hold);
   end=nextMusicAccent(groove,depart+scene.flight);
   legs.push({index,start,depart,end});
  }
  const leg=legs.findLast(leg=>leg.start<=time);
  const {id:from,label}=scenes[leg.index],to=scenes[(leg.index+1)%scenes.length].id;
  const travel=Math.max(0,(time-leg.depart)/(leg.end-leg.depart));
  const climbing=leg.index===2,jaws=leg.index===3;
  const external=from===5 || (to===5 && travel>0) ? 'VÍR' : null;
  // The original vortex shader projects LOM into the roof opening and becomes
  // the same LOM view once its camera crosses the lip. Keep that camera in charge.
  const orbit=Math.min(1,Math.max(0,(time-leg.start)/(leg.depart-leg.start)));
  const rail=climbing ? 6.85*orbit+4.55*travel : 0;
  return {from,to,label,external,rail,jaws,blend:climbing?0:travel,
   arrival:jaws || leg.start===0?4:Math.min(4,(time-leg.start)*2)};
 };
}
async function prepareConcert(gl,makeProgram,vertex300,stormCode){
 const [meshPage,shellFile]=await Promise.all(['../lom/index.html','../vyboj/lastura.png'].map(async url=>{
  const response=await fetch(url);
  if(!response.ok)throw Error('Nepodařilo se načíst '+url);
  return response;
 }));
 const meshDoc=new DOMParser().parseFromString(await meshPage.text(),'text/html');
 const mesh=makeProgram(meshDoc.getElementById('frag').textContent,['size','time','travel','impact','band'],meshDoc.getElementById('vert').textContent);
 const vao=gl.createVertexArray();
 gl.bindVertexArray(vao);
 const meshCount=bindLomMesh(gl,mesh.program);
 gl.bindVertexArray(null);
 const depth=gl.createRenderbuffer();
 let depthW=0,depthH=0,travel=0,lastAge=-1;
 const shell=makeProgram(`#version 300 es
precision highp float;
precision highp int;
#define VYBOJ_CAMERA_INPUT
uniform float iTime,uShellReadyAt;
uniform vec3 iResolution;
uniform vec4 uOrbit;
uniform highp sampler2D iChannel0,iChannel1;
${stormCode}
out vec4 result;void main(){mainImage(result,gl_FragCoord.xy);}`,['iTime','iResolution','uOrbit','iChannel0','iChannel1','uShellReadyAt'],vertex300);
 const texture=gl.createTexture(),atlas=await createImageBitmap(await shellFile.blob(),{colorSpaceConversion:'none',premultiplyAlpha:'none',imageOrientation:'none'});
 gl.bindTexture(gl.TEXTURE_2D,texture);
 for(const key of [gl.TEXTURE_MIN_FILTER,gl.TEXTURE_MAG_FILTER])gl.texParameteri(gl.TEXTURE_2D,key,gl.NEAREST);
 gl.texImage2D(gl.TEXTURE_2D,0,gl.RGB8,gl.RGB,gl.UNSIGNED_BYTE,atlas);
 atlas.close();
 function drawMesh(target,age,bands,kick){
  const {width:w,height:h}=target,u=mesh.uniforms;
  gl.useProgram(mesh.program);gl.bindVertexArray(vao);
  gl.bindRenderbuffer(gl.RENDERBUFFER,depth);
  if(w!==depthW||h!==depthH){
   gl.renderbufferStorage(gl.RENDERBUFFER,gl.DEPTH_COMPONENT16,w,h);
   depthW=w;depthH=h;
  }
  gl.framebufferRenderbuffer(gl.FRAMEBUFFER,gl.DEPTH_ATTACHMENT,gl.RENDERBUFFER,depth);
  if(age<lastAge||lastAge<0)travel=0;
  else travel+=Math.min(.25,age-lastAge)*(9.2+5.2*bands[0]+15.3*kick);
  lastAge=age;
  gl.enable(gl.DEPTH_TEST);gl.clearColor(.18,.047,.12,1);
  gl.clear(gl.COLOR_BUFFER_BIT|gl.DEPTH_BUFFER_BIT);
  gl.uniform2f(u.size,w,h);gl.uniform1f(u.time,age);gl.uniform1f(u.travel,travel);
  gl.uniform1f(u.impact,kick);gl.uniform3fv(u.band,bands);
  gl.drawArrays(gl.TRIANGLES,0,meshCount);
  gl.disable(gl.DEPTH_TEST);gl.bindVertexArray(null);
  gl.framebufferRenderbuffer(gl.FRAMEBUFFER,gl.DEPTH_ATTACHMENT,gl.RENDERBUFFER,null);
 }
 return {mesh,drawMesh,shell,texture};
}
