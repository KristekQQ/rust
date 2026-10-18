// Offline model authoring. Runtime imports textured real geometry through Rust.
import {writeFile} from 'node:fs/promises';
const assetRoot=new URL('./assets/',import.meta.url);
function author(name,build){
    const groups=[[],[],[]];
    const prism=(w,d,bottom,top,bevel=0,material=0,x=0,z=0)=>{
        const c=Math.min(bevel,w/3,d/3),hw=w/2,hd=d/2;
        const p=[[-hw+c,-hd],[hw-c,-hd],[hw,-hd+c],[hw,hd-c],[hw-c,hd],[-hw+c,hd],[-hw,hd-c],[-hw,-hd+c]];
        const triangle=(a,b,c,normal)=>{
            for(const v of [a,b,c])groups[material].push(...v,...normal,v[0]*2,v[1]*2+v[2]*2);
        };
        const quad=(vertices,normal)=>{
            const [a,b,c]=vertices,ab=b.map((v,i)=>v-a[i]),ac=c.map((v,i)=>v-a[i]);
            const cross=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];
            const indices=cross.reduce((n,v,i)=>n+v*normal[i],0)>0?[0,1,2,0,2,3]:[0,2,1,0,3,2];
            for(let i=0;i<6;i+=3)triangle(vertices[indices[i]],vertices[indices[i+1]],vertices[indices[i+2]],normal);
        };
        for(let i=0;i<8;i++){
            const a=p[i],b=p[(i+1)%8],dx=b[0]-a[0],dz=b[1]-a[1],length=Math.hypot(dx,dz);
            if(length<1e-8)continue;
            quad([[a[0]+x,bottom,a[1]+z],[b[0]+x,bottom,b[1]+z],[b[0]+x,top,b[1]+z],[a[0]+x,top,a[1]+z]],[dz/length,0,-dx/length]);
            triangle([x,top,z],[b[0]+x,top,b[1]+z],[a[0]+x,top,a[1]+z],[0,1,0]);
            triangle([x,bottom,z],[a[0]+x,bottom,a[1]+z],[b[0]+x,bottom,b[1]+z],[0,-1,0]);
        }
    };
    build(prism);
    const buffers=[],bufferViews=[],accessors=[],primitives=[];
    let offset=0;
    const accessor=(values,components)=>{
        const data=Buffer.from(new Float32Array(values).buffer),view=bufferViews.length;
        buffers.push(data);bufferViews.push({buffer:0,byteOffset:offset,byteLength:data.length,target:34962});offset+=data.length;
        const min=Array(components).fill(Infinity),max=Array(components).fill(-Infinity);
        values.forEach((v,i)=>{min[i%components]=Math.min(min[i%components],v);max[i%components]=Math.max(max[i%components],v);});
        const index=accessors.length;
        accessors.push({bufferView:view,componentType:5126,count:values.length/components,type:components===3?'VEC3':'VEC2',min,max});return index;
    };
    groups.forEach((vertices,material)=>{
        if(!vertices.length)return;
        const positions=[],normals=[],uv=[];
        for(let i=0;i<vertices.length;i+=8){positions.push(...vertices.slice(i,i+3));normals.push(...vertices.slice(i+3,i+6));uv.push(...vertices.slice(i+6,i+8));}
        primitives.push({attributes:{POSITION:accessor(positions,3),NORMAL:accessor(normals,3),TEXCOORD_0:accessor(uv,2)},material});
    });
    const gltf={asset:{version:'2.0',generator:'Neon arena offline monument authoring'},scene:0,scenes:[{nodes:[0]}],nodes:[{name,mesh:0}],meshes:[{primitives}],materials:[
        {name:'Honed charcoal granite',pbrMetallicRoughness:{baseColorTexture:{index:0},metallicFactor:.08,roughnessFactor:.78}},
        {name:'Brushed bronze',pbrMetallicRoughness:{baseColorFactor:[.46,.28,.10,1],metallicFactor:.65,roughnessFactor:.38}},
        {name:'Dark display housing',pbrMetallicRoughness:{baseColorFactor:[.018,.028,.045,1],metallicFactor:.45,roughnessFactor:.45}},
    ],images:[{uri:'granite.png'}],textures:[{source:0,sampler:0}],samplers:[{magFilter:9729,minFilter:9729,wrapS:10497,wrapT:10497}],buffers:[{uri:'data:application/octet-stream;base64,'+Buffer.concat(buffers).toString('base64'),byteLength:offset}],bufferViews,accessors};
    if(!groups[0].length){
        delete gltf.images;delete gltf.textures;delete gltf.samplers;
        delete gltf.materials[0].pbrMetallicRoughness.baseColorTexture;
    }
    return writeFile(new URL(name+'.gltf',assetRoot),JSON.stringify(gltf));
}
await author('codex-monument',prism=>{
    prism(1.64,.96,0,.18,.09);prism(1.43,.78,.18,.36,.07);
    prism(1.30,.44,.36,2.31,.07);prism(1.40,.56,2.31,2.43,.07);
    // A recessed bronze bezel around the diagnostic screen, stone behind it.
    prism(1.14,.025,.61,.64,.01,1,0,.232);
    prism(1.14,.025,1.57,1.60,.01,1,0,.232);
    for(const x of [-.56,.56])prism(.025,.025,.64,1.57,.004,1,x,.232);
    prism(.86,.018,1.75,1.77,.005,1,0,.230);
});
await author('score-display',prism=>{
    prism(2.40,.14,.44,.82,.025,2);
    prism(2.40,.02,.44,.46,.005,1,0,.075);
    prism(2.40,.02,.80,.82,.005,1,0,.075);
    for(const x of [-1.19,1.19])prism(.025,.02,.46,.80,.004,1,x,.075);
    prism(1.52,.14,-.37,.37,.025,2);
    prism(1.52,.02,-.37,-.345,.005,1,0,.075);
    prism(1.52,.02,.345,.37,.005,1,0,.075);
    for(const x of [-.75,.75])prism(.025,.02,-.345,.345,.004,1,x,.075);
});
console.log('Textured stone monument and bronze score display authored');
