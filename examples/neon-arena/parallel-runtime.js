// Asset loading and WASM calls only. The native and browser games share neon-core.
export async function createSimulation(assetBase,scores=[],assetOverrides=new Map()){
    const module=await import('../../pkg-neon-worker/neon_browser_worker.js');await module.default();
    const files=new Map();
    const read=async name=>{
        if(files.has(name))return files.get(name);
        if(assetOverrides.has(name)){const bytes=assetOverrides.get(name);files.set(name,bytes);return bytes;}
        const response=await fetch(new URL(name,assetBase));if(!response.ok)throw new Error(`Asset ${name}: ${response.status}`);
        const bytes=new Uint8Array(await response.arrayBuffer());files.set(name,bytes);return bytes;
    };
    const manifest=JSON.parse(new TextDecoder().decode(await read('scene.json')));
    for(const operation of manifest.operations){
        if(!['loadFont3D','loadModel'].includes(operation.op))continue;
        const file=operation.args[0],bytes=await read(file);
        if(file.endsWith('.gltf')){
            const document=JSON.parse(new TextDecoder().decode(bytes)),directory=file.slice(0,file.lastIndexOf('/')+1);
            for(const item of [...(document.buffers??[]),...(document.images??[])])if(item.uri&&!item.uri.startsWith('data:'))await read(directory+item.uri);
        }
    }
    const game=new module.NeonSimulation([...files.keys()],[...files.values()]);game.restore_scores(JSON.stringify(scores));return game;
}
