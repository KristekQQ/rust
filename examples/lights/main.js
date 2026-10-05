// 1. Renderer/SDK, definice scény a volitelné vývojové UI jsou oddělené moduly.
import { initEngine } from "../../js/engine.js";
import { createScene } from "./scene.js";
import { createTools } from "../shared/tools.js";

// 2. HTML poskytne canvas. Samotné vytvoření GPU a frame loop provede Rust/WASM.
const canvas = document.getElementById("gpu-canvas");
if (!(canvas instanceof HTMLCanvasElement)) throw new Error("Canvas nebyl nalezen");
const backend = new URLSearchParams(location.search).get("backend") === "webgl" ? "webgl" : "auto";
const engine = await initEngine(canvas, { backend });

// 3. Nastavíme pohled a výstupy. Tato konfigurace patří aplikaci, ne debug UI.
engine.set_camera_mode("orbit");
engine.set_render_effects(true, false);
engine.set_grid_visible(false);
engine.set_light_helpers_visible(true);

// 4. Jednou vytvoříme scénu přes SDK. Žádná vlastní requestAnimationFrame smyčka.
createScene(engine.scene);

// 5. Volitelný nástrojový panel. V hotové hře tento import a volání odstraníš.
const tools = createTools(document.getElementById("tools"), engine, {
    title: "04 · Pohyblivá světla",
    defaults: {shadows: true, mirror: false, grid: false, helpers: true},
    onReset: () => createScene(engine.scene),
});

// 6. Rozměr canvasu sledujeme nezávisle na nástrojovém panelu.
const fit = () => engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight));
fit();
window.addEventListener("resize", fit);
window.addEventListener("pagehide", () => {
    tools.dispose();
    window.removeEventListener("resize", fit);
}, { once: true });
