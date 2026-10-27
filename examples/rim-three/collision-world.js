// Static convex collision proxies. All plane normals face outwards.
const EPS = 1e-8;
const SUPPORT = 1e-5;
const dot = (a, b) => a.x*b.x + a.y*b.y + a.z*b.z;
const add = (a,b,t=1) => ({x:a.x+b.x*t,y:a.y+b.y*t,z:a.z+b.z*t});
const plane = (x,y,z,d) => { const l=Math.hypot(x,y,z); return {x:x/l,y:y/l,z:z/l,d:d/l}; };
function boundsOf(points) {
  return Object.fromEntries(['X','Y','Z'].flatMap(k => {
    const values=points.map(p=>p[k.toLowerCase()]);
    return [['min'+k,Math.min(...values)],['max'+k,Math.max(...values)]];
  }));
}
export function boxShape(x,y,z,w,h,d,ry=0) {
  const c=Math.cos(ry),s=Math.sin(ry), center={x,y,z};
  const axes=[{x:c,y:0,z:-s},{x:0,y:1,z:0},{x:s,y:0,z:c}];
  const sizes=[w/2,h/2,d/2];
  const planes=axes.flatMap((n,i)=>[1,-1].map(sign=>({x:n.x*sign,y:n.y*sign,z:n.z*sign,d:dot(n,center)*sign+sizes[i]})));
  const points=[];
  for(const a of [-1,1]) for(const b of [-1,1]) for(const e of [-1,1])
    points.push({x:x+a*w/2*c+e*d/2*s,y:y+b*h/2,z:z-a*w/2*s+e*d/2*c});
  return {bounds:boundsOf(points),planes};
}
export function roofShape(x,y,z,w,d,h) {
  return {bounds:{minX:x-w/2,maxX:x+w/2,minY:y,maxY:y+h,minZ:z-d/2,maxZ:z+d/2},planes:[
    plane(h,w/2,0,h*x+w/2*(y+h)),plane(-h,w/2,0,-h*x+w/2*(y+h)),
    plane(0,-1,0,-y),plane(0,0,1,z+d/2),plane(0,0,-1,-z+d/2)
  ]};
}
// An upper semicircle, approximated by small overlapping tangent boxes.
// Unlike a single arch AABB, these leave the passage beneath the stones open.
export function archShapes(x,y,z,radius,tube,segments=16,ry=0) {
  if(!Number.isInteger(segments) || segments<2 || radius<=0 || tube<=0) throw new RangeError('Invalid arch dimensions');
  const c=Math.cos(ry),s=Math.sin(ry),half=Math.PI/(segments*2);
  const radialHalf=tube+radius*(1-Math.cos(half));
  // Overlap through the outer corners too, so a crown seam exposes the
  // upward radial face rather than a nearly vertical segment end cap.
  const tangentHalf=Math.max((radius+radialHalf)*Math.tan(half),radius*Math.sin(half)+tube)+1e-5;
  const shapes=[];
  for(let i=0;i<segments;i++) {
    const a=(i+.5)*Math.PI/segments,ca=Math.cos(a),sa=Math.sin(a);
    const center={x:x+radius*ca*c,y:y+radius*sa,z:z-radius*ca*s};
    const axes=[{x:ca*c,y:sa,z:-ca*s},{x:-sa*c,y:ca,z:sa*s},{x:s,y:0,z:c}];
    const sizes=[radialHalf,tangentHalf,tube];
    const planes=axes.flatMap((n,j)=>[1,-1].map(sign=>({x:n.x*sign,y:n.y*sign,z:n.z*sign,d:dot(n,center)*sign+sizes[j]})));
    const points=[];
    for(const u of [-1,1]) for(const v of [-1,1]) for(const w of [-1,1]) {
      let p={...center};
      for(const [j,sign] of [u,v,w].entries()) p=add(p,axes[j],sign*sizes[j]);
      points.push(p);
    }
    shapes.push({bounds:boundsOf(points),planes});
  }
  return shapes;
}
function intersects(a,b) {
  return a.minX<=b.maxX && a.maxX>=b.minX && a.minY<=b.maxY && a.maxY>=b.minY && a.minZ<=b.maxZ && a.maxZ>=b.minZ;
}
function union(shapes) {
  const b={};
  for(const k of ['X','Y','Z']) { b['min'+k]=Math.min(...shapes.map(s=>s.bounds['min'+k])); b['max'+k]=Math.max(...shapes.map(s=>s.bounds['max'+k])); }
  return b;
}
function clip(start,delta,shape,height=0,radius=0,body=false) {
  let enter=0,exit=1,normal=null,inside=true,closest=-Infinity,closestNormal=null;
  for(const p of shape.planes) {
    const expansion=radius*(Math.abs(p.x)+Math.abs(p.z))+height/2*Math.abs(p.y);
    const distance=dot(p,start)-p.d-expansion, speed=dot(p,delta);
    if(distance>=-EPS) inside=false;
    if(distance>closest) { closest=distance; closestNormal=p; }
    if(Math.abs(speed)<EPS) {
      if(distance>EPS || (body && distance>=-EPS)) return null;
      continue;
    }
    const t=-distance/speed;
    if(speed<0) { if(t>=enter-EPS) { if(t>enter) enter=t; normal=p; } }
    else exit=Math.min(exit,t);
    if(enter>exit+EPS) return null;
  }
  if(inside) return {fraction:0,normal:closestNormal,shape};
  if(!normal || enter< -EPS || enter>1+EPS || exit<0 || (body && exit<=Math.max(0,enter)+EPS)) return null;
  return {fraction:Math.max(0,Math.min(1,enter)),normal,shape};
}
export class CollisionWorld {
  constructor(shapes=[]) {
    this.shapes=shapes.slice();
    this.stats={shapes:shapes.length,nodes:0,lastCandidates:0};
    const build=items=>{
      if(!items.length) return null;
      this.stats.nodes++;
      const bounds=union(items);
      if(items.length<=4) return {bounds,items};
      const axis=['X','Y','Z'].sort((a,b)=>(bounds['max'+b]-bounds['min'+b])-(bounds['max'+a]-bounds['min'+a]))[0];
      items.sort((a,b)=>(a.bounds['min'+axis]+a.bounds['max'+axis])-(b.bounds['min'+axis]+b.bounds['max'+axis]));
      const mid=items.length>>1;
      return {bounds,left:build(items.slice(0,mid)),right:build(items.slice(mid))};
    };
    this.root=build(this.shapes.slice());
  }
  query(bounds) {
    const found=[];
    const visit=node=>{
      if(!node || !intersects(node.bounds,bounds)) return;
      if(node.items) { for(const shape of node.items) if(intersects(shape.bounds,bounds)) found.push(shape); }
      else { visit(node.left); visit(node.right); }
    };
    visit(this.root); this.stats.lastCandidates=found.length; return found;
  }
  sweep(center,delta,height=0,radius=0,body=false) {
    const end=add(center,delta), bounds=boundsOf([center,end]);
    for(const k of ['X','Y','Z']) { const size=k==='Y'?height/2:radius; bounds['min'+k]-=size+EPS; bounds['max'+k]+=size+EPS; }
    let nearest=null;
    for(const shape of this.query(bounds)) {
      const hit=clip(center,delta,shape,height,radius,body);
      if(hit && (!nearest || hit.fraction<nearest.fraction)) nearest=hit;
    }
    return nearest;
  }
  castSegment(start,end) {
    const delta={x:end.x-start.x,y:end.y-start.y,z:end.z-start.z};
    const hit=this.sweep(start,delta);
    return hit ? {...hit,normal:{x:hit.normal.x,y:hit.normal.y,z:hit.normal.z},point:add(start,delta,hit.fraction)} : null;
  }
  clear(feet,height=1.7,radius=.32) {
    const center={...feet,y:feet.y+height/2};
    const bounds={minX:feet.x-radius,maxX:feet.x+radius,minY:feet.y,maxY:feet.y+height,minZ:feet.z-radius,maxZ:feet.z+radius};
    return !this.query(bounds).some(shape=>shape.planes.every(p=>dot(p,center)<p.d+radius*(Math.abs(p.x)+Math.abs(p.z))+height/2*Math.abs(p.y)-EPS));
  }
  move(feet,delta,height=1.7,radius=.32) {
    let center={...feet,y:feet.y+height/2}, remaining={...delta},grounded=false,hitCeiling=false;
    const collisions=[];
    for(let i=0;i<4;i++) {
      if(Math.hypot(remaining.x,remaining.y,remaining.z)<EPS) break;
      const hit=this.sweep(center,remaining,height,radius,true);
      if(!hit) { center=add(center,remaining); break; }
      center=add(center,remaining,hit.fraction);
      const n={x:hit.normal.x,y:hit.normal.y,z:hit.normal.z};
      collisions.push({...hit,normal:n,point:{...center,y:center.y-height/2}});
      if(n.y>.5) grounded=true;
      if(n.y<-.5) hitCeiling=true;
      remaining={x:remaining.x*(1-hit.fraction),y:remaining.y*(1-hit.fraction),z:remaining.z*(1-hit.fraction)};
      // Gravity alone must not turn into downhill motion on a resting roof.
      if(n.y>.5 && remaining.y<=0 && Math.abs(remaining.x)+Math.abs(remaining.z)<EPS) break;
      const inward=dot(remaining,n);
      if(inward<0) remaining=add(remaining,n,-inward);
      else break;
    }
    // A tiny support probe makes zero-delta/resting calls stable, including roofs.
    const support=this.sweep(center,{x:0,y:-SUPPORT,z:0},height,radius,true);
    if(support && support.normal.y>.5) {
      grounded=true;
      if(delta.y<=0) center.y-=SUPPORT*support.fraction;
    } else grounded=false;
    return {position:{x:center.x,y:center.y-height/2,z:center.z},grounded,hitCeiling,collisions};
  }
}
