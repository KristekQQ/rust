// Presentation timing only. Never predicts simulation or generates source poses.
export class InterpolationCadence {
 constructor(){this.last=null;this.intervals=[];this.duration=60;}
 sample(sampledAt){
  if(!Number.isFinite(sampledAt))return null;
  const previous=this.last;
  if(previous!==null&&sampledAt<=previous)return null;
  this.last=sampledAt;
  if(previous===null)return null;
  const dt=sampledAt-previous;
  if(dt<8||dt>750)return null; // Ignore duplicates, reordering and source pauses.
  this.intervals.push(dt);if(this.intervals.length>7)this.intervals.shift();
  if(this.intervals.length<3)return null;
  const ordered=[...this.intervals].sort((a,b)=>a-b);
  const wanted=Math.round(Math.min(250,Math.max(30,ordered[Math.floor(ordered.length/2)]*1.1)));
  // Configuration settles active tracks; hysteresis avoids doing that for jitter.
  if(Math.abs(wanted-this.duration)<Math.max(15,this.duration*.25))return null;
  this.duration=wanted;return wanted;
 }
}
