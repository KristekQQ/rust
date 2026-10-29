// Deterministic event fixture; no production scene/update-rate changes.
export function createPanelWorkload(engine,count,operations){
 if(count>64)throw Error('Bitmap event fixture budget: 64 live panels');
 const scene=engine.scene,panels=[],pieces=[];let last=-1,lastSeconds=0;
 const mesh={color:[.9,.15,.08],positions:[-1,-1,-1,1,-1,-1,1,1,-1,-1,1,-1,-1,-1,1,1,-1,1,1,1,1,-1,1,1],indices:[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5]};
 engine.setCameraView([0,1,7],[0,0,0]);engine.setBackgroundColor([.008,.014,.032]);
 engine.postprocessing.enable('fxaa',{strength:1});
 scene.createLight({position:[-2,3,5],color:[10,10,10]});
 const side=Math.ceil(Math.sqrt(count));
 const make=i=>scene.createInfoPanel('debug-text',{position:[(i%side-(side-1)/2)*6/side,(Math.floor(i/side)-(side-1)/2)*3.5/side,0],width:5.7/side,height:3.2/side});
 for(let i=0;i<count;i++)panels.push(make(i));
 const clearPieces=()=>{for(const p of pieces)if(p.exists){p.remove();operations.removes++;}pieces.length=0;};
 const pose=seconds=>{
  lastSeconds=seconds;const step=Math.floor(seconds*4);if(step===last)return;
  // Catch up every scheduled event once, independent of callback cadence.
  for(let n=last+1;n<=step;n++){
   for(let i=0;i<count;i++){
    if(n%16===12){panels[i].remove();operations.removes++;panels[i]=make(i);operations.spawns++;}
    const text=n%16===8?'':n%16===9?'SHORT':('PANEL '+String(i).padStart(2,'0')+' EVENT '+String(n%10)+' ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789');
    engine.diagnosticPanelText(panels[i].id,text);operations.panelTextChanges=(operations.panelTextChanges||0)+1;
   }
   if(n%8===0){
    clearPieces();const target=scene.createMesh(mesh).setPosition([0,0,0]).setScale([.4,.4,.4]).setCuttable(true);operations.spawns++;
    const cutter=scene.createCube([0,1,0]).setScale([.01,.01,.01]).setCutter({base:[0,0,-100],tip:[0,0,100]});
    try{operations.cutAttempts++;const cut=scene.cutWith(cutter,{motion:[0,-2,0],separation:.02});if(cut.removed.length!==1||cut.pieces.length!==2)throw Error('Panel event cut expected 2 pieces: '+JSON.stringify({removed:cut.removed.length,pieces:cut.pieces.length}));pieces.push(...cut.pieces);operations.cuts++;}finally{cutter.remove();operations.removes++;if(target.exists)target.remove();}
   }
  }
  last=step;
 };
 return {pose,reset(){clearPieces();last=-1;},state(){return {kind:'bitmap-panel-event-fixture',step:last,seconds:lastSeconds,livePanels:panels.length,liveCutPieces:pieces.filter(p=>p.exists).length,panels:engine.debugStats().panelMeshes};},quality:{profile:'deterministic bitmap/cut diagnostic fixture; not authored Neon gameplay',cadenceHz:4,textMaxCharacters:64,panels:count,cutEverySeconds:2,panelRecreationEverySeconds:4,fxaa:true,camera:{position:[0,1,7],target:[0,0,0]},clock:'every scheduled 250ms event processed once; geometry upload coalesces only within the same application frame'}};
}
