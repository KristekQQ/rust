/**
 * Prohlížeč spravuje fullscreen a zachycení kurzoru. Pohyb zůstává v Rustu.
 * @param {HTMLCanvasElement} canvas
 * @param {import('../../js/engine.js').Engine} engine
 * @param {HTMLElement|null} status
 */
export function createGameInput(canvas, engine, status) {
    let ready = false;
    let disposed = false;
    /** @param {string} message */
    const show = message => { if (status) status.textContent = message; };

    const enter = () => {
        if (!ready || disposed || document.pointerLockElement === canvas) return;
        canvas.focus();
        // Obě žádosti pocházejí přímo z kliknutí. Fullscreen spotřebuje aktivaci,
        // proto nejdřív požádáme o zachycení kurzoru a nečekáme mezi žádostmi.
        const lock = Promise.resolve(canvas.requestPointerLock());
        const fullscreen = document.fullscreenElement === canvas
            ? Promise.resolve()
            : canvas.requestFullscreen();
        void Promise.allSettled([lock, fullscreen]).then(results => {
            if (disposed) return;
            if (results[0].status === 'rejected') {
                show('Kurzor se nepodařilo zachytit. Klikni do hry znovu.');
                if (document.fullscreenElement === canvas) void document.exitFullscreen().catch(() => {});
            } else if (results[1].status === 'rejected') {
                show('WASD: pohyb · Shift: běh · myš: kamera · Esc: uvolnit kurzor (fullscreen není dostupný)');
            }
        });
    };

    const lockChanged = () => {
        if (disposed || !ready) return;
        const playing = document.pointerLockElement === canvas;
        engine.set_simulation_paused(!playing);
        show(playing
            ? 'WASD: pohyb · Space: skok · Shift: běh · R: návrat · myš: kamera · Esc: opustit hru'
            : 'Klikni do hry: fullscreen a ovládání myší.');
        if (!playing && document.fullscreenElement === canvas) {
            void document.exitFullscreen().catch(() => {});
        }
    };
    const fullscreenChanged = () => {
        if (document.fullscreenElement !== canvas && document.pointerLockElement === canvas) {
            document.exitPointerLock();
        }
    };
    const leave = () => {
        if (document.pointerLockElement === canvas) document.exitPointerLock();
        if (document.fullscreenElement === canvas) void document.exitFullscreen().catch(() => {});
    };
    /** @param {KeyboardEvent} event */
    const escape = event => {
        if (event.code === 'Escape') leave();
        if (event.code === 'Space' && document.pointerLockElement === canvas) event.preventDefault();
    };

    canvas.addEventListener('click', enter);
    document.addEventListener('pointerlockchange', lockChanged);
    document.addEventListener('fullscreenchange', fullscreenChanged);
    document.addEventListener('keydown', escape);
    window.addEventListener('blur', leave);

    return {
        enable() {
            ready = true;
            engine.set_simulation_paused(true);
            show('Klikni do hry: fullscreen a ovládání myší.');
        },
        dispose() {
            if (disposed) return;
            disposed = true;
            canvas.removeEventListener('click', enter);
            document.removeEventListener('pointerlockchange', lockChanged);
            document.removeEventListener('fullscreenchange', fullscreenChanged);
            document.removeEventListener('keydown', escape);
            window.removeEventListener('blur', leave);
            if (document.pointerLockElement === canvas) document.exitPointerLock();
            if (document.fullscreenElement === canvas) void document.exitFullscreen().catch(() => {});
        },
    };
}
