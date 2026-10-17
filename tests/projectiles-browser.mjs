export async function verifyProjectiles(page) {
    await page.waitForFunction(() => document.documentElement.dataset.rendererBackend);
    const state = () => page.evaluate(async () => {
        const {engine,pool} = await import('./main.js');
        return {count:engine.scene.objectCount,ids:pool.map(slot=>slot.apple.id),
            contacts:engine.projectileContacts(),active:pool.filter(slot=>slot.timer!==0).length,
            positions:pool.slice(0,2).map(slot=>slot.apple.meshInfo.position)};
    });
    const initial=await state();
    if(initial.ids.length!==48||initial.count!==51)throw new Error('Expected fixed pool of 48 apples plus floor/two launchers');
    await page.waitForFunction(async () => (await import('./main.js')).pool.some(slot=>slot.timer!==0));
    await page.click('#collision-test');
    const before=(await state()).contacts;
    await page.waitForFunction(async baseline => (await import('./main.js')).engine.projectileContacts()>baseline,{},before);
    // Once in contact, both pieces must separate outwards instead of crossing.
    await page.waitForFunction(async () => {
        const {pool}=await import('./main.js');
        return pool[0].apple.meshInfo.position[0]<-0.8 && pool[1].apple.meshInfo.position[0]>0.8;
    });
    const hit=await state();
    if(hit.positions[0][0]>=hit.positions[1][0])throw new Error('Apples crossed instead of bouncing');
    await page.keyboard.press('F3');
    await page.waitForFunction(async ()=>(await import('./main.js')).engine.debugStats().enabled);
    const overlap=await page.evaluate(()=>{
        const panel=document.querySelector('aside');
        if(getComputedStyle(panel).display==='none')return false;
        const box=panel.getBoundingClientRect();
        return box.left<320 && box.top<120;
    });
    if(overlap)throw new Error('Controls overlap F3 information');
    await page.keyboard.press('F3');
    await page.click('#reset');
    const reset=await state();
    if(reset.count!==initial.count||JSON.stringify(reset.ids)!==JSON.stringify(initial.ids))throw new Error('Reset recreated or grew the pool');
    return 'PASS: autonomous launch, real bounce, fixed pool/reset, F3 layout';
}
