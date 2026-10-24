import {capturePixels,assertForeground,assertChanged} from '../../tests/pixels.js';
import {engine,content,game,view,play} from './main.js';
export async function verify(){
 const result=document.getElementById('result');if(!result)return;
 const assert=(/** @type {unknown} */v,/** @type {string} */m)=>{if(!v)throw Error(m);};
 const frames=async(n=6)=>{for(let i=0;i<n;i++)await new Promise(requestAnimationFrame);};
 const state=()=>{const s=engine.scene.rimState;if(!s)throw Error("Native game unavailable");return s;};
 const canvas=/** @type {HTMLCanvasElement} */(document.getElementById('gpu-canvas'));
 try{
  assert(['BrowserWebGpu','Gl'].includes(engine.renderer_backend()),'Actual native renderer');
  assert(content.environment.info.primitives===19,'Original city geometry');assert(game.collision.shapes.length===1065,'Original convex roofs/arches/columns');
  view('gate');const city=await capturePixels(canvas);assertForeground(city,'Actual Roman city',40000);
  view('forum');assertChanged(city,await capturePixels(canvas),'Original camera landmarks',20000);
  view('gate');play();await frames();assert(state().enemies.filter(e=>e.phase!==3).length===6,'First wave: six small orbiters');
  const start=state().player.position;window.dispatchEvent(new KeyboardEvent('keydown',{code:'KeyW'}));await frames(30);window.dispatchEvent(new KeyboardEvent('keyup',{code:'KeyW'}));
  assert(state().player.position.z<start.z-.5,'Real keyboard drives collision-aware native player '+JSON.stringify({start,current:state().player}));
  const beforeDash=state().player.position.z;engine.scene.rimInput({dash:true});await frames(18);assert(state().player.dash.count===1&&state().player.position.z<beforeDash-4.8,'5m native dash');
  assert(state().player.stamina.value<3.1,'Dash spends one stamina segment');
  engine.scene.rimInput({aim:true});await frames(8);assert(state().player.aiming&&state().player.target,'Native slow-time arc finds convex-world surface');engine.scene.rimInput({aim:false});await frames(5);assert(state().player.jump.count===1,'Aim release launches native ballistic jump');
  // Reposition via the same validated teleport command, then strike using actual desktop equipment.
  engine.scene.rimInput({reset:true});const enemy=state().enemies.find(e=>e.phase===1||e.phase===2);if(!enemy)throw Error('Live opponent');
  engine.scene.rimInput({teleport:[enemy.position[0],0,enemy.position[2]+1.1]});await frames(2);
  const beforeHit=await capturePixels(canvas);engine.scene.rimInput({slash:1});await frames(14);
  const wounded=state().enemies.find(e=>e.id===enemy.id);
  assert(wounded?.hp===50&&wounded.phase!==3&&state().kills===0,'First gladius swing removes 50 HP without death or a cut');
  assert(state().cuts.queued===0&&state().cuts.pending===0&&state().cuts.pieces===0,'Nonfatal strike never queues worker geometry');
  await new Promise(r=>setTimeout(r,450));
  const target=state().enemies.find(e=>e.id===enemy.id);if(!target)throw Error('Wounded opponent disappeared');
  engine.scene.rimInput({teleport:[target.position[0],0,target.position[2]+1.1]});await frames(2);
  engine.scene.rimInput({slash:1});await frames(14);
  assert(state().kills>=1&&state().enemies.find(e=>e.id===enemy.id)?.hp===0,'Second separate gladius swing defeats original GLB opponent');
  for(let i=0;i<180&&state().cuts.pieces===0;i++)await frames(1);
  assert(state().cuts.pieces>0&&state().cuts.errors===0,'Rust/WASM worker returns real split GLB geometry');
  assert(state().frame.simulation===state().frame.visual,'Simulation and visuals share the presented frame');
  assertChanged(beforeHit,await capturePixels(canvas),'Actual cut/death geometry',40);
  assert(!performance.getEntriesByType('resource').some(r=>r.name.includes('three.module')),'No Three runtime');
  assert(content.characters.every(({model})=>!model.info.lastError),'No model animation failures');
  engine.scene.rimInput({reset:true,pause:true});view('gate');
  result.textContent=`PASS: ${engine.renderer_backend()} · original city + 1065 convex shapes, native movement/dash/stamina/jump, six orbiters, two gladius swings, actual Rust worker split geometry, synchronized frame pixels`;
 }catch(error){engine.scene.rimInput({reset:true,pause:true});result.textContent=`FAIL: ${error instanceof Error?error.message:String(error)}`;console.error(error);}
}
