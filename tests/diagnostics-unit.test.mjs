import test from 'node:test';
import assert from 'node:assert/strict';

test('diagnostics retain repeated performance/effect samples while bounding traffic',async t=>{
    const values={document:{documentElement:{dataset:{rendererBackend:'BrowserWebGpu'}}},location:{pathname:'/examples/neon-arena/',search:''},isSecureContext:true,navigator:{userAgent:'test'},fetch:async(_url,options)=>{records.push(JSON.parse(options.body));}};
    const records=[],saved=new Map(Object.keys(values).map(key=>[key,Object.getOwnPropertyDescriptor(globalThis,key)]));
    t.after(()=>{for(const [key,descriptor]of saved){if(descriptor)Object.defineProperty(globalThis,key,descriptor);else delete globalThis[key];}});
    for(const [key,value]of Object.entries(values))Object.defineProperty(globalThis,key,{configurable:true,value});
    const {reportDiagnostic}=await import('../examples/shared/diagnostics.js?test=stream');
    reportDiagnostic('page-load');reportDiagnostic('page-load');
    assert.equal(records.length,1,'ordinary diagnostics still deduplicate');
    reportDiagnostic('xr-performance',undefined,{ssr:true});
    reportDiagnostic('ssr-changed',undefined,{enabled:false});
    reportDiagnostic('xr-performance',undefined,{ssr:false});
    assert.deepEqual(records.slice(1).map(record=>record.stage),['xr-performance','ssr-changed','xr-performance']);
    for(let i=0;i<100;i++)reportDiagnostic('xr-performance',undefined,{sample:i});
    assert.equal(records.length,81,'stream has its own bounded 80-record budget');
});
