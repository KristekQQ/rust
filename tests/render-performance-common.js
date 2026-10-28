// Limited shared-GLB renderer control. Production appearance is measured separately.
import {stressPosition} from '/examples/rim/stress.js';
export async function createNativeCommon(engine,count){
 const base='/assets/models/rim/',layout=await(await fetch(base+'layout.json')).json(),spec=layout.models.find(s=>s.slug==='drone-orbiter');
 engine.scene.clear();engine.set_render_effects(false,false);engine.postprocessing.disable('fxaa');engine.setSceneAppearance(null);engine.setBackgroundColor([0,0,0]);
 if(!engine.setRendererExperiment(1))throw Error('Common base-color shader unavailable');
 const city=await engine.scene.loadModel(base+layout.cityFile);city.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});city.useCamera(layout.cameras[0].nodeIndex);
 const source=await engine.scene.loadModel(base+spec.file);
 for(let i=0;i<count;i++){const m=i===0?source:source.createInstance();m.setPosition(stressPosition(i)).setScale(spec.root.scale).playAnimation('Idle',{loop:true}).pauseAnimation().seekAnimation(0).setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,opaqueReorderable:true});}
 const metadata={assets:[base+layout.cityFile,base+spec.file],droneCount:count,pose:'Idle@0',camera:layout.cameras[0],scale:spec.root.scale,representation:'native shared glTF batching',profile:'limited base-color only; not production PBR',AA:'none',lights:0,shadows:false,toneMapping:'none',fog:false,postprocessing:false,output:'sRGB',gameplay:false};
 return {quality:metadata,metadata,phaseState:()=>({kind:'common-shared-glb',count,poseSeconds:0})};
}
