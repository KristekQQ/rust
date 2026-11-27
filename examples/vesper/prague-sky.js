// North-facing Prague panorama. Catalogue rows: HIP, J2000 RA and Dec (deg),
// Johnson V and B−V, from ESA Hipparcos I/239/hip_main (CDS/VizieR).
async function createPragueSky(gl) {
 const response=await fetch('hipparcos.json');
 if(!response.ok)throw Error('Katalog hvězd se nepodařilo načíst.');
 const catalog=await response.json();
 const rad=Math.PI/180, latitude=50.08*rad, elevation=43*rad, focal=1/Math.tan(36*rad);
 // Precess J2000 equatorial coordinates to the equinox used by sidereal time.
 const centuries=(Date.now()/86400000-10957.5)/36525;
 const zeta=(2306.2181+.30188*centuries)*centuries*rad/3600;
 const z=(2306.2181+1.09468*centuries)*centuries*rad/3600;
 const theta=(2004.3109-.42665*centuries)*centuries*rad/3600;
 const stars=catalog.map(([hip,ra,dec,mag,bv])=>{
  const alpha=ra*rad+zeta, delta=dec*rad;
  const a=Math.cos(delta)*Math.sin(alpha);
  const b=Math.cos(theta)*Math.cos(delta)*Math.cos(alpha)-Math.sin(theta)*Math.sin(delta);
  const c=Math.sin(theta)*Math.cos(delta)*Math.cos(alpha)+Math.cos(theta)*Math.sin(delta);
  const longitude=Math.atan2(a,b)+z, cosDec=Math.sqrt(Math.max(0,1-c*c));
  return {hip,x:cosDec*Math.cos(longitude),y:cosDec*Math.sin(longitude),z:c,mag,bv};
 });
 const texture=gl.createTexture();
 gl.bindTexture(gl.TEXTURE_2D,texture);
 for(const key of [gl.TEXTURE_MIN_FILTER,gl.TEXTURE_MAG_FILTER])gl.texParameteri(gl.TEXTURE_2D,key,gl.NEAREST);
 for(const key of [gl.TEXTURE_WRAP_S,gl.TEXTURE_WRAP_T])gl.texParameteri(gl.TEXTURE_2D,key,gl.CLAMP_TO_EDGE);
 const bright=new Float32Array(12), tile=12, slots=4;
 let stamp=-1,width=0,height=0,cols=0,rows=0;
 function update(w,h,now){
  const tick=Math.floor(now/2500);
  if(stamp===tick && width===w && height===h)return;
  stamp=tick;width=w;height=h;cols=Math.ceil(w/tile);rows=Math.ceil(h/tile);
  const bytes=new Uint8Array(cols*rows*slots*4);
  const days=now/86400000-10957.5;
  const sidereal=((280.46061837+360.98564736629*days+14.44)%360)*rad;
  const sin=Math.sin(sidereal),cos=Math.cos(sidereal);
  const visible=[];
  for(const star of stars){
   const meridian=cos*star.x+sin*star.y;
   const up=Math.cos(latitude)*meridian+Math.sin(latitude)*star.z;
   const north=-Math.sin(latitude)*meridian+Math.cos(latitude)*star.z;
   const depth=north*Math.cos(elevation)+up*Math.sin(elevation);
   if(up<=0 || depth<=0)continue;
   const x=focal*(cos*star.y-sin*star.x)/depth;
   const y=focal*(up*Math.cos(elevation)-north*Math.sin(elevation))/depth;
   if(Math.abs(x)>w/h+.04 || Math.abs(y)>1.04)continue;
   const magnitude=star.mag+.16*(1/Math.max(.12,up)-1);
   visible.push({x,y,magnitude,bv:star.bv});
  }
  visible.sort((a,b)=>a.magnitude-b.magnitude);
  bright.fill(0);
  for(let i=0;i<3;i++)bright[i*4]=100;
  let featured=0;
  for(const {x,y,magnitude,bv} of visible){
   const tone=Math.max(0,Math.min(1,(bv+.32)/2.05));
   if(featured<3 && Math.abs(x)<w/h-.09 && Math.abs(y)<.91){
    bright.set([x,y,tone,magnitude],4*featured++);
    continue;
   }
   const sx=(x*h+w)*.5,sy=(y+1)*h*.5;
   // Replicate the footprint across tile boundaries. Sorted insertion keeps
   // the brightest stars if an unresolved multiple system fills a tile.
   for(let ty=Math.max(0,Math.floor((sy-4)/tile));ty<=Math.min(rows-1,Math.floor((sy+4)/tile));ty++)
    for(let tx=Math.max(0,Math.floor((sx-4)/tile));tx<=Math.min(cols-1,Math.floor((sx+4)/tile));tx++){
     let idx=4*slots*(ty*cols+tx),end=idx+4*slots;
     while(idx<end && bytes[idx+2])idx+=4;
     if(idx===end)continue;
     bytes[idx]=Math.round(((sx/tile-tx)+.5)*127.5);
     bytes[idx+1]=Math.round(((sy/tile-ty)+.5)*127.5);
     bytes[idx+2]=Math.max(1,Math.min(255,Math.round((8-magnitude)*25)));
     bytes[idx+3]=Math.round(tone*255);
    }
  }
  gl.bindTexture(gl.TEXTURE_2D,texture);
  gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,cols*slots,rows,0,gl.RGBA,gl.UNSIGNED_BYTE,bytes);
 }
 return function bindSky(u,w,h){
  gl.activeTexture(gl.TEXTURE3);
  update(w,h,Date.now());
  gl.bindTexture(gl.TEXTURE_2D,texture);
  gl.uniform1i(u.starChart,3);gl.uniform2f(u.chartSize,cols*slots,rows);
  gl.uniform4fv(u.featuredStars,bright);
 };
}
