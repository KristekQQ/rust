import { createScene } from "../examples/basic/scene.js";

// Hlavní stránka používá stejný příklad jako basic/index.html.
// Sestavení proběhne při startu a při resetu; další snímky vykresluje Rust.
export function setupScene(engine) {
    createScene(engine.scene);
}
