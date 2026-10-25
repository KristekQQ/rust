// 1. Renderer/SDK, definice scény a volitelné vývojové UI jsou oddělené moduly.
import { initEngine } from "../shared/runtime-info.js";
import { createScene } from "./scene.js";
import { createTools } from "../shared/tools.js";

// 2. HTML poskytne canvas. Samotné vytvoření GPU a frame loop provede Rust/WASM.
const canvas = document.getElementById("gpu-canvas");
if (!(canvas instanceof HTMLCanvasElement)) throw new Error("Canvas nebyl nalezen");
const backend = new URLSearchParams(location.search).get("backend") === "webgl" ? "webgl" : "auto";
export const engine = await initEngine(canvas, { backend });

// 3. Nastavíme pohled a výstupy. Tato konfigurace patří aplikaci, ne debug UI.
engine.set_camera_mode("orbit");
engine.set_render_effects(false, false);
engine.set_grid_visible(true);
engine.set_light_helpers_visible(false);

createScene(engine.scene);
const tools = createTools(document.getElementById("tools"), engine, {
    title: "01 · Základní scéna",
    defaults: {shadows: false, mirror: false, grid: true, helpers: false},
    onReset: () => createScene(engine.scene),
});
const fit = () => engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight));
fit();
window.addEventListener("resize", fit);
window.addEventListener("pagehide", () => {
    tools.dispose();
    engine.dispose();
    window.removeEventListener("resize", fit);
}, { once: true });
