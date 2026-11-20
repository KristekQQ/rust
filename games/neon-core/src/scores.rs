//! Offline leaderboard shared by the native and web hosts.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const LIMIT: usize = 10;
const MAX_SCORE: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub score: u64,
    pub initials: String,
}

#[derive(Default)]
pub struct Scores {
    pub top: Vec<Entry>,
    path: Option<PathBuf>,
}

impl Scores {
    pub fn load(path: Option<PathBuf>) -> Self {
        let entries = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut scores = Self {
            top: Vec::new(),
            path,
        };
        scores.restore(entries);
        scores
    }

    pub fn restore(&mut self, mut entries: Vec<Entry>) {
        entries.retain(|e| {
            (1..=MAX_SCORE).contains(&e.score)
                && (1..=3).contains(&e.initials.len())
                && e.initials.bytes().all(|c| c.is_ascii_uppercase())
        });
        entries.sort_by_key(|e| std::cmp::Reverse(e.score));
        entries.truncate(LIMIT);
        self.top = entries;
    }

    pub fn qualifies(&self, score: u64) -> bool {
        score > 0
            && (self.top.len() < LIMIT || self.top.last().is_some_and(|last| score > last.score))
    }

    pub fn record(&mut self, entry: Entry) -> Result<(), String> {
        let mut entries = self.top.clone();
        entries.push(entry);
        entries.sort_by_key(|e| std::cmp::Reverse(e.score));
        entries.truncate(LIMIT);
        if let Some(path) = &self.path {
            save(path, &entries)?;
        }
        self.top = entries;
        Ok(())
    }
}

fn save(path: &Path, entries: &[Entry]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(entries).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_board_admits_only_a_better_score_and_keeps_older_ties_first() {
        let mut scores = Scores::default();
        scores.restore(
            (1..=10)
                .map(|score| Entry {
                    score,
                    initials: "AB".into(),
                })
                .collect(),
        );
        assert!(!scores.qualifies(0));
        assert!(!scores.qualifies(1));
        assert!(scores.qualifies(2));
        scores
            .record(Entry {
                score: 2,
                initials: "XYZ".into(),
            })
            .unwrap();
        assert_eq!(scores.top.len(), 10);
        assert_eq!(scores.top[8].initials, "AB");
        assert_eq!(scores.top[9].initials, "XYZ");
    }
}
