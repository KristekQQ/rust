//! Native offline TOP5, persisted atomically in the app's private data directory.
use std::path::{Path, PathBuf};
#[derive(Default)]
pub struct Scores {
    pub top: Vec<u64>,
    path: Option<PathBuf>,
}
impl Scores {
    pub fn load(path: Option<PathBuf>) -> Self {
        let mut top = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Vec<u64>>(&b).ok())
            .unwrap_or_default();
        top.retain(|n| *n <= 9_007_199_254_740_991);
        top.sort_unstable_by(|a, b| b.cmp(a));
        top.truncate(5);
        Self { top, path }
    }
    pub fn record(&mut self, score: u64) -> Result<(), String> {
        self.top.push(score.min(9_007_199_254_740_991));
        self.top.sort_unstable_by(|a, b| b.cmp(a));
        self.top.truncate(5);
        if let Some(path) = &self.path {
            save(path, &self.top)?;
        }
        Ok(())
    }
}
fn save(path: &Path, top: &[u64]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(top).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, path).map_err(|e| e.to_string())
}
