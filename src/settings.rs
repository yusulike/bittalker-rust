//! Settings persistence (`settings.json`). Port of `src/utils/settings_manager.py`.
//!
//! Missing keys fall back to defaults via `#[serde(default)]`, which mirrors
//! the Python "merge with defaults" behaviour. Unlike the Python dict, unknown
//! keys in the file are dropped when saving.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_INTERVAL: i64 = 50;
pub const DEFAULT_VOICE: &str = "F1";
pub const DEFAULT_LANGUAGE: &str = "Korean";

/// Language code mapping (shared constant in the original).
pub const LANG_CODE_MAP: &[(&str, &str)] = &[
    ("Korean", "ko"),
    ("English", "en"),
    ("Spanish", "es"),
    ("Portuguese", "pt"),
    ("French", "fr"),
];

pub fn lang_code(language: &str) -> &'static str {
    LANG_CODE_MAP
        .iter()
        .find(|(name, _)| *name == language)
        .map(|(_, code)| *code)
        .unwrap_or("ko")
}

pub fn available_languages() -> Vec<&'static str> {
    LANG_CODE_MAP.iter().map(|(name, _)| *name).collect()
}

pub const VOICES: &[&str] = &["F1", "F2", "F3", "F4", "F5", "M1", "M2", "M3", "M4", "M5"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// Alert threshold in USD
    #[serde(default = "default_interval")]
    pub interval: i64,
    /// Voice style (`F1`-`F5`, `M1`-`M5`)
    #[serde(default = "default_voice")]
    pub voice: String,
    /// Notification language
    #[serde(default = "default_language")]
    pub language: String,
    /// Keep the window floating above other apps
    #[serde(default = "default_always_on_top")]
    pub always_on_top: bool,
    /// Disable voice playback
    #[serde(default = "default_muted")]
    pub muted: bool,
    /// Use compact ticker layout
    #[serde(default = "default_ticker_mode")]
    pub ticker_mode: bool,
    /// Saved window position [x, y] in screen points (multi-monitor aware)
    #[serde(default)]
    pub window_pos: Option<[f32; 2]>,
}

fn default_interval() -> i64 {
    DEFAULT_INTERVAL
}
fn default_voice() -> String {
    DEFAULT_VOICE.to_string()
}
fn default_language() -> String {
    DEFAULT_LANGUAGE.to_string()
}
fn default_always_on_top() -> bool {
    true
}
fn default_muted() -> bool {
    false
}
fn default_ticker_mode() -> bool {
    false
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interval: DEFAULT_INTERVAL,
            voice: DEFAULT_VOICE.to_string(),
            language: DEFAULT_LANGUAGE.to_string(),
            always_on_top: true,
            muted: false,
            ticker_mode: false,
            window_pos: None,
        }
    }
}

pub struct SettingsManager {
    settings_file: PathBuf,
    settings: Settings,
}

impl SettingsManager {
    pub fn new(settings_file: impl Into<PathBuf>) -> Self {
        let settings_file = settings_file.into();
        let settings = Self::load_from(&settings_file);
        Self {
            settings_file,
            settings,
        }
    }

    /// Load settings from JSON file, or return defaults.
    fn load_from(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Settings>(&content) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error loading settings: {e}");
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    pub fn get(&self) -> &Settings {
        &self.settings
    }

    pub fn update(&mut self, new_settings: Settings) {
        self.settings = new_settings;
        self.save();
    }

    /// Persist current settings to JSON (pretty-printed, UTF-8).
    pub fn save(&self) {
        if let Ok(text) = serde_json::to_string_pretty(&self.settings) {
            if let Err(e) = std::fs::write(&self.settings_file, text) {
                eprintln!("Error saving settings: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bittalker_test_{name}_{}.json", std::process::id()))
    }

    #[test]
    fn default_settings() {
        let m = SettingsManager::new(temp_file("defaults"));
        assert_eq!(m.get().interval, 50);
        assert_eq!(m.get().voice, "F1");
        assert_eq!(m.get().language, "Korean");
        assert!(m.get().always_on_top);
    }

    #[test]
    fn set_and_get_persistence() {
        let path = temp_file("persist");
        let mut m = SettingsManager::new(path.clone());
        let mut s = m.get().clone();
        s.interval = 100;
        m.update(s);

        let m2 = SettingsManager::new(path.clone());
        assert_eq!(m2.get().interval, 100);

        let mut m3 = SettingsManager::new(path.clone());
        let mut s3 = m3.get().clone();
        s3.interval = 200;
        s3.voice = "M1".to_string();
        m3.update(s3);
        assert_eq!(m3.get().interval, 200);
        assert_eq!(m3.get().voice, "M1");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_keys_get_defaults() {
        let path = temp_file("missing");
        std::fs::write(&path, r#"{"interval": 123}"#).unwrap();
        let m = SettingsManager::new(path.clone());
        assert_eq!(m.get().interval, 123);
        assert_eq!(m.get().voice, "F1");
        assert!(!m.get().muted);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn lang_code_map() {
        assert_eq!(lang_code("Korean"), "ko");
        assert_eq!(lang_code("French"), "fr");
        assert_eq!(lang_code("Unknown"), "ko");
    }

    #[test]
    fn window_pos_multi_monitor_serialization() {
        let path = temp_file("winpos");
        let mut m = SettingsManager::new(path.clone());
        assert_eq!(m.get().window_pos, None);

        // Secondary monitor to the left can have negative x coordinates
        let mut s = m.get().clone();
        s.window_pos = Some([-1920.0, 150.0]);
        m.update(s);

        let m2 = SettingsManager::new(path.clone());
        assert_eq!(m2.get().window_pos, Some([-1920.0, 150.0]));
        let _ = std::fs::remove_file(path);
    }
}
