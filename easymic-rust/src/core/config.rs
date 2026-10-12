//! Application configuration persistence and data structures.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum IndicatorState {
    Hidden = 0,
    #[default]
    Muted = 1,
    MutedOrTalk = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ActionConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub built_in: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub sound: String,
    #[serde(default)]
    pub notification: String,
    #[serde(default)]
    pub hotkey: u64,
    #[serde(default)]
    pub on_release: bool,
    #[serde(default = "default_presses")]
    pub presses: u8,
    #[serde(default)]
    pub block: bool,
    #[serde(default)]
    pub tap_only: bool,
    #[serde(default = "default_sound_volume")]
    pub sound_volume: u8,
    #[serde(default)]
    pub show_notification: bool,
}

fn default_presses() -> u8 {
    1
}

fn default_sound_volume() -> u8 {
    100
}

impl Default for ActionConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            built_in: String::new(),
            command: String::new(),
            args: String::new(),
            sound: String::new(),
            notification: String::new(),
            hotkey: 0,
            on_release: false,
            presses: 1,
            block: false,
            tap_only: false,
            sound_volume: 100,
            show_notification: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppConfig {
    #[serde(default)]
    pub window_pos_x: i32,
    #[serde(default)]
    pub window_pos_y: i32,
    #[serde(default = "default_bell_volume")]
    pub bell_volume: i8,
    #[serde(default = "default_mic_volume")]
    pub mic_volume: i8,
    #[serde(default = "default_indicator_size")]
    pub indicator_size: u8,
    #[serde(default)]
    pub indicator_state: IndicatorState,
    #[serde(default = "default_volume_threshold")]
    pub indicator_volume_threshold: f32,
    #[serde(default)]
    pub exclude_from_capture: bool,
    #[serde(default)]
    pub on_top_exclusive: bool,
    #[serde(default = "default_true")]
    pub is_mic_keep_volume: bool,
    #[serde(default = "default_true")]
    pub is_updates_enabled: bool,
    #[serde(default)]
    pub is_auto_update_enabled: bool,
    #[serde(default)]
    pub is_skip_uac_enabled: bool,
    #[serde(default = "default_true")]
    pub hide_when_inactive: bool,
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    #[serde(default = "default_multi_press_window")]
    pub multi_press_window_ms: u16,
    #[serde(default = "default_version")]
    pub version: i32,
    #[serde(default)]
    pub skipped_versions: BTreeSet<String>,
    #[serde(default)]
    pub actions: Vec<ActionConfig>,
    #[serde(default)]
    pub recent_sounds: BTreeSet<String>,
    #[serde(default = "default_mute_sound")]
    pub mute_sound_source: String,
    #[serde(default = "default_unmute_sound")]
    pub unmute_sound_source: String,
}

fn default_bell_volume() -> i8 {
    25
}
fn default_mic_volume() -> i8 {
    -1
}
fn default_indicator_size() -> u8 {
    16
}
fn default_volume_threshold() -> f32 {
    0.001
}
fn default_true() -> bool {
    true
}
fn default_multi_press_window() -> u16 {
    200
}
fn default_version() -> i32 {
    2
}
fn default_mute_sound() -> String {
    "Mute".to_string()
}
fn default_unmute_sound() -> String {
    "Unmute".to_string()
}

// Shift modifier flag: 0x08. Virtual key 'Z': 0x5A. Hotkey mask: (0x5A << 8) | 0x08
pub const SHIFT_Z_HOTKEY_MASK: u64 = (0x5Au64 << 8) | 0x08;

impl Default for AppConfig {
    fn default() -> Self {
        // Default action: Shift + Z for Toggle Mute
        let default_actions = vec![ActionConfig {
            name: "Toggle mute".to_string(),
            built_in: "Toggle mute".to_string(),
            command: String::new(),
            args: String::new(),
            sound: String::new(),
            notification: "Mic {mic}".to_string(),
            hotkey: SHIFT_Z_HOTKEY_MASK,
            on_release: false,
            presses: 1,
            block: false,
            tap_only: false,
            sound_volume: 100,
            show_notification: true,
        }];

        Self {
            window_pos_x: 0,
            window_pos_y: 0,
            bell_volume: 25,
            mic_volume: -1,
            indicator_size: 16,
            indicator_state: IndicatorState::Muted,
            indicator_volume_threshold: 0.001,
            exclude_from_capture: false,
            on_top_exclusive: false,
            is_mic_keep_volume: true,
            is_updates_enabled: true,
            is_auto_update_enabled: false,
            is_skip_uac_enabled: false,
            hide_when_inactive: true,
            notifications_enabled: true,
            multi_press_window_ms: 200,
            version: 2,
            skipped_versions: BTreeSet::new(),
            actions: default_actions,
            recent_sounds: BTreeSet::new(),
            mute_sound_source: "Mute".to_string(),
            unmute_sound_source: "Unmute".to_string(),
        }
    }
}

impl AppConfig {
    /// Resolves default configuration file path relative to current executable or working dir.
    pub fn get_config_path() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let candidate_json = parent.join("conf.json");
                if candidate_json.exists() {
                    return candidate_json;
                }
                let candidate_b = parent.join("conf.b");
                if candidate_b.exists() {
                    return candidate_b;
                }
                return candidate_json;
            }
        }
        PathBuf::from("conf.json")
    }

    /// Loads configuration from disk, falling back to default.
    pub fn load() -> Self {
        let path = Self::get_config_path();
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &Path) -> Self {
        if !path.exists() {
            log::info!("Config file not found at {:?}, using defaults", path);
            return Self::default();
        }

        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                Ok(mut cfg) => {
                    log::info!("Loaded configuration from {:?}", path);
                    // Ensure at least default Toggle mute exists if actions list was empty
                    if cfg.actions.is_empty() {
                        cfg.actions = Self::default().actions;
                    }
                    cfg
                }
                Err(err) => {
                    log::warn!(
                        "Failed to parse config json {:?}: {}, falling back to defaults",
                        path,
                        err
                    );
                    Self::default()
                }
            },
            Err(err) => {
                log::warn!(
                    "Failed to read config file {:?}: {}, falling back to defaults",
                    path,
                    err
                );
                Self::default()
            }
        }
    }

    /// Saves configuration to disk.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::get_config_path();
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, json)?;
        log::info!("Saved configuration to {:?}", path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.bell_volume, 25);
        assert_eq!(cfg.indicator_state, IndicatorState::Muted);
        assert!(!cfg.actions.is_empty());
        assert_eq!(cfg.actions[0].name, "Toggle mute");
        assert_eq!(cfg.actions[0].hotkey, SHIFT_Z_HOTKEY_MASK);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let mut cfg = AppConfig::default();
        cfg.bell_volume = 40;
        cfg.window_pos_x = 100;
        cfg.window_pos_y = 200;

        let json = serde_json::to_string(&cfg).expect("serialize");
        let deserialized: AppConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.bell_volume, 40);
        assert_eq!(deserialized.window_pos_x, 100);
        assert_eq!(deserialized.window_pos_y, 200);
        assert_eq!(deserialized.actions, cfg.actions);
    }
}
