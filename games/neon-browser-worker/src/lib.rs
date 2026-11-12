//! Browser adapter only. The exact native Neon game runs in a CPU-only WASM instance.
use glam::Mat4;
use neon_core::{Content, FrameInput, Game};
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use webgpu_wasm::scene::{wire::WirePublisher, SceneManager};
// Matrices use the same engine glam types through FrameInput inference.
const STRIDE: usize = 88;
#[wasm_bindgen]
pub struct NeonSimulation {
    scene: SceneManager,
    game: Game,
    publisher: WirePublisher,
    haptics: [f32; 2],
    immersive: bool,
}
#[wasm_bindgen]
impl NeonSimulation {
    #[wasm_bindgen(constructor)]
    pub fn new(names: js_sys::Array, values: js_sys::Array) -> Result<NeonSimulation, JsValue> {
        if names.length() != values.length() || names.length() > 64 {
            return Err(JsValue::from_str("Invalid worker assets"));
        }
        let mut assets = BTreeMap::new();
        let mut size = 0;
        for index in 0..names.length() {
            let name = names
                .get(index)
                .as_string()
                .ok_or_else(|| JsValue::from_str("Invalid asset name"))?;
            let bytes = values
                .get(index)
                .dyn_into::<js_sys::Uint8Array>()
                .map_err(|_| error("Asset is not Uint8Array"))?
                .to_vec();
            size += bytes.len();
            if size > 64 * 1024 * 1024 {
                return Err(JsValue::from_str("Worker asset budget exceeded"));
            }
            assets.insert(name, bytes);
        }
        let mut scene = SceneManager::default();
        let content = Content::load(&mut scene, &|name| {
            assets
                .get(name)
                .cloned()
                .ok_or_else(|| format!("Missing worker asset {name}"))
        })
        .map_err(error)?;
        let game = Game::new(&mut scene, &content, None).map_err(error)?;
        Ok(Self {
            scene,
            game,
            publisher: WirePublisher::default(),
            haptics: [0.; 2],
            immersive: false,
        })
    }
    pub fn refs(&self) -> Result<String, JsValue> {
        let mut refs = serde_json::Map::new();
        for name in [
            "sword",
            "leftSword",
            "infoPanel",
            "timerPanel",
            "scorePanel",
            "restartButton",
            "scoreDisplay",
        ] {
            refs.insert(name.into(), self.game.ref_id(name).map_err(error)?.into());
        }
        Ok(serde_json::Value::Object(refs).to_string())
    }
    pub fn set_immersive(&mut self, immersive: bool) {
        self.immersive = immersive;
    }
    pub fn initial(&mut self) -> Result<Vec<u8>, JsValue> {
        self.scene.set_text_facing(self.immersive);
        self.publisher.capture(&mut self.scene).map_err(error)
    }
    /// Packed original input samples, never a JS gameplay/physics loop.
    pub fn tick(&mut self, samples: &[f32]) -> Result<Vec<u8>, JsValue> {
        if samples.len() % STRIDE != 0 || samples.len() > 128 * STRIDE {
            return Err(error("Invalid worker input batch"));
        }
        for values in samples.chunks_exact(STRIDE) {
            let bits = values[1] as u32;
            let buttons = values[3] as u32;
            let pose = |bit, offset| {
                if bits & bit != 0 {
                    Some(Mat4::from_cols_array(
                        values[offset..offset + 16].try_into().unwrap(),
                    ))
                } else {
                    None
                }
            };
            if buttons & 2 != 0 {
                self.game.active = false;
            }
            if buttons & 4 != 0 {
                self.game.reset_tracking(&mut self.scene);
            }
            let mut frame = FrameInput {
                dt: values[0],
                head: pose(1, 8),
                hands: [pose(2, 24), pose(4, 40)],
                rays: [pose(8, 56), pose(16, 72)],
                axes: [values[4], values[5], values[6]],
                triggers: [values[2] as u32 & 1 != 0, values[2] as u32 & 2 != 0],
                primary: buttons & 1 != 0,
            };
            frame.dt = if frame.dt.is_finite() && frame.dt > 0. && frame.head.is_some() {
                frame.dt.min(0.1)
            } else {
                0.
            };
            self.scene.update(frame.dt);
            self.game.tick(&mut self.scene, frame).map_err(error)?;
            for i in 0..2 {
                self.haptics[i] = self.haptics[i].max(self.game.haptics[i]);
            }
            self.game.haptics = [0.; 2];
        }
        self.scene.set_text_facing(self.immersive);
        self.publisher.capture(&mut self.scene).map_err(error)
    }
    pub fn restore_scores(&mut self, json: &str) -> Result<(), JsValue> {
        let scores: Vec<u64> = serde_json::from_str(json).map_err(|e| error(e.to_string()))?;
        if scores.len() > 100 {
            return Err(error("Invalid saved scores"));
        }
        self.game
            .restore_scores(&mut self.scene, &scores)
            .map_err(error)
    }
    pub fn state(&self) -> String {
        self.game.state().to_string()
    }
    pub fn take_haptics(&mut self) -> Vec<f32> {
        std::mem::replace(&mut self.haptics, [0.; 2]).to_vec()
    }
}
fn error(value: impl AsRef<str>) -> JsValue {
    JsValue::from_str(value.as_ref())
}
