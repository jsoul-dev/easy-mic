//! Application configuration persistence and data structures.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum IndicatorState {
    #[default]
    Hidden = 0,
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
    #[serde(default = "default_false")]
    pub is_updates_enabled: bool,
    #[serde(default = "default_false")]
    pub is_auto_update_enabled: bool,
    #[serde(default = "default_true")]
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
    50
}
fn default_mic_volume() -> i8 {
    100
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
fn default_false() -> bool {
    false
}
fn default_multi_press_window() -> u16 {
    200
}
fn default_version() -> i32 {
    2
}
fn default_mute_sound() -> String {
    "D:\\Backups\\[ This PC Backup ] July 11, 2026\\Music\\Mic Cue\\off.wav".to_string()
}
fn default_unmute_sound() -> String {
    "D:\\Backups\\[ This PC Backup ] July 11, 2026\\Music\\Mic Cue\\on.wav".to_string()
}

// Shift modifier flag: 0x08. Virtual key 'Z': 0x5A. Hotkey mask: (0x5A << 8) | 0x08
pub const SHIFT_Z_HOTKEY_MASK: u64 = (0x5Au64 << 8) | 0x08;

impl Default for AppConfig {
    fn default() -> Self {
        // Default action: Shift + Z for Toggle Mute matching user's exact configuration
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
            show_notification: false,
        }];

        let mut recent_sounds = BTreeSet::new();
        recent_sounds.insert(
            "D:\\Backups\\[ This PC Backup ] July 11, 2026\\Music\\Mic Cue\\off.wav".to_string(),
        );
        recent_sounds.insert(
            "D:\\Backups\\[ This PC Backup ] July 11, 2026\\Music\\Mic Cue\\on.wav".to_string(),
        );
        recent_sounds.insert("off.wav".to_string());
        recent_sounds.insert("on.wav".to_string());

        Self {
            window_pos_x: 0,
            window_pos_y: 0,
            bell_volume: 50,
            mic_volume: 100,
            indicator_size: 16,
            indicator_state: IndicatorState::Hidden,
            indicator_volume_threshold: 0.001,
            exclude_from_capture: false,
            on_top_exclusive: false,
            is_mic_keep_volume: true,
            is_updates_enabled: false,
            is_auto_update_enabled: false,
            is_skip_uac_enabled: true,
            hide_when_inactive: true,
            notifications_enabled: true,
            multi_press_window_ms: 200,
            version: 2,
            skipped_versions: BTreeSet::new(),
            actions: default_actions,
            recent_sounds,
            mute_sound_source:
                "D:\\Backups\\[ This PC Backup ] July 11, 2026\\Music\\Mic Cue\\off.wav".to_string(),
            unmute_sound_source:
                "D:\\Backups\\[ This PC Backup ] July 11, 2026\\Music\\Mic Cue\\on.wav".to_string(),
        }
    }
}

impl AppConfig {
    /// Parses Glaze BEVE binary configuration (conf.b format from C++ EasyMic).
    pub fn parse_beve(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 16 {
            return None;
        }
        let mut cfg = Self::default();

        let find_key = |key: &str| -> Option<usize> {
            let key_bytes = key.as_bytes();
            bytes
                .windows(key_bytes.len())
                .position(|window| window == key_bytes)
        };

        if let Some(idx) = find_key("WindowPosX") {
            let v_idx = idx + 10;
            if v_idx + 4 < bytes.len() && bytes[v_idx] == 0x49 {
                cfg.window_pos_x =
                    i32::from_le_bytes(bytes[v_idx + 1..v_idx + 5].try_into().unwrap_or_default());
            }
        }
        if let Some(idx) = find_key("WindowPosY") {
            let v_idx = idx + 10;
            if v_idx + 4 < bytes.len() && bytes[v_idx] == 0x49 {
                cfg.window_pos_y =
                    i32::from_le_bytes(bytes[v_idx + 1..v_idx + 5].try_into().unwrap_or_default());
            }
        }
        if let Some(idx) = find_key("BellVolume") {
            let v_idx = idx + 10;
            if v_idx + 1 < bytes.len() && bytes[v_idx] == 0x09 {
                cfg.bell_volume = bytes[v_idx + 1] as i8;
            }
        }
        if let Some(idx) = find_key("MicVolume") {
            let v_idx = idx + 9;
            if v_idx + 1 < bytes.len() && bytes[v_idx] == 0x09 {
                cfg.mic_volume = bytes[v_idx + 1] as i8;
            }
        }
        if let Some(idx) = find_key("IndicatorSize") {
            let v_idx = idx + 13;
            if v_idx + 1 < bytes.len() && bytes[v_idx] == 0x11 {
                cfg.indicator_size = bytes[v_idx + 1];
            }
        }
        if let Some(idx) = find_key("IndicatorState") {
            let v_idx = idx + 14;
            if v_idx + 4 < bytes.len() && bytes[v_idx] == 0x49 {
                let st =
                    i32::from_le_bytes(bytes[v_idx + 1..v_idx + 5].try_into().unwrap_or_default());
                cfg.indicator_state = match st {
                    0 => IndicatorState::Hidden,
                    1 => IndicatorState::Muted,
                    2 => IndicatorState::MutedOrTalk,
                    _ => IndicatorState::Hidden,
                };
            }
        }
        if let Some(idx) = find_key("IndicatorVolumeThreshold") {
            let v_idx = idx + 24;
            if v_idx + 4 < bytes.len() && bytes[v_idx] == 0x41 {
                cfg.indicator_volume_threshold =
                    f32::from_le_bytes(bytes[v_idx + 1..v_idx + 5].try_into().unwrap_or_default());
            }
        }
        if let Some(idx) = find_key("ExcludeFromCapture") {
            let v_idx = idx + 18;
            if v_idx < bytes.len() {
                cfg.exclude_from_capture = bytes[v_idx] == 0x18;
            }
        }
        if let Some(idx) = find_key("OnTopExclusive") {
            let v_idx = idx + 14;
            if v_idx < bytes.len() {
                cfg.on_top_exclusive = bytes[v_idx] == 0x18;
            }
        }
        if let Some(idx) = find_key("IsMicKeepVolume") {
            let v_idx = idx + 15;
            if v_idx < bytes.len() {
                cfg.is_mic_keep_volume = bytes[v_idx] == 0x18;
            }
        }
        if let Some(idx) = find_key("IsUpdatesEnabled") {
            let v_idx = idx + 16;
            if v_idx < bytes.len() {
                cfg.is_updates_enabled = bytes[v_idx] == 0x18;
            }
        }
        if let Some(idx) = find_key("IsAutoUpdateEnabled") {
            let v_idx = idx + 19;
            if v_idx < bytes.len() {
                cfg.is_auto_update_enabled = bytes[v_idx] == 0x18;
            }
        }
        if let Some(idx) = find_key("IsSkipUACEnabled") {
            let v_idx = idx + 16;
            if v_idx < bytes.len() {
                cfg.is_skip_uac_enabled = bytes[v_idx] == 0x18;
            }
        }
        if let Some(idx) = find_key("HideWhenInactive") {
            let v_idx = idx + 16;
            if v_idx < bytes.len() {
                cfg.hide_when_inactive = bytes[v_idx] == 0x18;
            }
        }
        if let Some(idx) = find_key("Toggle mute") {
            let v_idx = idx + 11;
            if v_idx + 8 < bytes.len() && bytes[v_idx] == 0x71 {
                let mask =
                    u64::from_le_bytes(bytes[v_idx + 1..v_idx + 9].try_into().unwrap_or_default());
                if mask != 0 {
                    cfg.actions = vec![ActionConfig {
                        name: "Toggle mute".to_string(),
                        built_in: "Toggle mute".to_string(),
                        command: String::new(),
                        args: String::new(),
                        sound: String::new(),
                        notification: "Mic {mic}".to_string(),
                        hotkey: mask,
                        on_release: false,
                        presses: 1,
                        block: false,
                        tap_only: false,
                        sound_volume: 100,
                        show_notification: false,
                    }];
                }
            }
        }

        let read_beve_str = |idx: usize, key_len: usize| -> Option<String> {
            let v_idx = idx + key_len;
            if v_idx + 2 >= bytes.len() {
                return None;
            }
            if bytes[v_idx] == 0x02 {
                let raw_len = u16::from_le_bytes([bytes[v_idx + 1], bytes[v_idx + 2]]) as usize;
                let s_len = raw_len >> 2;
                let s_start = v_idx + 3;
                if s_start + s_len <= bytes.len() {
                    return String::from_utf8(bytes[s_start..s_start + s_len].to_vec()).ok();
                }
            } else if bytes[v_idx] & 0x03 == 0 {
                let s_len = (bytes[v_idx] >> 2) as usize;
                let s_start = v_idx + 1;
                if s_start + s_len <= bytes.len() {
                    return String::from_utf8(bytes[s_start..s_start + s_len].to_vec()).ok();
                }
            }
            None
        };

        if let Some(idx) = find_key("MuteSoundSource") {
            if let Some(s) = read_beve_str(idx, 15) {
                if !s.is_empty() {
                    cfg.recent_sounds.insert(s.clone());
                    cfg.mute_sound_source = s;
                }
            }
        }
        if let Some(idx) = find_key("UnmuteSoundSource") {
            if let Some(s) = read_beve_str(idx, 17) {
                if !s.is_empty() {
                    cfg.recent_sounds.insert(s.clone());
                    cfg.unmute_sound_source = s;
                }
            }
        }

        Some(cfg)
    }

    /// Resolves default configuration file path relative to current executable, working dir, or system install.
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
            }
        }
        let local_json = PathBuf::from("conf.json");
        if local_json.exists() {
            return local_json;
        }
        let local_b = PathBuf::from("conf.b");
        if local_b.exists() {
            return local_b;
        }
        let system_b = PathBuf::from(r"C:\Program Files\Easymic\conf.b");
        if system_b.exists() {
            return system_b;
        }
        let system_json = PathBuf::from(r"C:\Program Files\Easymic\conf.json");
        if system_json.exists() {
            return system_json;
        }

        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                return parent.join("conf.json");
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

        match fs::read(path) {
            Ok(bytes) => {
                // Try JSON first if it looks like UTF-8 JSON
                if bytes.starts_with(b"{")
                    || path.extension().and_then(|e| e.to_str()) == Some("json")
                {
                    if let Ok(mut cfg) = serde_json::from_slice::<AppConfig>(&bytes) {
                        log::info!("Loaded configuration JSON from {:?}", path);
                        if cfg.actions.is_empty() {
                            cfg.actions = Self::default().actions;
                        }
                        return cfg;
                    }
                }
                // Try BEVE conf.b binary format
                if let Some(cfg) = Self::parse_beve(&bytes) {
                    log::info!("Loaded configuration BEVE (conf.b) from {:?}", path);
                    return cfg;
                }
                // Fallback attempt JSON deserialization anyway
                if let Ok(mut cfg) = serde_json::from_slice::<AppConfig>(&bytes) {
                    log::info!("Loaded configuration from {:?}", path);
                    if cfg.actions.is_empty() {
                        cfg.actions = Self::default().actions;
                    }
                    return cfg;
                }
                log::warn!("Failed to parse config from {:?}, using defaults", path);
                Self::default()
            }
            Err(err) => {
                log::warn!(
                    "Failed to read config file {:?}: {}, using defaults",
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
        assert_eq!(cfg.bell_volume, 50);
        assert_eq!(cfg.mic_volume, 100);
        assert_eq!(cfg.indicator_size, 16);
        assert_eq!(cfg.indicator_state, IndicatorState::Hidden);
        assert!(cfg.is_mic_keep_volume);
        assert!(cfg.is_skip_uac_enabled);
        assert!(!cfg.is_updates_enabled);
        assert!(cfg.hide_when_inactive);
        assert!(!cfg.actions.is_empty());
        assert_eq!(cfg.actions[0].name, "Toggle mute");
        assert_eq!(cfg.actions[0].hotkey, SHIFT_Z_HOTKEY_MASK);
        assert!(cfg.mute_sound_source.ends_with("off.wav"));
        assert!(cfg.unmute_sound_source.ends_with("on.wav"));
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

    #[test]
    fn test_parse_conf_b_file() {
        let path = Path::new(r"C:\Program Files\Easymic\conf.b");
        if path.exists() {
            let bytes = fs::read(path).expect("read conf.b");
            let cfg = AppConfig::parse_beve(&bytes).expect("parse beve");
            assert_eq!(cfg.bell_volume, 50);
            assert_eq!(cfg.mic_volume, 100);
            assert_eq!(cfg.indicator_size, 16);
            assert_eq!(cfg.indicator_state, IndicatorState::Hidden);
            assert!(cfg.is_mic_keep_volume);
            assert!(cfg.is_skip_uac_enabled);
            assert!(!cfg.is_updates_enabled);
            assert!(cfg.hide_when_inactive);
            assert_eq!(cfg.actions[0].hotkey, SHIFT_Z_HOTKEY_MASK);
            assert!(cfg.mute_sound_source.ends_with("off.wav"));
            assert!(cfg.unmute_sound_source.ends_with("on.wav"));
        }
    }
}
