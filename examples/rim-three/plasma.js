import * as THREE from 'three';

export const PLASMA_CHARGE = .85;
export const PLASMA_DURATION = 2;
const UP = new THREE.Vector3(0,1,0);

// Fixed effects per shooter. Target and origin are locked throughout anticipation/fire.
export function createPlasmaAttack({scene,castWorld,heavy=false}) {
  const color=heavy?0xffac32:0xff399f, radius=heavy?.34:.23;
  const group=new THREE.Group();scene.add(group);group.visible=false;
  const material=(c,opacity)=>new THREE.MeshBasicMaterial({color:c,transparent:true,opacity,depthWrite:false,toneMapped:false,blending:THREE.AdditiveBlending});
  const tube=new THREE.CylinderGeometry(1,1,1,12,1,true);
  const outer=new THREE.Mesh(tube,material(color,.2)),core=new THREE.Mesh(tube,material(0xfff3da,.9));
  const body=new THREE.Mesh(tube,material(color,.7));body.material.blending=THREE.NormalBlending;group.add(outer,body,core);
  const ringGeometry=new THREE.TorusGeometry(1,.035,5,32);
  const rings=Array.from({length:6},()=>{const mesh=new THREE.Mesh(ringGeometry,material(color,.8));group.add(mesh);return mesh;});
  const impact=new THREE.Mesh(new THREE.IcosahedronGeometry(1,1),material(color,.5));group.add(impact);
  const origin=new THREE.Vector3(),target=new THREE.Vector3(),end=new THREE.Vector3(),direction=new THREE.Vector3();
  let phase='idle',elapsed=0,length=0;
  function cancel(){phase='idle';elapsed=0;group.visible=false;}
  function begin(from,to){if(phase!=='idle'||from.distanceToSquared(to)<.001)return false;
    origin.copy(from);target.copy(to);direction.subVectors(target,origin).normalize();elapsed=0;phase='charge';group.visible=true;return true;}
  function layout(){
    const far=origin.clone().addScaledVector(direction,18),hit=castWorld(origin,far);
    length=18*(hit?THREE.MathUtils.clamp(hit.fraction,0,1):1);end.copy(origin).addScaledVector(direction,length);
    const pulse=1+.06*Math.sin(elapsed*24),firing=phase==='fire';
    for(const [mesh,width] of [[outer,firing?radius*1.8: .018],[body,firing?radius:.012],[core,firing?radius*.25:.006]]){
      mesh.position.copy(origin).lerp(end,.5);mesh.quaternion.setFromUnitVectors(UP,direction);mesh.scale.set(width*pulse,length,width*pulse);
    }
    outer.material.opacity=firing?.2:.16;core.material.opacity=firing?.95:.35;
    rings.forEach((ring,i)=>{const travel=firing?((elapsed*9+i*length/6)%Math.max(.01,length)):Math.min(length,origin.distanceTo(target));
      ring.position.copy(origin).addScaledVector(direction,travel);ring.quaternion.setFromUnitVectors(new THREE.Vector3(0,0,1),direction);
      ring.scale.setScalar(firing?radius*(1.1+.15*Math.sin(elapsed*18+i)):.25+elapsed/PLASMA_CHARGE*.55+i*.045);
      ring.material.opacity=firing?.75:(i===0?.9:.18);
    });
    impact.visible=firing;impact.position.copy(end);impact.scale.setScalar(radius*2*pulse);impact.rotation.y=elapsed*3;
  }
  function update(dt,head){
    if(!Number.isFinite(dt)||dt<=0||phase==='idle')return 0;
    // Integrate exact overlap with the damaging two-second interval, even on large steps.
    const before=elapsed;elapsed+=dt;
    const active=Math.max(0,Math.min(elapsed,PLASMA_CHARGE+PLASMA_DURATION)-Math.max(before,PLASMA_CHARGE));
    phase=elapsed<PLASMA_CHARGE?'charge':'fire';layout();
    // Capsule from chest to head; walls clip the damaging segment as well as its mesh.
    let hit=false;
    for(const y of [0,-.3,-.6,-.9]){const p=head.clone();p.y+=y;const t=p.clone().sub(origin).dot(direction);
      if(t>=0&&t<=length&&p.distanceToSquared(origin.clone().addScaledVector(direction,t))<(radius+.23)**2&&!castWorld(origin,p))hit=true;}
    const damage=hit?active*(heavy?170:125):0;
    if(elapsed>=PLASMA_CHARGE+PLASMA_DURATION)cancel();
    return damage;
  }
  function describe(){return {phase,elapsed,target:target.toArray(),origin:origin.toArray(),end:end.toArray(),radius,duration:PLASMA_DURATION};}
  function dispose(){cancel();scene.remove(group);tube.dispose();ringGeometry.dispose();impact.geometry.dispose();for(const mesh of [outer,body,core,...rings,impact])mesh.material.dispose();}
  return {begin,update,cancel,describe,dispose,get active(){return phase!=='idle'}};
}
