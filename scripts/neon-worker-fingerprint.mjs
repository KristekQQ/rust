import {createHash} from 'node:crypto';
import {readFile,readdir,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {assertFreshBuild,root} from './build-fingerprint.mjs';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
async function files(dir,directory=root){const entries=await readdir(path.join(directory,dir),{withFileTypes:true});return(await Promise.all(entries.map(e=>e.isDirectory()?files(path.posix.join(dir,e.name),directory):[path.posix.join(dir,e.name)]))).flat();}
export async function workerIdentity({directory=root,engine}={}){
 engine??=await assertFreshBuild();
 const inputs=[...await files('games/neon-core/src',directory),...await files('games/neon-browser-worker/src',directory),'games/neon-core/Cargo.toml','games/neon-browser-worker/Cargo.toml','games/neon-browser-worker/Cargo.lock'].sort();
 const digest=createHash('sha256').update(engine.source).update(JSON.stringify(engine.configuration));
 for(const input of inputs)digest.update(input).update('\0').update(await readFile(path.join(directory,input))).update('\0');
 return {version:1,source:digest.digest('hex'),wasm:hash(await readFile(path.join(directory,'pkg-neon-worker/neon_browser_worker_bg.wasm'))),bindings:hash(await readFile(path.join(directory,'pkg-neon-worker/neon_browser_worker.js')))};
}
export async function assertFreshWorkerBuild(){
 const current=await workerIdentity();
 const receipt=path.join(root,'pkg-neon-worker/.verification-build.json');
 if(JSON.stringify(current)!==JSON.stringify(JSON.parse(await readFile(receipt,'utf8'))))throw new Error('Worker WASM/source changed; rebuild neon-browser-worker');
 return current;
}
if(process.argv[1]===fileURLToPath(import.meta.url)){
 try{
  if(process.argv.includes('--write'))await writeFile(path.join(root,'pkg-neon-worker/.verification-build.json'),JSON.stringify(await workerIdentity(),null,2)+'\n');
  else await assertFreshWorkerBuild();
  console.log('Neon shared-game worker build identity PASS');
 }catch(error){console.error(String(error.message??error));process.exitCode=1;}
}
