//! Windows Registry autostart integration matching C++ Registry.hpp.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};

const AUTOSTART_SUBKEY: PCWSTR = w!(r#"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"#);
pub const APP_NAME_W: PCWSTR = w!("Easy-Mic");
pub const LEGACY_APP_NAME_W: PCWSTR = w!("Easymic");

pub fn is_in_auto_startup() -> bool {
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            AUTOSTART_SUBKEY,
            APP_NAME_W,
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        ) == ERROR_SUCCESS
    }
}

pub fn add_to_auto_startup() -> bool {
    unsafe {
        let mut path_buf = [0u16; 1024];
        let len = GetModuleFileNameW(None, &mut path_buf);
        if len == 0 || len as usize >= path_buf.len() {
            return false;
        }

        let raw_path = String::from_utf16_lossy(&path_buf[..len as usize]);
        let formatted = if raw_path.contains(' ') && !raw_path.starts_with('"') {
            format!("\"{}\"", raw_path)
        } else {
            raw_path
        };
        let wide: Vec<u16> = formatted.encode_utf16().chain(std::iter::once(0)).collect();
        let byte_len = (wide.len() * std::mem::size_of::<u16>()) as u32;

        // Clean legacy key if present
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, AUTOSTART_SUBKEY, LEGACY_APP_NAME_W);

        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            AUTOSTART_SUBKEY,
            APP_NAME_W,
            REG_SZ.0,
            Some(wide.as_ptr() as *const _),
            byte_len,
        ) == ERROR_SUCCESS
    }
}

pub fn remove_from_auto_startup() -> bool {
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, AUTOSTART_SUBKEY, LEGACY_APP_NAME_W);
        RegDeleteKeyValueW(HKEY_CURRENT_USER, AUTOSTART_SUBKEY, APP_NAME_W) == ERROR_SUCCESS
    }
}
