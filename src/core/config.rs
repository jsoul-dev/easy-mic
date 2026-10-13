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
    #[serde(default)]
    pub start_with_windows: bool,
}

fn default_bell_volume() -> i8 {
    50
}
fn default_mic_volume() -> i8 {
    100
}
fn default_indicator_size() -> u8 {
    15
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
    "off.wav".to_string()
}
fn default_unmute_sound() -> String {
    "on.wav".to_string()
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
            show_notification: false,
        }];

        let recent_sounds = BTreeSet::new();

        Self {
            window_pos_x: 0,
            window_pos_y: 0,
            bell_volume: 50,
            mic_volume: 100,
            indicator_size: 15,
            indicator_state: IndicatorState::Hidden,
            indicator_volume_threshold: 0.001,
            exclude_from_capture: false,
            on_top_exclusive: false,
            is_mic_keep_volume: true,
            is_updates_enabled: false,
            is_auto_update_enabled: false,
            is_skip_uac_enabled: false,
            hide_when_inactive: true,
            notifications_enabled: true,
            multi_press_window_ms: 200,
            version: 2,
            skipped_versions: BTreeSet::new(),
            actions: default_actions,
            recent_sounds,
            mute_sound_source: "Off".to_string(),
            unmute_sound_source: "On".to_string(),
            start_with_windows: false,
        }
    }
}

impl AppConfig {
    /// Normalizes sound cue names, mapping built-in sound filenames and legacy paths to canonical preset names.
    pub fn normalize_sound_name(name: &str) -> String {
        let trimmed = name.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower == "off"
            || lower == "off.wav"
            || lower.ends_with("\\off.wav")
            || lower.ends_with("/off.wav")
        {
            return "Off".to_string();
        }
        if lower == "on"
            || lower == "on.wav"
            || lower.ends_with("\\on.wav")
            || lower.ends_with("/on.wav")
        {
            return "On".to_string();
        }
        trimmed.to_string()
    }

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
                    cfg.mute_sound_source = Self::normalize_sound_name(&s);
                }
            }
        }
        if let Some(idx) = find_key("UnmuteSoundSource") {
            if let Some(s) = read_beve_str(idx, 17) {
                if !s.is_empty() {
                    cfg.unmute_sound_source = Self::normalize_sound_name(&s);
                }
            }
        }

        Some(cfg)
    }

    /// Serializes configuration to a clean, commented INI format (config.ini).
    pub fn to_ini(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        writeln!(
            out,
            "; =============================================================================="
        )
        .unwrap();
        writeln!(out, "; Easy-Mic Configuration File (config.ini)").unwrap();
        writeln!(
            out,
            "; =============================================================================="
        )
        .unwrap();
        writeln!(out).unwrap();

        writeln!(out, "[General]").unwrap();
        writeln!(out, "; Application configuration format version").unwrap();
        writeln!(out, "Version = {}", self.version).unwrap();
        writeln!(out, "; Launch Easy-Mic automatically on Windows startup").unwrap();
        writeln!(out, "StartWithWindows = {}", self.start_with_windows).unwrap();
        writeln!(
            out,
            "; Run with elevated privileges silently via Task Scheduler"
        )
        .unwrap();
        writeln!(out, "SkipUAC = {}", self.is_skip_uac_enabled).unwrap();
        writeln!(out, "; Check for application updates on startup").unwrap();
        writeln!(out, "CheckForUpdates = {}", self.is_updates_enabled).unwrap();
        writeln!(out, "; Automatically download and install updates").unwrap();
        writeln!(out, "AutoUpdates = {}", self.is_auto_update_enabled).unwrap();
        writeln!(out, "; Display notification toasts for microphone actions").unwrap();
        writeln!(out, "NotificationsEnabled = {}", self.notifications_enabled).unwrap();
        writeln!(
            out,
            "; Multi-press window in milliseconds (for double/triple press hotkeys)"
        )
        .unwrap();
        writeln!(out, "MultiPressWindowMs = {}", self.multi_press_window_ms).unwrap();
        if !self.skipped_versions.is_empty() {
            let versions: Vec<&str> = self.skipped_versions.iter().map(|s| s.as_str()).collect();
            writeln!(out, "SkippedVersions = {}", versions.join("; ")).unwrap();
        }
        writeln!(out).unwrap();

        writeln!(out, "[Indicator]").unwrap();
        writeln!(
            out,
            "; Display state: Hidden (0), Muted (1), MutedOrTalk (2)"
        )
        .unwrap();
        let state_str = match self.indicator_state {
            IndicatorState::Hidden => "Hidden",
            IndicatorState::Muted => "Muted",
            IndicatorState::MutedOrTalk => "MutedOrTalk",
        };
        writeln!(out, "State = {}", state_str).unwrap();
        writeln!(out, "; Indicator icon size in pixels").unwrap();
        writeln!(out, "Size = {}", self.indicator_size).unwrap();
        writeln!(out, "; Voice activity volume threshold (0.0 to 1.0)").unwrap();
        writeln!(
            out,
            "VolumeThreshold = {:.4}",
            self.indicator_volume_threshold
        )
        .unwrap();
        writeln!(
            out,
            "; Hide indicator when active window is fullscreen or inactive"
        )
        .unwrap();
        writeln!(out, "HideWhenInactive = {}", self.hide_when_inactive).unwrap();
        writeln!(out, "; Exclude overlay window from screen recording tools").unwrap();
        writeln!(out, "ExcludeFromCapture = {}", self.exclude_from_capture).unwrap();
        writeln!(
            out,
            "; Maintain top-most Z-order over exclusive fullscreen windows"
        )
        .unwrap();
        writeln!(out, "OnTopExclusive = {}", self.on_top_exclusive).unwrap();
        writeln!(out, "; Saved position on screen").unwrap();
        writeln!(out, "PositionX = {}", self.window_pos_x).unwrap();
        writeln!(out, "PositionY = {}", self.window_pos_y).unwrap();
        writeln!(out).unwrap();

        writeln!(out, "[Sounds]").unwrap();
        writeln!(
            out,
            "; Hardware microphone volume (-1 to disable volume enforcement, 0-100)"
        )
        .unwrap();
        writeln!(out, "MicVolume = {}", self.mic_volume).unwrap();
        writeln!(out, "; Continuously enforce configured microphone volume").unwrap();
        writeln!(out, "KeepMicVolume = {}", self.is_mic_keep_volume).unwrap();
        writeln!(out, "; Feedback sound volume (0-100 percent)").unwrap();
        writeln!(out, "BellVolume = {}", self.bell_volume).unwrap();
        writeln!(out, "; Sound cue played when muting the microphone").unwrap();
        writeln!(out, "MuteSound = {}", Self::normalize_sound_name(&self.mute_sound_source)).unwrap();
        writeln!(out, "; Sound cue played when unmuting the microphone").unwrap();
        writeln!(out, "UnmuteSound = {}", Self::normalize_sound_name(&self.unmute_sound_source)).unwrap();
        writeln!(out).unwrap();

        writeln!(
            out,
            "; =============================================================================="
        )
        .unwrap();
        writeln!(out, "; Actions Configuration").unwrap();
        writeln!(
            out,
            "; =============================================================================="
        )
        .unwrap();
        writeln!(out).unwrap();

        for (idx, action) in self.actions.iter().enumerate() {
            if idx > 0 {
                writeln!(out).unwrap();
            }
            writeln!(out, "[Action.{}]", idx + 1).unwrap();
            writeln!(out, "Name = {}", action.name).unwrap();
            writeln!(out, "Type = {}", action.built_in).unwrap();
            let hotkey_str = crate::input::keys::get_hotkey_name(action.hotkey);
            writeln!(out, "Hotkey = {}", hotkey_str).unwrap();
            writeln!(out, "Presses = {}", action.presses).unwrap();
            writeln!(out, "OnRelease = {}", action.on_release).unwrap();
            writeln!(out, "Block = {}", action.block).unwrap();
            writeln!(out, "TapOnly = {}", action.tap_only).unwrap();
            writeln!(out, "Sound = {}", action.sound).unwrap();
            writeln!(out, "SoundVolume = {}", action.sound_volume).unwrap();
            writeln!(out, "Notification = {}", action.notification).unwrap();
            writeln!(out, "ShowNotification = {}", action.show_notification).unwrap();
            writeln!(out, "Command = {}", action.command).unwrap();
            writeln!(out, "Args = {}", action.args).unwrap();
        }

        out
    }

    /// Parses configuration from INI text.
    pub fn parse_ini(content: &str) -> Option<Self> {
        let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
        let mut cfg = Self::default();
        let mut parsed_any = false;

        let mut current_section = String::new();
        let mut current_action: Option<ActionConfig> = None;
        let mut custom_actions: Vec<ActionConfig> = Vec::new();

        let finalize_action = |act: &mut Option<ActionConfig>, actions: &mut Vec<ActionConfig>| {
            if let Some(mut a) = act.take() {
                if !a.name.is_empty() || !a.built_in.is_empty() || a.hotkey != 0 {
                    if a.name.is_empty() {
                        a.name = if !a.built_in.is_empty() {
                            a.built_in.clone()
                        } else {
                            "Action".to_string()
                        };
                    }
                    if a.built_in.is_empty() && a.command.is_empty() {
                        a.built_in = "Toggle mute".to_string();
                    }
                    actions.push(a);
                }
            }
        };

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                let section_name = line[1..line.len() - 1].trim();
                let lower_sec = section_name.to_ascii_lowercase();

                if lower_sec.starts_with("action") {
                    finalize_action(&mut current_action, &mut custom_actions);
                    current_action = Some(ActionConfig::default());
                    parsed_any = true;
                } else {
                    finalize_action(&mut current_action, &mut custom_actions);
                    if lower_sec == "general"
                        || lower_sec == "indicator"
                        || lower_sec == "sounds"
                        || lower_sec == "audio"
                        || lower_sec == "sound"
                        || lower_sec == "hotkeys"
                    {
                        parsed_any = true;
                    }
                }

                current_section = section_name.to_string();
                continue;
            }

            // Parse key-value: Key = Value or Key: Value
            let (key, val) = if let Some((k, v)) = line.split_once('=') {
                (k.trim(), v.trim())
            } else if let Some((k, v)) = line.split_once(':') {
                (k.trim(), v.trim())
            } else {
                continue;
            };

            // Strip trailing inline comment if separated by whitespace and semicolon/hash
            let val = {
                let mut v_clean = val;
                if (v_clean.starts_with('"') && v_clean.ends_with('"') && v_clean.len() >= 2)
                    || (v_clean.starts_with('\'') && v_clean.ends_with('\'') && v_clean.len() >= 2)
                {
                    v_clean = &v_clean[1..v_clean.len() - 1];
                } else {
                    for (idx, _) in v_clean.match_indices([';', '#']) {
                        if idx > 0 && v_clean.as_bytes()[idx - 1].is_ascii_whitespace() {
                            v_clean = v_clean[..idx].trim_end();
                            break;
                        }
                    }
                }
                v_clean
            };

            let sec_lower = current_section.to_ascii_lowercase();
            let key_lower = key.to_ascii_lowercase();

            if sec_lower.starts_with("action") {
                if let Some(ref mut act) = current_action {
                    parsed_any = true;
                    match key_lower.as_str() {
                        "name" => act.name = val.to_string(),
                        "type" | "builtin" | "built_in" => act.built_in = val.to_string(),
                        "command" => act.command = val.to_string(),
                        "args" => act.args = val.to_string(),
                        "sound" => act.sound = val.to_string(),
                        "notification" => act.notification = val.to_string(),
                        "hotkey" | "hotkeymask" => {
                            act.hotkey = crate::input::keys::parse_hotkey(val);
                        }
                        "presses" => {
                            if let Ok(n) = val.parse::<u8>() {
                                act.presses = n;
                            }
                        }
                        "onrelease" | "on_release" => {
                            if let Some(b) = parse_bool(val) {
                                act.on_release = b;
                            }
                        }
                        "block" => {
                            if let Some(b) = parse_bool(val) {
                                act.block = b;
                            }
                        }
                        "taponly" | "tap_only" => {
                            if let Some(b) = parse_bool(val) {
                                act.tap_only = b;
                            }
                        }
                        "soundvolume" | "sound_volume" => {
                            if let Ok(v) = val.parse::<u8>() {
                                act.sound_volume = v;
                            }
                        }
                        "shownotification" | "show_notification" => {
                            if let Some(b) = parse_bool(val) {
                                act.show_notification = b;
                            }
                        }
                        _ => {}
                    }
                }
            } else if sec_lower == "general" {
                match key_lower.as_str() {
                    "version" => {
                        if let Ok(v) = val.parse::<i32>() {
                            cfg.version = v;
                        }
                    }
                    "startwithwindows" | "autostart" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.start_with_windows = b;
                        }
                    }
                    "skipuac" | "isskipuacenabled" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.is_skip_uac_enabled = b;
                        }
                    }
                    "checkforupdates" | "isupdatesenabled" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.is_updates_enabled = b;
                        }
                    }
                    "autoupdates" | "isautoupdateenabled" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.is_auto_update_enabled = b;
                        }
                    }
                    "notificationsenabled" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.notifications_enabled = b;
                        }
                    }
                    "multipresswindowms" => {
                        if let Ok(ms) = val.parse::<u16>() {
                            cfg.multi_press_window_ms = ms;
                        }
                    }
                    "skippedversions" => {
                        for v in val.split([',', ';']) {
                            let item = v.trim();
                            if !item.is_empty() {
                                cfg.skipped_versions.insert(item.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            } else if sec_lower == "indicator" {
                match key_lower.as_str() {
                    "state" | "indicatorstate" => {
                        cfg.indicator_state = parse_indicator_state(val);
                    }
                    "size" | "indicatorsize" => {
                        if let Ok(s) = val.parse::<u8>() {
                            cfg.indicator_size = s;
                        }
                    }
                    "volumethreshold" | "indicatorvolumethreshold" => {
                        if let Ok(th) = val.parse::<f32>() {
                            cfg.indicator_volume_threshold = th;
                        }
                    }
                    "hidewheninactive" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.hide_when_inactive = b;
                        }
                    }
                    "excludefromcapture" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.exclude_from_capture = b;
                        }
                    }
                    "ontopexclusive" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.on_top_exclusive = b;
                        }
                    }
                    "positionx" | "windowposx" => {
                        if let Ok(x) = val.parse::<i32>() {
                            cfg.window_pos_x = x;
                        }
                    }
                    "positiony" | "windowposy" => {
                        if let Ok(y) = val.parse::<i32>() {
                            cfg.window_pos_y = y;
                        }
                    }
                    _ => {}
                }
            } else if sec_lower == "sounds" || sec_lower == "audio" || sec_lower == "sound" {
                match key_lower.as_str() {
                    "micvolume" => {
                        if let Ok(vol) = val.parse::<i8>() {
                            cfg.mic_volume = vol;
                        }
                    }
                    "keepmicvolume" | "ismickeepvolume" => {
                        if let Some(b) = parse_bool(val) {
                            cfg.is_mic_keep_volume = b;
                        }
                    }
                    "bellvolume" => {
                        if let Ok(vol) = val.parse::<i8>() {
                            cfg.bell_volume = vol;
                        }
                    }
                    "mutesound" | "mutesoundsource" => {
                        if !val.is_empty() {
                            cfg.mute_sound_source = Self::normalize_sound_name(val);
                        }
                    }
                    "unmutesound" | "unmutesoundsource" => {
                        if !val.is_empty() {
                            cfg.unmute_sound_source = Self::normalize_sound_name(val);
                        }
                    }
                    "recentsounds" => {}
                    _ => {}
                }
            } else if sec_lower == "hotkeys"
                && (key_lower == "togglemute"
                    || key_lower == "toggle_mute"
                    || key_lower == "toggle mute")
            {
                let mask = crate::input::keys::parse_hotkey(val);
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

        finalize_action(&mut current_action, &mut custom_actions);

        if !custom_actions.is_empty() {
            cfg.actions = custom_actions;
        }

        if parsed_any {
            Some(cfg)
        } else {
            None
        }
    }

    /// Returns the directory containing the running executable.
    pub fn get_exe_dir() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                return parent.to_path_buf();
            }
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    /// Returns the user APPDATA configuration directory for Easy-Mic: %APPDATA%\Easy-Mic
    pub fn get_appdata_dir() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|appdata| PathBuf::from(appdata).join("Easy-Mic"))
    }

    /// Returns the %APPDATA%\Easy-Mic\config.ini path if APPDATA is available.
    pub fn get_appdata_config_path() -> Option<PathBuf> {
        Self::get_appdata_dir().map(|d| d.join("config.ini"))
    }

    /// Tests if a file or directory path is writable by the current process.
    pub fn is_path_writable(path: &Path) -> bool {
        if path.exists() {
            if path.is_dir() {
                let test_file = path.join(format!(".write_test_{}", std::process::id()));
                match fs::write(&test_file, b"") {
                    Ok(_) => {
                        let _ = fs::remove_file(&test_file);
                        true
                    }
                    Err(_) => false,
                }
            } else {
                fs::OpenOptions::new().write(true).open(path).is_ok()
            }
        } else if let Some(parent) = path.parent() {
            if parent.exists() {
                let test_file = parent.join(format!(".write_test_{}", std::process::id()));
                match fs::write(&test_file, b"") {
                    Ok(_) => {
                        let _ = fs::remove_file(&test_file);
                        true
                    }
                    Err(_) => false,
                }
            } else {
                match fs::create_dir_all(parent) {
                    Ok(_) => {
                        let test_file = parent.join(format!(".write_test_{}", std::process::id()));
                        match fs::write(&test_file, b"") {
                            Ok(_) => {
                                let _ = fs::remove_file(&test_file);
                                true
                            }
                            Err(_) => false,
                        }
                    }
                    Err(_) => false,
                }
            }
        } else {
            false
        }
    }

    /// Resolves primary configuration file path.
    /// Priority:
    /// 1. Portable config.ini beside the executable IF it exists and is writable.
    /// 2. If running from a write-protected directory (e.g. C:\Program Files\Easy-Mic):
    ///    uses %APPDATA%\Easy-Mic\config.ini.
    /// 3. Default portable config.ini beside the executable if the executable directory is writable.
    pub fn get_config_path() -> PathBuf {
        let exe_dir = Self::get_exe_dir();
        let candidate_config_ini = exe_dir.join("config.ini");

        // 1. Next to current executable: config.ini
        if candidate_config_ini.exists() {
            if Self::is_path_writable(&candidate_config_ini) {
                return candidate_config_ini;
            }
            // If beside exe exists but is NOT writable (e.g. in Program Files without admin):
            if let Some(appdata_ini) = Self::get_appdata_config_path() {
                return appdata_ini;
            }
            return candidate_config_ini;
        }

        // 2. If %APPDATA%\Easy-Mic\config.ini already exists:
        if let Some(appdata_ini) = Self::get_appdata_config_path() {
            if appdata_ini.exists() {
                return appdata_ini;
            }
        }

        // 3. If neither exists, check if exe directory is writable (portable mode)
        if Self::is_path_writable(&candidate_config_ini) {
            return candidate_config_ini;
        }

        // 4. If exe directory is write-protected (e.g. C:\Program Files), use %APPDATA%
        if let Some(appdata_ini) = Self::get_appdata_config_path() {
            return appdata_ini;
        }

        // 5. Compatibility fallback: conf.ini
        let candidate_conf_ini = exe_dir.join("conf.ini");
        if candidate_conf_ini.exists() {
            return candidate_conf_ini;
        }

        // 6. User-dropped legacy config files (easy-mic.d, easymic.d, conf.d, conf.b, conf.json)
        for d_name in ["easy-mic.d", "Easy-Mic.d", "easymic.d"] {
            let candidate_d = exe_dir.join(d_name);
            if candidate_d.exists() {
                let is_compiler_artifact = fs::read_to_string(&candidate_d)
                    .map(|content| content.contains(".exe:") || content.contains(".rs"))
                    .unwrap_or(false);
                if !is_compiler_artifact {
                    return candidate_d;
                }
            }
        }
        let candidate_conf_d = exe_dir.join("conf.d");
        if candidate_conf_d.exists() {
            return candidate_conf_d;
        }
        let candidate_b = exe_dir.join("conf.b");
        if candidate_b.exists() {
            return candidate_b;
        }
        let candidate_json = exe_dir.join("conf.json");
        if candidate_json.exists() {
            return candidate_json;
        }

        // Default: candidate_config_ini
        candidate_config_ini
    }

    /// Loads configuration from disk, falling back to default.
    /// Ensures config.ini is created immediately (beside exe or in %APPDATA%).
    pub fn load() -> Self {
        let exe_dir = Self::get_exe_dir();

        // Clean up compiler artifact if present (rustc emits .d dep-info on build)
        for d_name in ["easy-mic.d", "Easy-Mic.d", "easymic.d"] {
            let candidate_d = exe_dir.join(d_name);
            if candidate_d.exists() {
                if let Ok(content) = fs::read_to_string(&candidate_d) {
                    if content.contains(".exe:") || content.contains(".rs") {
                        let _ = fs::remove_file(&candidate_d);
                    }
                }
            }
        }

        let target_ini = Self::get_config_path();

        if target_ini.exists() {
            return Self::load_from_path(&target_ini);
        }

        // If target doesn't exist, check candidate beside exe or legacy imports
        let candidate_exe_ini = exe_dir.join("config.ini");
        let cfg = if candidate_exe_ini.exists() {
            Self::load_from_path(&candidate_exe_ini)
        } else {
            // Check legacy ProgramFiles import once if present
            let mut imported = None;
            if let Ok(pf) = std::env::var("ProgramFiles") {
                let sys_b = PathBuf::from(pf).join("Easymic").join("conf.b");
                if sys_b.exists() {
                    if let Ok(bytes) = fs::read(&sys_b) {
                        imported = Self::parse_beve(&bytes);
                    }
                }
            }
            imported.unwrap_or_default()
        };

        // Ensure parent directory exists (e.g. %APPDATA%\Easy-Mic) and create initial config.ini
        if let Some(parent) = target_ini.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = cfg.save_to_path(&target_ini);
        cfg
    }

    pub fn load_from_path(path: &Path) -> Self {
        if !path.exists() {
            log::info!("Config file not found at {:?}, using defaults", path);
            return Self::default();
        }

        match fs::read(path) {
            Ok(bytes) => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let is_ini_ext = ext.eq_ignore_ascii_case("ini");
                let is_json_ext = ext.eq_ignore_ascii_case("json");

                // If it has .ini extension or text that is not JSON, try INI first
                if is_ini_ext || (!is_json_ext && !bytes.starts_with(b"{")) {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        if let Some(cfg) = Self::parse_ini(text) {
                            log::info!("Loaded configuration INI from {:?}", path);
                            return cfg;
                        }
                    }
                }

                // Try JSON
                if is_json_ext || bytes.starts_with(b"{") {
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

                // Fallback attempt text INI or JSON
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    if let Some(cfg) = Self::parse_ini(text) {
                        log::info!("Loaded configuration INI from {:?}", path);
                        return cfg;
                    }
                }
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

    /// Saves configuration to the active configuration path (beside exe or %APPDATA%).
    pub fn save(&self) -> Result<(), std::io::Error> {
        let target_path = Self::get_config_path();
        self.save_to_path(&target_path)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), std::io::Error> {
        // If the path was an old legacy file (conf.b, easymic.d, conf.d), migrate target to config.ini
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let target_path = if ext.eq_ignore_ascii_case("b")
            || ext.eq_ignore_ascii_case("d")
            || file_name.eq_ignore_ascii_case("easy-mic.d")
            || file_name.eq_ignore_ascii_case("easymic.d")
            || file_name.eq_ignore_ascii_case("conf.d")
        {
            path.with_file_name("config.ini")
        } else {
            path.to_path_buf()
        };

        if let Some(parent) = target_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let result = if target_path.extension().and_then(|e| e.to_str()) == Some("json") {
            let json = serde_json::to_string_pretty(self)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            fs::write(&target_path, json)
        } else {
            let ini = self.to_ini();
            fs::write(&target_path, ini)
        };

        if let Err(ref e) = result {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                if let Some(appdata_ini) = Self::get_appdata_config_path() {
                    if appdata_ini != target_path {
                        if let Some(parent) = appdata_ini.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let ini = self.to_ini();
                        if fs::write(&appdata_ini, ini).is_ok() {
                            log::info!(
                                "Permission denied writing to {:?}; saved fallback to {:?}",
                                target_path,
                                appdata_ini
                            );
                            return Ok(());
                        }
                    }
                }
            }
        }

        result?;
        log::info!("Saved configuration to {:?}", target_path);
        Ok(())
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    let lower = s.trim().to_ascii_lowercase();
    match lower.as_str() {
        "true" | "1" | "yes" | "on" | "enabled" => Some(true),
        "false" | "0" | "no" | "off" | "disabled" => Some(false),
        _ => None,
    }
}

fn parse_indicator_state(s: &str) -> IndicatorState {
    let lower = s.trim().to_ascii_lowercase();
    match lower.as_str() {
        "0" | "hidden" => IndicatorState::Hidden,
        "1" | "muted" => IndicatorState::Muted,
        "2" | "mutedortalk" | "mutedortalking" | "muted_or_talk" => IndicatorState::MutedOrTalk,
        _ => IndicatorState::Hidden,
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
        assert_eq!(cfg.indicator_size, 15);
        assert_eq!(cfg.indicator_state, IndicatorState::Hidden);
        assert!(cfg.is_mic_keep_volume);
        assert!(!cfg.is_skip_uac_enabled);
        assert!(!cfg.is_updates_enabled);
        assert!(cfg.hide_when_inactive);
        assert!(!cfg.actions.is_empty());
        assert_eq!(cfg.actions[0].name, "Toggle mute");
        assert_eq!(cfg.actions[0].hotkey, SHIFT_Z_HOTKEY_MASK);
        assert_eq!(cfg.mute_sound_source, "Off");
        assert_eq!(cfg.unmute_sound_source, "On");
    }

    #[test]
    fn test_ini_serialization_roundtrip() {
        let cfg = AppConfig::default();
        let ini_text = cfg.to_ini();
        assert!(ini_text.contains("[General]"));
        assert!(ini_text.contains("[Indicator]"));
        assert!(ini_text.contains("[Sounds]"));
        assert!(ini_text.contains("[Action.1]"));
        assert!(ini_text.contains("Hotkey = SHIFT + Z"));

        let parsed = AppConfig::parse_ini(&ini_text).expect("parse default ini");
        assert_eq!(parsed.bell_volume, cfg.bell_volume);
        assert_eq!(parsed.mic_volume, cfg.mic_volume);
        assert_eq!(parsed.indicator_size, cfg.indicator_size);
        assert_eq!(parsed.indicator_state, cfg.indicator_state);
        assert!((parsed.indicator_volume_threshold - cfg.indicator_volume_threshold).abs() < 1e-4);
        assert_eq!(parsed.is_mic_keep_volume, cfg.is_mic_keep_volume);
        assert_eq!(parsed.is_skip_uac_enabled, cfg.is_skip_uac_enabled);
        assert_eq!(parsed.actions.len(), cfg.actions.len());
        assert_eq!(parsed.actions[0].name, cfg.actions[0].name);
        assert_eq!(parsed.actions[0].hotkey, cfg.actions[0].hotkey);
        assert_eq!(parsed.mute_sound_source, cfg.mute_sound_source);
        assert_eq!(parsed.unmute_sound_source, cfg.unmute_sound_source);
    }

    #[test]
    fn test_ini_custom_values() {
        let mut cfg = AppConfig::default();
        cfg.bell_volume = 35;
        cfg.mic_volume = 80;
        cfg.indicator_size = 24;
        cfg.indicator_state = IndicatorState::MutedOrTalk;
        cfg.window_pos_x = 120;
        cfg.window_pos_y = 240;
        cfg.mute_sound_source = "custom_off.wav".to_string();
        cfg.unmute_sound_source = "custom_on.wav".to_string();

        let ini_text = cfg.to_ini();
        let parsed = AppConfig::parse_ini(&ini_text).expect("parse custom ini");
        assert_eq!(parsed.bell_volume, 35);
        assert_eq!(parsed.mic_volume, 80);
        assert_eq!(parsed.indicator_size, 24);
        assert_eq!(parsed.indicator_state, IndicatorState::MutedOrTalk);
        assert_eq!(parsed.window_pos_x, 120);
        assert_eq!(parsed.window_pos_y, 240);
        assert_eq!(parsed.mute_sound_source, "custom_off.wav");
        assert_eq!(parsed.unmute_sound_source, "custom_on.wav");
    }

    #[test]
    fn test_get_config_path_beside_exe() {
        let path = AppConfig::get_config_path();
        let exe_dir = AppConfig::get_exe_dir();
        let is_beside_exe = path.parent().map(|p| p == exe_dir).unwrap_or(false);
        let is_appdata = AppConfig::get_appdata_dir()
            .map(|d| path.parent() == Some(&d))
            .unwrap_or(false);
        assert!(is_beside_exe || is_appdata);
        assert!(
            path.ends_with("config.ini")
                || path.ends_with("conf.ini")
                || path.ends_with("conf.b")
                || path.ends_with("conf.json")
                || path.ends_with("easy-mic.d")
                || path.ends_with("Easy-Mic.d")
                || path.ends_with("easymic.d")
                || path.ends_with("conf.d")
        );
    }

    #[test]
    fn test_ini_forgiving_parser() {
        let snippet = r#"
        ; Custom configuration test
        [general]
        version = 2
        skipuac = 1
        startwithwindows = true
        multipresswindowms = 250

        [indicator]
        state = muted
        size = 20
        volumethreshold = 0.005 ; inline comment test
        positionx = 50
        positiony = 100

        [sounds]
        micvolume = 95
        bellvolume = 40
        mutesound = "my_mute.wav"
        unmutesound = 'my_unmute.wav'

        [action.1]
        name = Custom Toggle
        type = Toggle mute
        hotkey = ctrl + alt + z
        presses = 1
        block = yes
        "#;

        let parsed = AppConfig::parse_ini(snippet).expect("parse forgiving snippet");
        assert_eq!(parsed.version, 2);
        assert!(parsed.is_skip_uac_enabled);
        assert!(parsed.start_with_windows);
        assert_eq!(parsed.multi_press_window_ms, 250);
        assert_eq!(parsed.indicator_state, IndicatorState::Muted);
        assert_eq!(parsed.indicator_size, 20);
        assert!((parsed.indicator_volume_threshold - 0.005).abs() < 1e-4);
        assert_eq!(parsed.window_pos_x, 50);
        assert_eq!(parsed.window_pos_y, 100);
        assert_eq!(parsed.mic_volume, 95);
        assert_eq!(parsed.bell_volume, 40);
        assert_eq!(parsed.mute_sound_source, "my_mute.wav");
        assert_eq!(parsed.unmute_sound_source, "my_unmute.wav");
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(parsed.actions[0].name, "Custom Toggle");
        assert!(parsed.actions[0].block);
    }

    #[test]
    fn test_config_json_serialization_roundtrip() {
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
        if let Ok(pf) = std::env::var("ProgramFiles") {
            let path = PathBuf::from(pf).join("Easymic").join("conf.b");
            if path.exists() {
                let bytes = fs::read(&path).expect("read conf.b");
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
                assert_eq!(cfg.mute_sound_source, "Off");
                assert_eq!(cfg.unmute_sound_source, "On");
            }
        }
    }

    #[test]
    fn test_ini_formatting_actions_blank_line_and_no_trailing_blank_line() {
        let cfg = AppConfig::default();
        let ini = cfg.to_ini();

        // Must have blank line between Actions Configuration header and [Action.1]
        assert!(
            ini.contains(";\n\n[Action.1]")
                || ini.contains(";\r\n\r\n[Action.1]")
                || ini.contains("; ==============================================================================\n\n[Action.1]")
                || ini.contains("; ==============================================================================\r\n\r\n[Action.1]")
        );

        // Must NOT end with double newline (no extra blank line at the end)
        assert!(!ini.ends_with("\n\n"));
        assert!(!ini.ends_with("\r\n\r\n"));
        assert!(ini.ends_with("Args = \n") || ini.ends_with("Args = \r\n"));
    }

    #[test]
    fn test_non_config_text_rejected_by_parse_ini() {
        let fake_dep_info = "C:\\projects\\Easy-Mic.exe: C:\\projects\\src\\main.rs C:\\projects\\build.rs\n";
        assert!(AppConfig::parse_ini(fake_dep_info).is_none());
    }

    #[test]
    fn test_save_to_path_redirects_legacy_extensions() {
        let temp_dir = std::env::temp_dir().join("easymic_test_migration");
        let _ = fs::create_dir_all(&temp_dir);

        let legacy_d = temp_dir.join("easymic.d");
        let cfg = AppConfig::default();
        let res = cfg.save_to_path(&legacy_d);
        assert!(res.is_ok());

        // easymic.d should NOT have been created; config.ini should exist instead
        assert!(!legacy_d.exists());
        let migrated_ini = temp_dir.join("config.ini");
        assert!(migrated_ini.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_is_path_writable() {
        let temp_dir = std::env::temp_dir().join("easymic_test_writable");
        let _ = fs::create_dir_all(&temp_dir);
        let test_ini = temp_dir.join("config.ini");
        assert!(AppConfig::is_path_writable(&test_ini));
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_appdata_path_resolution() {
        if std::env::var_os("APPDATA").is_some() {
            assert!(AppConfig::get_appdata_dir().is_some());
            assert!(AppConfig::get_appdata_config_path().is_some());
            let appdata_ini = AppConfig::get_appdata_config_path().unwrap();
            assert!(appdata_ini.ends_with(Path::new("Easy-Mic").join("config.ini")));
        }
    }
}

