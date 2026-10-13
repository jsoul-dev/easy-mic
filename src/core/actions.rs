//! Built-in action definitions and catalog.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltInId {
    ToggleMute,
    PushToTalk,
    MicVolumeUp,
    MicVolumeDown,
    ToggleBellSound,
    SwitchLanguage,
}

#[derive(Debug, Clone)]
pub struct BuiltInAction {
    pub id: BuiltInId,
    pub key: &'static str,
    pub title: &'static str,
    pub default_sound: &'static str,
    pub default_notification: &'static str,
    pub args_label: &'static str,
    pub args_hint: &'static str,
    pub has_sound: bool,
    pub hold_only: bool,
}

pub const DEFAULT_NOTIFICATION: &str = "{name} triggered";

pub const ALL_BUILT_IN_ACTIONS: &[BuiltInAction] = &[
    BuiltInAction {
        id: BuiltInId::ToggleMute,
        key: "Toggle mute",
        title: "Toggle mute",
        default_sound: "",
        default_notification: "Mic {mic}",
        args_label: "",
        args_hint: "",
        has_sound: false,
        hold_only: false,
    },
    BuiltInAction {
        id: BuiltInId::PushToTalk,
        key: "Push to talk",
        title: "Push to talk",
        default_sound: "",
        default_notification: "",
        args_label: "",
        args_hint: "",
        has_sound: false,
        hold_only: true,
    },
    BuiltInAction {
        id: BuiltInId::MicVolumeUp,
        key: "Mic volume up",
        title: "Mic volume up",
        default_sound: "Tick",
        default_notification: "Mic {volume}%",
        args_label: "",
        args_hint: "",
        has_sound: true,
        hold_only: false,
    },
    BuiltInAction {
        id: BuiltInId::MicVolumeDown,
        key: "Mic volume down",
        title: "Mic volume down",
        default_sound: "Tick",
        default_notification: "Mic {volume}%",
        args_label: "",
        args_hint: "",
        has_sound: true,
        hold_only: false,
    },
    BuiltInAction {
        id: BuiltInId::ToggleBellSound,
        key: "Toggle bell sound",
        title: "Toggle bell sound",
        default_sound: "Tick",
        default_notification: "Bell {bell}",
        args_label: "",
        args_hint: "",
        has_sound: true,
        hold_only: false,
    },
    BuiltInAction {
        id: BuiltInId::SwitchLanguage,
        key: "Switch language",
        title: "Switch language",
        default_sound: "",
        default_notification: "",
        args_label: "Locales",
        args_hint: "en, ru - empty for all",
        has_sound: true,
        hold_only: false,
    },
];

pub fn find_built_in(key: &str) -> Option<&'static BuiltInAction> {
    ALL_BUILT_IN_ACTIONS.iter().find(|a| a.key == key)
}

pub fn find_built_in_by_id(id: BuiltInId) -> Option<&'static BuiltInAction> {
    ALL_BUILT_IN_ACTIONS.iter().find(|a| a.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_built_in() {
        let toggle = find_built_in("Toggle mute").expect("Toggle mute should exist");
        assert_eq!(toggle.id, BuiltInId::ToggleMute);
        assert!(!toggle.hold_only);

        let ptt = find_built_in("Push to talk").expect("Push to talk should exist");
        assert_eq!(ptt.id, BuiltInId::PushToTalk);
        assert!(ptt.hold_only);

        assert!(find_built_in("Nonexistent action").is_none());
    }
}
