//! Placeholder tokens used in notifications and command formatting.

pub const TOKEN_NAME: &str = "{name}";
pub const TOKEN_KEY: &str = "{key}";
pub const TOKEN_VOLUME: &str = "{volume}";
pub const TOKEN_MIC: &str = "{mic}";
pub const TOKEN_BELL: &str = "{bell}";
pub const TOKEN_STDOUT: &str = "{stdout}";
pub const TOKEN_DIR: &str = "{dir}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenField {
    Notification,
    Command,
}

#[derive(Debug, Clone)]
pub struct TokenDef {
    pub text: &'static str,
    pub description: &'static str,
    pub field: TokenField,
    pub custom_only: bool,
}

pub const ALL_TOKENS: &[TokenDef] = &[
    TokenDef {
        text: TOKEN_NAME,
        description: "Action name",
        field: TokenField::Notification,
        custom_only: false,
    },
    TokenDef {
        text: TOKEN_KEY,
        description: "Key combination",
        field: TokenField::Notification,
        custom_only: false,
    },
    TokenDef {
        text: TOKEN_VOLUME,
        description: "Microphone volume, 0-100",
        field: TokenField::Notification,
        custom_only: false,
    },
    TokenDef {
        text: TOKEN_MIC,
        description: "Microphone, on or off",
        field: TokenField::Notification,
        custom_only: false,
    },
    TokenDef {
        text: TOKEN_BELL,
        description: "Bell sound, on or off",
        field: TokenField::Notification,
        custom_only: false,
    },
    TokenDef {
        text: TOKEN_STDOUT,
        description: "Command output - waits for it to finish",
        field: TokenField::Notification,
        custom_only: true,
    },
    TokenDef {
        text: TOKEN_DIR,
        description: "Folder the active Explorer tab shows",
        field: TokenField::Command,
        custom_only: true,
    },
];

/// Replaces token strings in notification text.
pub fn replace_tokens(
    template: &str,
    name: &str,
    key_name: &str,
    volume_percent: u8,
    mic_muted: bool,
    bell_enabled: bool,
    stdout: Option<&str>,
) -> String {
    let mut res = template.to_string();
    if res.contains(TOKEN_NAME) {
        res = res.replace(TOKEN_NAME, name);
    }
    if res.contains(TOKEN_KEY) {
        res = res.replace(TOKEN_KEY, key_name);
    }
    if res.contains(TOKEN_VOLUME) {
        res = res.replace(TOKEN_VOLUME, &volume_percent.to_string());
    }
    if res.contains(TOKEN_MIC) {
        res = res.replace(TOKEN_MIC, if mic_muted { "off" } else { "on" });
    }
    if res.contains(TOKEN_BELL) {
        res = res.replace(TOKEN_BELL, if bell_enabled { "on" } else { "off" });
    }
    if let Some(out) = stdout {
        if res.contains(TOKEN_STDOUT) {
            res = res.replace(TOKEN_STDOUT, out);
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_replacement() {
        let template = "Mic is {mic}, vol {volume}%, bell {bell}";
        let formatted = replace_tokens(template, "Test", "Shift + Z", 75, true, false, None);
        assert_eq!(formatted, "Mic is off, vol 75%, bell off");

        let formatted_on = replace_tokens(template, "Test", "Shift + Z", 80, false, true, None);
        assert_eq!(formatted_on, "Mic is on, vol 80%, bell on");
    }

    #[test]
    fn test_name_and_key_replacement() {
        let template = "{name} ({key}) triggered";
        let formatted = replace_tokens(template, "Toggle mute", "Shift + Z", 50, false, true, None);
        assert_eq!(formatted, "Toggle mute (Shift + Z) triggered");
    }
}
