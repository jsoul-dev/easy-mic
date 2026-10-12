pub use windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub const MOD_LCTRL: u8 = 0x04;
pub const MOD_LSHIFT: u8 = 0x08;
pub const MOD_LALT: u8 = 0x10;
pub const MOD_RCTRL: u8 = 0x20;
pub const MOD_RSHIFT: u8 = 0x40;
pub const MOD_RALT: u8 = 0x80;

pub struct ModifierEntry {
    pub bit: u8,
    pub name: &'static str,
}

pub const MODIFIERS_ORDERED: &[ModifierEntry] = &[
    ModifierEntry {
        bit: MOD_LCTRL,
        name: "CTRL",
    },
    ModifierEntry {
        bit: MOD_RCTRL,
        name: "RCTRL",
    },
    ModifierEntry {
        bit: MOD_LSHIFT,
        name: "SHIFT",
    },
    ModifierEntry {
        bit: MOD_RSHIFT,
        name: "RSHIFT",
    },
    ModifierEntry {
        bit: MOD_LALT,
        name: "ALT",
    },
    ModifierEntry {
        bit: MOD_RALT,
        name: "RALT",
    },
];

pub fn get_vk_name(vk: u8) -> Option<&'static str> {
    match VIRTUAL_KEY(vk as u16) {
        VK_LBUTTON => Some("MouseLeft"),
        VK_RBUTTON => Some("MouseRight"),
        VK_CANCEL => Some("Cancel"),
        VK_MBUTTON => Some("MouseMiddle"),
        VK_XBUTTON1 => Some("MouseX1"),
        VK_XBUTTON2 => Some("MouseX2"),
        VK_BACK => Some("Back"),
        VK_TAB => Some("Tab"),
        VK_CLEAR => Some("Clear"),
        VK_RETURN => Some("Enter"),
        VK_SHIFT => Some("Shift"),
        VK_CONTROL => Some("Ctrl"),
        VK_MENU => Some("Alt"),
        VK_PAUSE => Some("Pause"),
        VK_CAPITAL => Some("CapsLock"),
        VK_ESCAPE => Some("Escape"),
        VK_SPACE => Some("Space"),
        VK_PRIOR => Some("PageUp"),
        VK_NEXT => Some("PageDown"),
        VK_END => Some("End"),
        VK_HOME => Some("Home"),
        VK_LEFT => Some("Left"),
        VK_UP => Some("Up"),
        VK_RIGHT => Some("Right"),
        VK_DOWN => Some("Down"),
        VK_SELECT => Some("Select"),
        VK_PRINT => Some("Print"),
        VK_EXECUTE => Some("Execute"),
        VK_SNAPSHOT => Some("PrintScreen"),
        VK_INSERT => Some("Insert"),
        VK_DELETE => Some("Delete"),
        VK_HELP => Some("Help"),
        VK_0 => Some("0"),
        VK_1 => Some("1"),
        VK_2 => Some("2"),
        VK_3 => Some("3"),
        VK_4 => Some("4"),
        VK_5 => Some("5"),
        VK_6 => Some("6"),
        VK_7 => Some("7"),
        VK_8 => Some("8"),
        VK_9 => Some("9"),
        VK_A => Some("A"),
        VK_B => Some("B"),
        VK_C => Some("C"),
        VK_D => Some("D"),
        VK_E => Some("E"),
        VK_F => Some("F"),
        VK_G => Some("G"),
        VK_H => Some("H"),
        VK_I => Some("I"),
        VK_J => Some("J"),
        VK_K => Some("K"),
        VK_L => Some("L"),
        VK_M => Some("M"),
        VK_N => Some("N"),
        VK_O => Some("O"),
        VK_P => Some("P"),
        VK_Q => Some("Q"),
        VK_R => Some("R"),
        VK_S => Some("S"),
        VK_T => Some("T"),
        VK_U => Some("U"),
        VK_V => Some("V"),
        VK_W => Some("W"),
        VK_X => Some("X"),
        VK_Y => Some("Y"),
        VK_Z => Some("Z"),
        VK_LWIN => Some("LWin"),
        VK_RWIN => Some("RWin"),
        VK_APPS => Some("Apps"),
        VK_SLEEP => Some("Sleep"),
        VK_NUMPAD0 => Some("NumPad0"),
        VK_NUMPAD1 => Some("NumPad1"),
        VK_NUMPAD2 => Some("NumPad2"),
        VK_NUMPAD3 => Some("NumPad3"),
        VK_NUMPAD4 => Some("NumPad4"),
        VK_NUMPAD5 => Some("NumPad5"),
        VK_NUMPAD6 => Some("NumPad6"),
        VK_NUMPAD7 => Some("NumPad7"),
        VK_NUMPAD8 => Some("NumPad8"),
        VK_NUMPAD9 => Some("NumPad9"),
        VK_MULTIPLY => Some("Multiply"),
        VK_ADD => Some("Add"),
        VK_SEPARATOR => Some("Separator"),
        VK_SUBTRACT => Some("Subtract"),
        VK_DECIMAL => Some("Decimal"),
        VK_DIVIDE => Some("Divide"),
        VK_F1 => Some("F1"),
        VK_F2 => Some("F2"),
        VK_F3 => Some("F3"),
        VK_F4 => Some("F4"),
        VK_F5 => Some("F5"),
        VK_F6 => Some("F6"),
        VK_F7 => Some("F7"),
        VK_F8 => Some("F8"),
        VK_F9 => Some("F9"),
        VK_F10 => Some("F10"),
        VK_F11 => Some("F11"),
        VK_F12 => Some("F12"),
        VK_F13 => Some("F13"),
        VK_F14 => Some("F14"),
        VK_F15 => Some("F15"),
        VK_F16 => Some("F16"),
        VK_F17 => Some("F17"),
        VK_F18 => Some("F18"),
        VK_F19 => Some("F19"),
        VK_F20 => Some("F20"),
        VK_F21 => Some("F21"),
        VK_F22 => Some("F22"),
        VK_F23 => Some("F23"),
        VK_F24 => Some("F24"),
        VK_NUMLOCK => Some("NumLock"),
        VK_SCROLL => Some("ScrollLock"),
        VK_LSHIFT => Some("LSHIFT"),
        VK_RSHIFT => Some("RSHIFT"),
        VK_LCONTROL => Some("LCTRL"),
        VK_RCONTROL => Some("RCTRL"),
        VK_LMENU => Some("LALT"),
        VK_RMENU => Some("RALT"),
        VK_OEM_1 => Some(":"),
        VK_OEM_PLUS => Some("+"),
        VK_OEM_COMMA => Some(","),
        VK_OEM_MINUS => Some("-"),
        VK_OEM_PERIOD => Some("."),
        VK_OEM_2 => Some("/?"),
        VK_OEM_3 => Some("~"),
        VK_OEM_4 => Some("["),
        VK_OEM_5 => Some("\\"),
        VK_OEM_6 => Some("]"),
        VK_OEM_7 => Some("\""),
        VK_OEM_8 => Some("OEM_8"),
        VK_OEM_102 => Some("<"),
        _ => None,
    }
}

pub fn get_modifier_bit(vk: u8) -> u8 {
    match VIRTUAL_KEY(vk as u16) {
        VK_LCONTROL => MOD_LCTRL,
        VK_LSHIFT => MOD_LSHIFT,
        VK_LMENU => MOD_LALT,
        VK_RCONTROL => MOD_RCTRL,
        VK_RSHIFT => MOD_RSHIFT,
        VK_RMENU => MOD_RALT,
        _ => 0,
    }
}

pub fn is_mouse_button(vk: u8) -> bool {
    matches!(
        VIRTUAL_KEY(vk as u16),
        VK_LBUTTON | VK_RBUTTON | VK_MBUTTON | VK_XBUTTON1 | VK_XBUTTON2
    )
}

pub fn mask_contains_mouse_button(mask: u64) -> bool {
    for i in 1..8 {
        let vk = ((mask >> (i * 8)) & 0xFF) as u8;
        if vk != 0 && is_mouse_button(vk) {
            return true;
        }
    }
    false
}

pub fn get_hotkey_name(mask: u64) -> String {
    if mask == 0 {
        return String::new();
    }

    let modifier_byte = (mask & 0xFF) as u8;
    let mut modifier_parts = Vec::new();
    for m in MODIFIERS_ORDERED {
        if (modifier_byte & m.bit) != 0 {
            modifier_parts.push(m.name);
        }
    }

    let mut key_parts = Vec::new();
    for i in 1..8 {
        let key_byte = ((mask >> (i * 8)) & 0xFF) as u8;
        if key_byte == 0 {
            break;
        }

        if let Some(name) = get_vk_name(key_byte) {
            key_parts.insert(0, name.to_string());
        } else {
            key_parts.insert(0, format!("0x{:02X}", key_byte));
        }
    }

    let mod_str = modifier_parts.join(" + ");
    let key_str = key_parts.join(" + ");

    if !mod_str.is_empty() && !key_str.is_empty() {
        format!("{} + {}", mod_str, key_str)
    } else if !mod_str.is_empty() {
        mod_str
    } else {
        key_str
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_z_hotkey_name() {
        // Shift bit: MOD_LSHIFT = 0x08. VK_Z = 0x5A ('Z'). Mask: (0x5A << 8) | 0x08
        let mask = (0x5Au64 << 8) | (MOD_LSHIFT as u64);
        let name = get_hotkey_name(mask);
        assert_eq!(name, "SHIFT + Z");
    }

    #[test]
    fn test_mouse_button_detection() {
        let mouse_mask = (0x01u64 << 8) | (MOD_LCTRL as u64); // VK_LBUTTON + Ctrl
        assert!(mask_contains_mouse_button(mouse_mask));

        let keyboard_mask = (0x5Au64 << 8) | (MOD_LSHIFT as u64); // Shift + Z
        assert!(!mask_contains_mouse_button(keyboard_mask));
    }
}
