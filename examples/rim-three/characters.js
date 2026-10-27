import * as THREE from 'three';
import {GLTFLoader} from './vendor/GLTFLoader.js';

const cast = [
  {slug:'blade-wraith',name:'Blade Wraith',x:-7,z:-12,height:2.8,color:'#e868b2'},
  {slug:'core-tank',name:'Core Tank',x:7,z:-12,height:1.05,color:'#efb34b'},
  {slug:'drone-orbiter',name:'Drone Orbiter',x:13,z:-18,height:.82,color:'#e868b2'},
  {slug:'madmechanic',name:'Madmechanic',x:-15,z:-12,height:2.2,color:'#62cfbc'}
];

function plaque(name,color){
  const canvas=document.createElement('canvas');canvas.width=512;canvas.height=112;
  const ctx=canvas.getContext('2d');ctx.fillStyle='#17392f';ctx.fillRect(0,0,512,112);
  ctx.fillStyle=color;ctx.fillRect(0,0,7,112);ctx.font='bold 37px Arial';ctx.fillStyle='#fff1da';ctx.textAlign='center';ctx.textBaseline='middle';ctx.fillText(name,256,56);
  const texture=new THREE.CanvasTexture(canvas);texture.colorSpace=THREE.SRGBColorSpace;
  const mesh=new THREE.Mesh(new THREE.PlaneGeometry(1.7,.37),new THREE.MeshBasicMaterial({map:texture,side:THREE.DoubleSide}));
  mesh.position.set(0,.44,1.2);mesh.name=`Jmenovka ${name}`;return mesh;
}

export function createCharacters(city,renderer){
  const group=new THREE.Group();group.name='Naše postavy na fóru';city.group.add(group);
  const loader=new GLTFLoader(),mixers=[],entries=[],instances=[];
  const state={loading:true,loaded:[],failed:[],entries};
  const ready=Promise.all(cast.map(async item=>{
    const entry={name:item.name,slug:item.slug,x:item.x,z:item.z,loaded:false,clips:[]};entries.push(entry);
    try{
      const gltf=await loader.loadAsync(new URL(`./models/${item.slug}.glb`,import.meta.url).href);
      const root=gltf.scene;root.updateMatrixWorld(true);const bounds=new THREE.Box3().setFromObject(root),size=bounds.getSize(new THREE.Vector3());
      if(!Number.isFinite(size.y)||size.y<=0)throw new Error('Model nemá platné rozměry');
      const scale=item.height/size.y;const holder=new THREE.Group();holder.name=item.name;
      holder.position.set(item.x,.13,item.z);root.scale.multiplyScalar(scale);
      root.position.y-=bounds.min.y*scale;root.position.x-=(bounds.min.x+bounds.max.x)*.5*scale;root.position.z-=(bounds.min.z+bounds.max.z)*.5*scale;
      root.traverse(o=>{if(o.isMesh){o.castShadow=false;o.receiveShadow=true;}});
      holder.add(root);holder.add(plaque(item.name,item.color));group.add(holder);
      const shadow=new THREE.Mesh(new THREE.CircleGeometry(.9,32),new THREE.MeshBasicMaterial({color:'#183126',transparent:true,opacity:.16,depthWrite:false}));shadow.rotation.x=-Math.PI/2;shadow.position.y=.006;holder.add(shadow);
      city.colliders.push({minX:item.x-.8,maxX:item.x+.8,minZ:item.z-.8,maxZ:item.z+.8,actor:item.slug});
      const mixer=new THREE.AnimationMixer(root);const idle=gltf.animations.find(c=>c.name==='Idle')||gltf.animations[0];if(idle)mixer.clipAction(idle).play();mixers.push(mixer);
      Object.assign(entry,{loaded:true,root,holder,mixer,clips:gltf.animations.map(c=>c.name),height:item.height,animations:gltf.animations});state.loaded.push(item.name);renderer.shadowMap.needsUpdate=true;
    }catch(error){entry.error=error.message;state.failed.push({name:item.name,error:error.message});console.error(`Nelze načíst ${item.name}: ${error.message}`)}
  })).then(()=>{state.loading=false;return state});
  // These GLBs have rigid animated parts (no skins). Clones share GPU geometry
  // and materials; every instance gets its own node tree and animation mixer.
  function createInstance(slug){
    const original=entries.find(e=>e.slug===slug&&e.loaded);
    if(!original)throw new Error(`Postava není načtená: ${slug}`);
    const holder=original.holder.clone(true),root=holder.children[0];
    const instance={...original,holder,root,mixer:new THREE.AnimationMixer(root)};
    holder.visible=false;group.add(holder);instances.push(instance);return instance;
  }
  return {state,ready,createInstance,update:dt=>[...entries,...instances].forEach(e=>{if(e.loaded&&e.holder.visible)e.mixer.update(dt)}),describe:()=>({loading:state.loading,loaded:state.loaded,failed:state.failed,instances:instances.length,entries:entries.map(({name,slug,x,z,loaded,clips,height})=>({name,slug,x,z,loaded,clips,height}))})};
}
