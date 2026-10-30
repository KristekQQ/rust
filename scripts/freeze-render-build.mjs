#!/usr/bin/env node
// An immutable local release snapshot; no Git mutations and no live-package writes.
import fs from 'node:fs/promises';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {root,assertFreshBuild} from './build-fingerprint.mjs';
const runtimeOnly=process.argv.includes('--runtime-only');
const args=process.argv.slice(2).filter(arg=>arg!=='--runtime-only');
if(args.length!==1||args[0]==='--help'){
 console.log('Usage: VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs [--runtime-only] OUTPUT_ROOT\nRefuses an existing destination; captures verified release WASM, runtime/assets, source and tool hashes. --runtime-only omits historical docs/evidence archives, never runtime files.');
 process.exit(args[0]==='--help'?0:1);
}
process.env.VERIFY_WASM_PROFILE||='release';
const build=await assertFreshBuild();
if(build.configuration.profile!=='release')throw Error('A verified release build is required');
const destination=path.resolve(args[0]);
if(!destination.startsWith(path.join(root,'output')+path.sep))throw Error('Use an isolated destination under output/');
try{await fs.stat(destination);throw Error('Destination already exists; choose a new snapshot name');}catch(e){if(e.code!=='ENOENT')throw e;}
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const inputDirs=['pkg','js','examples','assets','tests','src','third_party','scripts',...(runtimeOnly?[]:['docs']),'.agents/skills'];
const inputFiles=['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo/config.toml','package.json','package-lock.json','tsconfig.json','AGENTS.md','MEMORY.md'];
const files=[];
async function list(relative){for(const entry of await fs.readdir(path.join(root,relative),{withFileTypes:true})){
 if(entry.name==='.DS_Store'||entry.name==='__pycache__'||/\.blend\d+$/.test(entry.name))continue;
 const file=path.join(relative,entry.name);
 if(entry.isSymbolicLink())throw Error('Snapshot input is a symlink: '+file);
 if(entry.isDirectory())await list(file);else if(entry.isFile())files.push(file);
}}
for(const dir of inputDirs)await list(dir);files.push(...inputFiles);files.sort();
const manifest={version:1,complete:false,runtimeOnly,omitted:runtimeOnly?['docs (historical documentation and evidence; no runtime inputs)']:[],created:new Date().toISOString(),head:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),branch:execFileSync('git',['branch','--show-current'],{cwd:root,encoding:'utf8'}).trim(),dirty:execFileSync('git',['status','--porcelain'],{cwd:root,encoding:'utf8'}).trim().split('\n').filter(Boolean),build,node:process.version,files:{}};
await fs.mkdir(destination,{recursive:true});
try{
 for(const file of files){const bytes=await fs.readFile(path.join(root,file));manifest.files[file]={sha256:hash(bytes),bytes:bytes.length};await fs.mkdir(path.dirname(path.join(destination,file)),{recursive:true});await fs.writeFile(path.join(destination,file),bytes);}
 for(const file of files){if(hash(await fs.readFile(path.join(root,file)))!==manifest.files[file].sha256)throw Error('Source changed while freezing: '+file);if(hash(await fs.readFile(path.join(destination,file)))!==manifest.files[file].sha256)throw Error('Copied bytes differ: '+file);}
 if(JSON.stringify(build)!==JSON.stringify(await assertFreshBuild()))throw Error('WASM build identity changed while freezing');
 manifest.complete=true;
}catch(e){manifest.error=String(e.stack||e);throw e;}
finally{await fs.writeFile(path.join(destination,'render-build.json'),JSON.stringify(manifest,null,2)+'\n');}
console.log(JSON.stringify({destination,head:manifest.head,wasm:build.wasm,files:files.length,complete:manifest.complete}));
