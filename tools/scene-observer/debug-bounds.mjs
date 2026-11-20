// Optional DOM presentation only; all bounds, clipping and projection come from Rust.
export function installDebugBounds(engine){
 if(typeof engine.debugSceneBounds!=='function')return null;
 const ns='http://www.w3.org/2000/svg';
 const svg=document.createElementNS(ns,'svg');svg.id='debug-bounds';
 svg.setAttribute('viewBox','0 0 1000 1000');svg.setAttribute('preserveAspectRatio','none');
 svg.setAttribute('aria-label','Debug hranice objektů a vykreslovacích vrstev');
 Object.assign(svg.style,{position:'fixed',inset:'0',width:'100%',height:'100%',pointerEvents:'none',zIndex:'4',display:'none'});
 const legend=document.createElement('div');legend.id='debug-bounds-legend';
 Object.assign(legend.style,{position:'fixed',top:'8px',left:'8px',right:'48px',font:'11px system-ui',color:'#fff',background:'#070b12df',padding:'7px 9px',borderRadius:'5px',pointerEvents:'none',zIndex:'5',display:'none'});
 document.body.append(svg,legend);
 let enabled=false,last=-Infinity;
 function setEnabled(value){enabled=!!value;svg.style.display=legend.style.display=enabled?'block':'none';if(!enabled)svg.replaceChildren();last=-Infinity;}
 function update(now){
  if(!enabled)return;last=now;
  const data=engine.debugSceneBounds(),fragment=document.createDocumentFragment();
  for(const entry of data.entries){
   const layer=String(entry.layer),color=/SSR/i.test(layer)?'#ff7070':/glow/i.test(layer)?'#ff78ee':/primitive|part/i.test(entry.kind+' '+layer)?'#ffcf62':'#53e5ff';
   const group=document.createElementNS(ns,'g');group.dataset.objectId=entry.id;group.dataset.layer=layer;
   const title=document.createElementNS(ns,'title');title.textContent=`${entry.id} · ${entry.kind} · ${layer} · částí: ${entry.parts}`;group.append(title);
   const path=document.createElementNS(ns,'path');
   path.setAttribute('d',entry.edges.map(([x,y,u,v])=>`M${x*1000},${y*1000}L${u*1000},${v*1000}`).join(''));
   path.setAttribute('fill','none');path.setAttribute('stroke',color);path.setAttribute('stroke-width','1.2');path.setAttribute('vector-effect','non-scaling-stroke');group.append(path);
   if(entry.label&&entry.parts>1){const label=document.createElementNS(ns,'text');label.setAttribute('x',String(entry.label[0]*1000));label.setAttribute('y',String(entry.label[1]*1000));label.setAttribute('fill',color);label.setAttribute('font-size','13');label.setAttribute('stroke','#000');label.setAttribute('stroke-width','.7');label.setAttribute('paint-order','stroke');label.textContent=`#${entry.id} ×${entry.parts}`;group.append(label);}
   fragment.append(group);
  }
  svg.replaceChildren(fragment);svg.dataset.entries=String(data.entries.length);
  legend.textContent=`DEBUG · azurová: objekt · žlutá: části · růžová: možný glow · červená: SSR obdélník · ${data.entries.length} obálek${data.truncated?' (omezeno)':''} · Počet vrstev není měření GPU času; debug zvyšuje režii.`;
 }
 return {setEnabled,update,dispose(){svg.remove();legend.remove();}};
}
