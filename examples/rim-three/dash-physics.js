export const DASH_DISTANCE=5;
export const DASH_DURATION=.2;

// Collision handling and vertical gravity stay in the existing CollisionWorld.
export class DashPhysics {
  constructor(){this.count=0;this.reset()}
  reset(){this.active=false;this.remaining=0;this.x=0;this.z=0;this.distance=0}
  start(direction){
    const length=Math.hypot(direction.x,direction.z);
    if(this.active||!Number.isFinite(length)||length<1e-6)return false;
    this.x=direction.x/length;this.z=direction.z/length;this.remaining=DASH_DURATION;this.distance=0;this.active=true;this.count++;return true;
  }
  step(dt){
    if(!this.active||!Number.isFinite(dt)||dt<=0)return {dx:0,dz:0};
    const time=Math.min(dt,this.remaining),distance=time*DASH_DISTANCE/DASH_DURATION;
    this.remaining=Math.max(0,this.remaining-time);this.distance+=distance;
    if(this.remaining<1e-9)this.active=false;
    return {dx:this.x*distance,dz:this.z*distance};
  }
  describe(){return {active:this.active,remaining:this.remaining,distance:this.distance,count:this.count}}
}
