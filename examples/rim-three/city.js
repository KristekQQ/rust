import {boxShape,roofShape,archShapes} from './collision-world.js';
import { mergeGeometries } from './vendor/BufferGeometryUtils.js';

// A self-contained, deterministic city. All architectural parts are real meshes;
// static meshes are batched by material to keep the scene inexpensive in VR.
export function createCity(THREE) {
  const group = new THREE.Group();
  group.name = 'Antické středomořské město';
  const colliders = [], collisionShapes = [], bins = new Map();
  let seed = 41831;
  const random = () => ((seed = (1664525 * seed + 1013904223) >>> 0) / 4294967296);
  const mat = (color, roughness = 0.9, metalness = 0) => new THREE.MeshStandardMaterial({ color, roughness, metalness });
  const materials = {
    earth: mat('#c3ad80'), stone: mat('#d7c5a1'), light: mat('#eee0be'), shadow: mat('#ac9876'),
    plaster: mat('#e1cda3'), peach: mat('#d9ae89'), white: mat('#efe4c9'), roof: mat('#a65036'), roofLight: mat('#c77349'),
    wood: mat('#604731'), dark: mat('#34302a'), bronze: mat('#877545', .6, .45),
    leaf: mat('#627149'), leafLight: mat('#7c8250'), cypress: mat('#394d35'), red: mat('#a64132'), blue: mat('#476979'),
    water: new THREE.MeshStandardMaterial({ color: '#528c91', roughness: .24, metalness: .25 }),
    sand: mat('#d6bf8b'), flower: mat('#bd637a')
  };
  const add = (g, key, x=0,y=0,z=0, rx=0,ry=0,rz=0) => {
    g.rotateX(rx); g.rotateY(ry); g.rotateZ(rz); g.translate(x,y,z);
    // Use a single attribute layout for every primitive before merging.
    g.deleteAttribute('uv');
    if (g.index) { const old = g; g = g.toNonIndexed(); old.dispose(); }
    if (!bins.has(key)) bins.set(key, []);
    bins.get(key).push(g);
  };
  const box = (key,x,y,z,w,h,d,ry=0) => {
    add(new THREE.BoxGeometry(w,h,d),key,x,y,z,0,ry);
    // Structural solids only; tiny trim/paving does not need collision geometry.
    if(h>=.18 && w>=.15 && d>=.15) collisionShapes.push(boxShape(x,y,z,w,h,d,ry));
  };
  const cyl = (key,x,y,z,rt,rb,h,n=10) => {
    add(new THREE.CylinderGeometry(rt,rb,h,n),key,x,y,z);
    if(Math.max(rt,rb)>=.15 && h>=.18 && key!=='water')
      collisionShapes.push(boxShape(x,y,z,Math.max(rt,rb)*2,h,Math.max(rt,rb)*2));
  };
  const sphere = (key,x,y,z,r,sx=1,sy=1,sz=1) => { const g=new THREE.IcosahedronGeometry(r,1); g.scale(sx,sy,sz); add(g,key,x,y,z); };
  const blocked = (x,z,w,d) => colliders.push({minX:x-w/2,maxX:x+w/2,minZ:z-d/2,maxZ:z+d/2});
  const roof = (x,y,z,w,d,h,key='roof') => {
    collisionShapes.push(roofShape(x,y,z,w,d,h));
    const v=[-w/2,0,-d/2,w/2,0,-d/2,0,h,-d/2,-w/2,0,d/2,w/2,0,d/2,0,h,d/2];
    const indices=[0,2,1,3,4,5,0,3,5,0,5,2,1,2,5,1,5,4,0,1,4,0,4,3];
    const g=new THREE.BufferGeometry();g.setAttribute('position',new THREE.Float32BufferAttribute(v,3));g.setIndex(indices);g.computeVertexNormals(); add(g,key,x,y,z);
    box('roofLight',x,y+h+.045,z,.19,.17,d+.25);
    for(let zz=z-d/2;zz<z+d/2;zz+=.72) {
      const ridge=new THREE.CylinderGeometry(.115,.115,d>0?.62:.62,6);ridge.rotateX(Math.PI/2); add(ridge,'roofLight',x,y+h+.09,zz+.3);
    }
  };
  function column(x,z,h=6,r=.46,y=0) {
    box('light',x,y+.16,z,r*2.9,.32,r*2.9);
    cyl('shadow',x,y+.42,z,r*1.2,r*1.35,.2,12);
    cyl('light',x,y+h/2+.5,z,r*.83,r,h,12);
    cyl('shadow',x,y+h+.56,z,r*1.18,r*.9,.2,12);
    box('light',x,y+h+.82,z,r*2.8,.32,r*2.8);
  }
  function amphora(x,z,s=1) {
    cyl('roof',x,.18*s,z,.34*s,.25*s,.35*s);
    sphere('roofLight',x,.66*s,z,.49*s,.82,1.2,.82);
    cyl('roof',x,1.21*s,z,.18*s,.23*s,.45*s);
    cyl('roofLight',x,1.45*s,z,.24*s,.24*s,.10*s);
    for(const side of [-1,1]) add(new THREE.TorusGeometry(.22*s,.055*s,5,9),'roof',x+side*.31*s,1.12*s,z,Math.PI/2);
  }
  function cypress(x,z,s=1) {
    cyl('wood',x,1.4*s,z,.17*s,.24*s,2.8*s,7);
    for(let j=0;j<3;j++) cyl('cypress',x,(3.4+j*1.3)*s,z,.07*s,(1.14-j*.19)*s,4*s,9);
  }
  function olive(x,z,s=1) {
    cyl('wood',x,1.4*s,z,.20*s,.34*s,2.8*s,7);
    for(let j=0;j<5;j++) {
      const a=j*2.4; sphere(j%2?'leaf':'leafLight',x+Math.cos(a)*1.25*s,(3.3+(j%2)*.6)*s,z+Math.sin(a)*1.25*s,1.6*s,1.15,.72,1.1);
    }
  }
  // Ground, stone avenue and the forum's concentric paving.
  box('earth',0,-.3,-12,176,.6,162);
  box('stone',0,.035,5,15,.07,116);
  box('stone',0,.035,-9,57,.07,37);
  for(let z=-49;z<63;z+=2.6) {
    for(let x=-6;x<=6;x+=2.4) box((Math.round(z*3+x)%4===0)?'light':'shadow',x,.083,z,2.31,.025,2.48);
  }
  for(let z=-25;z<10;z+=3.1) for(let x=-27;x<28;x+=3.1) {
    if(Math.abs(x)<8) continue;
    box((Math.round(x+z)%3===0)?'light':'stone',x,.09,z,2.97,.03,2.97);
  }
  // Sanctuary: raised stereobate, a full ring of columns, cella and pediments.
  blocked(0,-41,25,31);
  for(let i=0;i<4;i++) box(i%2?'light':'stone',0,.22+i*.38,-41,27-i*1.0,.44,33-i*1.0);
  box('light',0,5.5,-44,13,8,18);
  box('shadow',0,4.5,-34.94,3.7,5.8,.12);
  box('wood',0,4.1,-34.83,2.8,5.1,.16);
  for(const x of [-10.5,10.5]) for(let z=-54;z<=-28;z+=5.2) column(x,z,8.3,.65,1.6);
  for(const z of [-54,-28]) for(let x=-6.3;x<=6.4;x+=4.2) column(x,z,8.3,.65,1.6);
  box('light',0,11.15,-41,24.6,.8,30.6);
  box('shadow',0,11.73,-41,25.0,.3,31);
  roof(0,11.9,-41,26,32,4.5);
  // Pale triangular pediments with sculptural medallions and miniature figures.
  roof(0,11.97,-24.98,24.2,.4,4.1,'light');
  roof(0,11.97,-57.02,24.2,.4,4.1,'light');
  sphere('bronze',0,13.4,-24.65,.75,1,1,.20);
  for(const x of [-7,-4,4,7]) {cyl('shadow',x,12.8,-24.65,.18,.25,1.3,7);sphere('light',x,13.65,-24.65,.25);}
  for(const x of [-11,0,11]) sphere('light',x,12.7+(x===0?4.2:0),-25,.35);
  // Temple approach flanked by braziers and votive statues.
  for(const x of [-10,10]) {
    box('stone',x,.8,-21,2,1.6,2);cyl('bronze',x,2.0,-21,.55,.25,.7);
    for(const z of [-17,9]) {box('light',x,1.1,z,1.6,2.2,1.6);cyl('white',x,3,z,.34,.48,1.8,8);sphere('white',x,4.15,z,.42);}
  }
  // Civic colonnade on the western forum edge.
  blocked(-31,-9,8,38);
  box('light',-31,.35,-9,9,.7,39);
  box('plaster',-34,4,-9,2,7,38);
  for(let z=-26;z<=9;z+=4.3) column(-28,z,5.7,.42,.7);
  box('light',-31,7.5,-9,9,.7,39);roof(-31,7.85,-9,10,40,1.8);
  // Fountain with stepped octagonal basin, water and central sculptural spout.
  blocked(19,-7,9,9);
  cyl('stone',19,.22,-7,5.0,5.1,.44,12);
  cyl('light',19,.58,-7,4.5,4.7,.7,12);
  cyl('water',19,.96,-7,4.08,4.08,.045,32);
  cyl('light',19,1.7,-7,.5,.8,2.0,12);
  cyl('light',19,2.7,-7,1.8,.6,.45,16);
  cyl('water',19,2.95,-7,1.52,1.52,.04,24);
  sphere('bronze',19,3.55,-7,.6,.65,1.3,.65);
  // Radial seats and pots around the forum.
  for(const z of [-20,6]) for(const x of [-20,20]) {box('light',x,.65,z,4,.35,1.2);box('shadow',x-1.4,.3,z,.5,.6,1);box('shadow',x+1.4,.3,z,.5,.6,1);}
  function house(x,z,w,d,h,index) {
    blocked(x,z,w+.25,d+.25);
    const color=['plaster','peach','white'][index%3];
    box(color,x,h/2,z,w,h,d);
    box('shadow',x,.25,z,w+.25,.5,d+.25);
    box('light',x,h-.2,z,w+.18,.4,d+.18);
    roof(x,h,z,w+.9,d+.9,1.7+random()*.8);
    // South facade faces the approaching visitor; dark inset doors and shutters.
    const front=z+d/2+.055;
    box('dark',x,h*.26,front,1.55,h*.51,.12);
    box('wood',x+.13,h*.25,front+.08,1.25,h*.48,.12);
    box('light',x,h*.54,front+.03,1.9,.28,.32);
    for(const dx of [-w*.30,w*.30]) {
      box('dark',x+dx,h*.7,front,1.2,1.5,.13);
      box('wood',x+dx-.47,h*.7,front+.12,.29,1.55,.16);
      box('wood',x+dx+.47,h*.7,front+.12,.29,1.55,.16);
      box('light',x+dx,h*.7-.84,front+.15,1.5,.18,.38);
    }
    // Side windows make narrow lanes convincing from every direction.
    for(const side of [-1,1]) for(const zz of [-d*.26,d*.26]) box('dark',x+side*(w/2+.04),h*.65,z+zz,.10,1.35,1.05);
    if(index%3===0) {box('wood',x,h*.52,front+.68,w*.76,.22,1.5);for(let dx=-w*.32;dx<=w*.33;dx+=.65)box('wood',x+dx,h*.61,front+1.35,.10,1.2,.10);box('wood',x,h*.72,front+1.35,w*.76,.11,.12);}
    if(index%4===0) {box('plaster',x+w*.28,h+1,z-d*.23,1.05,2,1.1);box('light',x+w*.28,h+2,z-d*.23,1.3,.23,1.35);}
    if(index%2===0) amphora(x+w*.43,front+.6,.7);
  }
  // Multiple distinct blocks leave the avenue and lateral lanes open.
  let hi=0;
  for(const side of [-1,1]) {
    for(const z of [23,42,-39,-62]) for(const xx of [20,38,57]) {
      if(side<0 && xx===57 && z<0)continue;
      house(side*xx,z,10+random()*3,10+random()*3,5.5+random()*4,hi++);
    }
    for(const z of [2,-18]) house(side*52,z,12,12,6+random()*3,hi++);
  }
  // A vivid market occupies the eastern side of the avenue.
  for(let j=0;j<6;j++) {
    const x=11.2+(j%2)*14,z=17+Math.floor(j/2)*11;
    box('wood',x,.9,z,5,1.6,2.2);box('light',x,1.8,z,5.2,.18,2.4);
    for(const dx of [-2.5,2.5])for(const dz of [-1.3,1.3])cyl('wood',x+dx,1.8,z+dz,.075,.075,3.6,6);
    for(let k=0;k<8;k++)box(k%2?'white':j%2?'blue':'red',x-2.3+k*.66,3.7,z,.68,.12,3.5);
    collisionShapes.push(boxShape(x+.01,3.7,z,5.3,.12,3.5));
    for(let k=0;k<6;k++)sphere(k%2?'roofLight':'leafLight',x-1.8+k*.65,2.03,z,.26);
    for(let k=0;k<3;k++)amphora(x-1.8+k*1.2,z+2,.62);
    blocked(x,z,5.4,3);
  }
  // Elevated aqueduct on the western edge, assembled from genuine arch voussoirs.
  for(let z=-65;z<49;z+=10) {
    box('stone',-65,4.6,z,3.4,9.2,2.0);
    box('light',-65,.25,z,4.2,.5,3);
    // Arch plane is YZ. Each wedge is a separate solid segment, then merged.
    const arch=new THREE.TorusGeometry(4, .67, 5, 16, Math.PI);
    add(arch,'light',-65,7.6,z+5,0,Math.PI/2);
    collisionShapes.push(...archShapes(-65,7.6,z+5,4,.67,16,Math.PI/2));
    box('stone',-65,12.1,z+5,3.6,1.6,10);
    box('light',-66.55,13.15,z+5,.45,.7,10);box('light',-63.45,13.15,z+5,.45,.7,10);
    blocked(-65,z,3.4,2);
  }
  // Garden, pergola, trees, and a tiled reflecting pool in the northeastern court.
  box('sand',44,.07,-47,21,.14,24);
  box('light',44,.25,-47,10,.5,15);box('water',44,.52,-47,8.8,.04,13.8);blocked(44,-47,10,15);
  for(const x of [35,53])for(const z of [-57,-37])cypress(x,z,1.1);
  for(let x=32;x<=56;x+=4) {column(x,-30,3.6,.23);box('wood',x,4.7,-30,.24,.24,5);}
  box('wood',44,4.55,-28,27,.22,.26);box('wood',44,4.55,-32,27,.22,.26);
  for(let i=0;i<12;i++)sphere('leaf',33+i*2,4.8,-30,1.1,1.2,.28,1.7);
  for(const side of [-1,1])for(const z of [14,34,54])cypress(side*8.8,z,.75);
  for(const p of [[-43,15],[-47,51],[43,53],[-45,-49],[29,-66],[-25,-69],[60,-73]])olive(p[0],p[1],1+random()*.25);
  for(const x of [-76,76])for(let z=-75;z<=55;z+=16)cypress(x,z,.9+random()*.3);
  // Perimeter walls and a monumental southern entrance, with an open central gate.
  for(const x of [-83,83]) {box('shadow',x,2.7,-13,2,5.4,152);blocked(x,-13,2,152);}
  for(const z of [-89,63])for(const x of [-46,46]){box('stone',x,2.5,z,74,5,2);blocked(x,z,74,2);}
  for(const x of [-10,10]) {box('light',x,4.1,61,4,8.2,5);box('shadow',x,8.3,61,4.7,.6,5.6);blocked(x,61,4,5);}
  add(new THREE.TorusGeometry(8,.9,6,20,Math.PI),'light',0,6.8,61);
  collisionShapes.push(...archShapes(0,6.8,61,8,.9,20));
  box('stone',0,15.5,61,22,1.4,5);box('light',0,16.3,61,23,.3,5.5);
  // Sparse roof-edge trim and border paving provide scale without texture assets.
  for(const x of [-8.1,8.1])for(let z=12;z<58;z+=1.5)box('light',x,.16,z,.4,.3,1.35);
  let triangles=0,drawCalls=0;
  for(const [key,geometries] of bins) {
    const merged=mergeGeometries(geometries,false);
    if(!merged)throw new Error(`Unable to merge city material ${key}`);
    const mesh=new THREE.Mesh(merged,materials[key]);mesh.name=`Město • ${key}`;
    mesh.castShadow=false;mesh.receiveShadow=true;group.add(mesh);
    triangles+=merged.getAttribute('position').count/3;drawCalls++;
    for(const geometry of geometries)geometry.dispose();
  }
  const landmarks=[
    {name:'Brána města',x:0,z:48},
    {name:'Římské fórum',x:0,z:-8},
    {name:'Chrám',x:0,z:-20},
    {name:'Tržiště',x:0,z:24},
    {name:'Zahrada',x:31,z:-46},
    {name:'Akvadukt',x:-57,z:9}
  ];
  return {group,colliders,collisionShapes,landmarks,stats:{triangles,drawCalls,buildings:hi,materials:drawCalls}};
}
