//! Operator settings read once before creating the OpenXR reference space.
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceMode {
    #[default]
    Recenterable,
    /// Room-anchored exhibition mode; does not follow ordinary system recenter.
    FixedStage,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub reference_mode: ReferenceMode,
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(settings) => settings,
                Err(error) => {
                    log::warn!("Invalid Neon settings; using recenterable mode: {error}");
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!("Cannot read Neon settings; using recenterable mode: {error}");
                Self::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_mode_is_recenterable_unless_exhibition_is_explicit() {
        assert_eq!(
            Settings::default().reference_mode,
            ReferenceMode::Recenterable
        );
        let empty: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(empty.reference_mode, ReferenceMode::Recenterable);
        let exhibition: Settings =
            serde_json::from_str(r#"{"referenceMode":"fixed-stage"}"#).unwrap();
        assert_eq!(exhibition.reference_mode, ReferenceMode::FixedStage);
        let normal: Settings = serde_json::from_str(r#"{"referenceMode":"recenterable"}"#).unwrap();
        assert_eq!(normal.reference_mode, ReferenceMode::Recenterable);
        assert!(serde_json::from_str::<Settings>(r#"{"referenceMode":"typo"}"#).is_err());
    }
}
