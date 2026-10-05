// Separate example for developers; not loaded by the default page.
import { initEngine } from "../js/engine.js";
import { createScene } from "./basic/scene.js";

const canvas = document.getElementById("gpu-canvas");
if (!(canvas instanceof HTMLCanvasElement)) throw new Error("Canvas not found");
const engine = await initEngine(canvas);
createScene(engine.scene);

// JavaScript/TypeScript issues commands; Rust owns state and runs the animation.
const cube = engine.scene.createCube([-2, 0, 0]);
cube.setScale([0.7, 0.7, 0.7]);
cube.rotate([0, 180, 0], { duration: 2 });
cube.removeAfter(8);
