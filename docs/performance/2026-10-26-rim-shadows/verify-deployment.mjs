import fs from 'node:fs/promises';
import path from 'node:path';
import {homedir} from 'node:os';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {assertFreshBuild} from '../../../scripts/build-fingerprint.mjs';
import {assertProductionBuild} from '../../../scripts/build-production.mjs';
const root=process.cwd(),out=path.join(root,'docs/performance/2026-10-26-rim-shadows/evidence/deployment'),base='https://192.168.11.134:8443';
const caFile=process.env.QUEST_CA_FILE||path.join(homedir(),'.local/share/rust-webxr/tls/rootCA.crt');
const expected=JSON.parse(await fs.readFile('output/production-rim-blob-verified/production-manifest.json','utf8'));
const actual=await assertProductionBuild(path.join(root,'output/production'),{checkCurrentSource:true});
if(actual.releaseId!==expected.releaseId||JSON.stringify(actual.files)!==JSON.stringify(expected.files))throw Error('Live package differs from browser-verified package');
const buildBefore=await assertFreshBuild(),files=[];
for(const file of ['examples/rim/index.html','examples/rim/main.js','examples/rim/scene.js','js/engine.js','pkg/webgpu_wasm.js','pkg/webgpu_wasm_bg.wasm']){
 const record=expected.files.find(r=>r.path===file);if(!record)throw Error('Missing manifest file '+file);
 const header=path.join(out,'served-'+file.replaceAll('/','_')+'.headers');
 const bytes=execFileSync('/usr/bin/curl',['--cacert',caFile,'--fail','--silent','--show-error','--compressed','--connect-timeout','5','--max-time','20','-D',header,base+'/'+file],{maxBuffer:16*1024*1024});
 const sha256=createHash('sha256').update(bytes).digest('hex');if(sha256!==record.sha256)throw Error('Served bytes mismatch '+file);
 const headers=await fs.readFile(header,'utf8');if(!/^cache-control:.*no-store/im.test(headers))throw Error('Missing no-store '+file);
 if(file.endsWith('index.html')&&(!bytes.includes(Buffer.from(`name="engine-build" content="${expected.build.wasm.slice(0,8)}"`))||!bytes.includes(Buffer.from(`name="app-build" content="${expected.releaseId}"`))))throw Error('Incorrect visible build markers');
 files.push({path:file,sha256,bytes:bytes.length,cacheControlNoStore:true,tlsVerified:true});
}
const buildAfter=await assertFreshBuild();if(JSON.stringify(buildBefore)!==JSON.stringify(buildAfter))throw Error('Build changed during deployment check');
const report={pass:true,date:new Date().toISOString(),base,releaseId:expected.releaseId,build:buildBefore,files,trust:'Existing local development CA explicitly supplied; system/Quest trust not established by this check.',scope:'Verified HTTPS certificate chain and hostname, actual served decoded payload bytes and no-store headers matched the same production package tested on WebGPU/WebGL. This does not verify physical Quest performance or CI.'};
await fs.writeFile(path.join(out,'deployment-check.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({pass:true,base,releaseId:expected.releaseId,wasm:buildBefore.wasm,files:files.length}));
