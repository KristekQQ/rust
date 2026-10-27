// Metres and seconds. The caller scales simulation time; tracking stays realtime.
export const GRAVITY = 9.8;
export const SLOW_TIME = .12;

export function planJump(from, target) {
  const distance = Math.hypot(target.x - from.x, target.z - from.z);
  const height = Math.max(0, from.height), targetHeight = Math.max(0,target.height ?? target.y ?? 0);
  const rise = Math.min(2.2, Math.max(.8, distance * .16)) + Math.max(0,targetHeight-height)*.8;
  const apex = Math.max(height,targetHeight)+rise;
  const vy = Math.sqrt(2 * GRAVITY * (apex-height));
  const duration = vy / GRAVITY + Math.sqrt(2 * (apex-targetHeight) / GRAVITY);
  return {duration, vx:(target.x-from.x)/duration, vy, vz:(target.z-from.z)/duration,
    from:{x:from.x,z:from.z,height}, target:{x:target.x,z:target.z,height:targetHeight}};
}

export class JumpPhysics {
  constructor() { this.count=0; this.reset(); }
  reset() { this.active=false; this.height=0; this.vx=0; this.vy=0; this.vz=0; this.elapsed=0; this.duration=0; this.target=null; this.collisions=0; }
  launch(from,target) {
    const p=planJump(from,target);
    Object.assign(this,{active:true,height:p.from.height,vx:p.vx,vy:p.vy,vz:p.vz,elapsed:0,duration:p.duration,target:p.target,collisions:0});
    this.count++; return p;
  }
  step(dt,position,canStand) {
    let x=position.x,z=position.z,remaining=dt;
    while(this.active && remaining>1e-9) {
      const impact=(this.vy+Math.sqrt(this.vy*this.vy+2*GRAVITY*this.height))/GRAVITY;
      const h=Math.min(remaining,1/120,.1/Math.max(1,Math.abs(this.vx),Math.abs(this.vz)),Math.max(0,impact));
      if(h<1e-9) { this.active=false; this.height=0; this.vx=this.vy=this.vz=0; break; }
      const dx=this.vx*h,dz=this.vz*h;
      if(canStand(x+dx,z))x+=dx;else {this.vx=0;this.collisions++;}
      if(canStand(x,z+dz))z+=dz;else {this.vz=0;this.collisions++;}
      this.height=Math.max(0,this.height+this.vy*h-.5*GRAVITY*h*h);
      this.vy-=GRAVITY*h;this.elapsed+=h;remaining-=h;
      if(h>=impact-1e-9){this.active=false;this.height=0;this.vx=this.vy=this.vz=0;}
    }
    return {dx:x-position.x,dz:z-position.z,height:this.height};
  }
  // Shared convex-world path also runs while idle, so stepping off a roof starts a fall.
  stepWorld(dt,position,world,bodyHeight=1.7) {
    let p={x:position.x,y:this.height,z:position.z},remaining=dt;
    while(remaining>1e-9){
      const h=Math.min(remaining,1/120);
      const delta={x:this.active?this.vx*h:0,y:this.vy*h-.5*GRAVITY*h*h,z:this.active?this.vz*h:0};
      const hit=world.move(p,delta,bodyHeight);
      p=hit.position;this.vy-=GRAVITY*h;this.elapsed+=this.active?h:0;remaining-=h;
      this.collisions+=hit.collisions.length;
      if(hit.hitCeiling&&this.vy>0)this.vy=0;
      if(hit.grounded&&this.vy<=0){this.active=false;this.vx=this.vy=this.vz=0;}
      else this.active=true;
    }
    this.height=p.y;
    return {dx:p.x-position.x,dz:p.z-position.z,height:p.y};
  }
  describe() { return {active:this.active,height:this.height,velocity:{x:this.vx,y:this.vy,z:this.vz},elapsed:this.elapsed,duration:this.duration,target:this.target,count:this.count,collisions:this.collisions}; }
}
