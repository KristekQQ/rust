// Presentation timing only. Never predicts simulation or generates source poses.
export class InterpolationCadence {
 constructor(){this.last=null;this.intervals=[];this.duration=60;this.lastArrival=null;this.arrivals=[];}
 sample(sampledAt,arrivedAt){
  if(!Number.isFinite(sampledAt))return null;
  const previous=this.last;
  if(previous!==null&&sampledAt<=previous)return null;
  this.last=sampledAt;
  if(Number.isFinite(arrivedAt)){
   const gap=this.lastArrival===null?null:arrivedAt-this.lastArrival;
   this.lastArrival=arrivedAt;
   // A hidden tab/source pause is not normal jitter and must not inflate smoothing.
   if(gap>=8&&gap<=750){this.arrivals.push(gap);if(this.arrivals.length>21)this.arrivals.shift();}
   else if(gap>750)this.arrivals=[];
  }
  if(previous===null)return null;
  const dt=sampledAt-previous;
  if(dt<8||dt>750)return null; // Ignore duplicates, reordering and source pauses.
  this.intervals.push(dt);if(this.intervals.length>7)this.intervals.shift();
  if(this.intervals.length<3)return null;
  const ordered=[...this.intervals].sort((a,b)=>a-b);
  const delivery=[...this.arrivals].sort((a,b)=>a-b);
  const jitterWindow=delivery.length>=7?Math.min(160,delivery[Math.floor((delivery.length-1)*.95)]):0;
  const wanted=Math.round(Math.min(250,Math.max(30,ordered[Math.floor(ordered.length/2)]*1.1,jitterWindow)));

  // Hysteresis prevents rapid changes of presentation lag; active tracks keep their poses.
  if(Math.abs(wanted-this.duration)<Math.max(15,this.duration*.25))return null;
  this.duration=wanted;return wanted;
 }
}
