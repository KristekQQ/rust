const status = document.getElementById("status");
try {
    const version = Date.now();
    const { initEngine } = await import(`./engine.js?v=${version}`);
    const { setupScene } = await import(`./scene.js?v=${version}`);
    const canvas = document.getElementById("gpu-canvas");
    const engine = await initEngine(canvas, {
        backend: new URLSearchParams(location.search).get("backend") || "auto",
    });

    window.engine = engine;
    // Keep the existing console API, with an explicit reset of the sample scene.
    window.scene = Object.freeze({ ...engine, reset_scene: () => setupScene(engine) });
    setupScene(engine);
    engine.set_grid_visible(true);
    let grid = true;

    document.getElementById("grid-btn").onclick = () => {
        grid = !grid;
        engine.set_grid_visible(grid);
        document.getElementById("grid-btn").textContent = grid ? "Skrýt mřížku" : "Zobrazit mřížku";
        document.getElementById("grid-btn").setAttribute("aria-pressed", String(grid));
    };
    for (const mode of ["orbit", "free"]) {
        document.getElementById(`${mode}-btn`).onclick = () => {
            engine.set_camera_mode(mode);
            for (const candidate of ["orbit", "free"]) {
                document.getElementById(`${candidate}-btn`).setAttribute("aria-pressed", String(candidate === mode));
            }
            canvas.focus();
        };
    }
    document.getElementById("reset-btn").onclick = () => setupScene(engine);
    window.addEventListener("resize", () => {
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
        engine.resize(canvas.width, canvas.height);
    });
    status.textContent = `Scéna běží · ${engine.renderer_backend() === "BrowserWebGpu" ? "WebGPU" : "WebGL"}`;
    for (const button of document.querySelectorAll("button")) button.disabled = false;
} catch (error) {
    console.error(error);
    status.textContent = `Scénu se nepodařilo spustit: ${error instanceof Error ? error.message : String(error)}. Použijte prohlížeč s WebGPU nebo WebGL2 a ověřte sestavení pkg/.`;
}
