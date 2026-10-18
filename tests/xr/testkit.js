// Reusable scenario/report layer. Contains no game rules or renderer replacement.
export class XRTestRun {
    constructor(id, simulator, onUpdate = () => {}) {
        this.simulator = simulator;
        this.onUpdate = onUpdate;
        this.report = {version:1, suite:id, environment:'simulated-webxr-real-rust-renderer', status:'RUNNING', profile:simulator.profile, checks:[], trace:null};
    }
    assert(condition, name, details) {
        let check = this.report.checks.find(check => check.name === name);
        if (!check) { check = {name,status:'PASS',observations:0}; this.report.checks.push(check); }
        check.observations++;
        if (!condition) { check.status='FAIL'; if(details!==undefined)check.details=details; }
        this.onUpdate(this.report);
        if (!condition) throw new Error(name+(details===undefined?'':`: ${JSON.stringify(details)}`));
    }
    async execute(callback) {
        try { await callback(this); }
        catch(error) { this.report.error=String(error.stack||error); }
        if(!this.report.checks.length&&!this.report.error)this.report.error='Scenario performed no assertions';
        this.report.status=this.report.error || this.report.checks.some(check=>check.status==='FAIL')?'FAIL':'PASS';
        this.report.trace=this.simulator.recording();
        this.onUpdate(this.report);
        return this.report;
    }
}

export function publishXRReport(report, {result=document.querySelector('#result'), checks=document.querySelector('#checks')}={}) {
    window.xrTestReport=report;
    const passed=report.checks.filter(check=>check.status==='PASS').length;
    if(result)result.textContent=`${report.status}: ${report.suite} · ${passed}/${report.checks.length} kontrol · simulované XR, skutečný Rust renderer${report.error?'\n'+report.error:''}`;
    if(checks) {
        checks.replaceChildren(...report.checks.map(check=>{
            const item=document.createElement('li');
            item.textContent=`${check.status} · ${check.name}${check.observations>1?` (×${check.observations})`:''}`;
            return item;
        }));
    }
}

export function enableReportDownload(button, getReport=()=>window.xrTestReport) {
    button.addEventListener('click',()=>{
        const report=getReport();if(!report)return;
        const url=URL.createObjectURL(new Blob([JSON.stringify(report,null,2)],{type:'application/json'}));
        const link=document.createElement('a');link.href=url;link.download=`${report.suite}-report.json`;link.click();
        setTimeout(()=>URL.revokeObjectURL(url),1000);
    });
}

export function pixelDifference(a,b,threshold=20) {
    if(a.width!==b.width || a.height!==b.height)throw new Error('Image dimensions differ');
    let changed=0;
    for(let i=0;i<a.data.length;i+=4)if(Math.abs(a.data[i]-b.data[i])+Math.abs(a.data[i+1]-b.data[i+1])+Math.abs(a.data[i+2]-b.data[i+2])>threshold)changed++;
    return changed;
}
