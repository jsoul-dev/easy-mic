//! Windows Autostart integration (Registry Run key and Task Scheduler with Skip UAC).

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const AUTOSTART_SUBKEY: PCWSTR = w!(r#"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"#);
pub const APP_NAME_W: PCWSTR = w!("Easy-Mic");
pub const LEGACY_APP_NAME_W: PCWSTR = w!("Easymic");
const TASK_NAME: &str = "Easy-Mic";
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Checks whether the current process is running with elevated Administrator privileges.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_ok() {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut return_length = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut return_length,
            )
            .is_ok();
            let _ = windows::Win32::Foundation::CloseHandle(token);
            if ok {
                return elevation.TokenIsElevated != 0;
            }
        }
        false
    }
}

pub fn get_current_exe_path() -> Option<String> {
    unsafe {
        let mut path_buf = [0u16; 1024];
        let len = GetModuleFileNameW(None, &mut path_buf);
        if len == 0 || len as usize >= path_buf.len() {
            return None;
        }
        Some(String::from_utf16_lossy(&path_buf[..len as usize]))
    }
}

pub fn is_in_auto_startup() -> bool {
    is_in_auto_startup_registry() || is_scheduled_task_present()
}

pub fn is_in_auto_startup_registry() -> bool {
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

pub fn is_scheduled_task_present() -> bool {
    use std::os::windows::process::CommandExt;
    let status = std::process::Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    status.map(|s| s.success()).unwrap_or(false)
}

pub fn add_to_auto_startup() -> bool {
    let Some(raw_path) = get_current_exe_path() else {
        return false;
    };
    let formatted = if raw_path.contains(' ') && !raw_path.starts_with('"') {
        format!("\"{}\"", raw_path)
    } else {
        raw_path
    };
    let wide: Vec<u16> = formatted.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = (wide.len() * std::mem::size_of::<u16>()) as u32;

    unsafe {
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

pub fn delete_scheduled_task() -> bool {
    if !is_scheduled_task_present() {
        return true;
    }
    use std::os::windows::process::CommandExt;
    let status = std::process::Command::new("schtasks")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    status.map(|s| s.success()).unwrap_or(false)
}

pub fn create_scheduled_task() -> bool {
    if is_scheduled_task_present() {
        return true;
    }
    let Some(raw_path) = get_current_exe_path() else {
        return false;
    };
    use std::os::windows::process::CommandExt;

    if is_elevated() {
        let status = std::process::Command::new("schtasks")
            .args([
                "/Create",
                "/TN",
                TASK_NAME,
                "/TR",
                &format!("\"{}\"", raw_path),
                "/SC",
                "ONLOGON",
                "/RL",
                "HIGHEST",
                "/F",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        status.map(|s| s.success()).unwrap_or(false)
    } else {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

        let args = format!(
            "/Create /TN \"{}\" /TR \"\\\"{}\\\"\" /SC ONLOGON /RL HIGHEST /F",
            TASK_NAME, raw_path
        );
        let args_wide: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();

        let res = unsafe {
            ShellExecuteW(
                None,
                w!("runas"),
                w!("schtasks.exe"),
                PCWSTR(args_wide.as_ptr()),
                None,
                SW_HIDE,
            )
        };
        res.0 as usize > 32
    }
}

/// Synchronizes autostart configuration with Windows (Task Scheduler or Registry).
pub fn sync_autostart(enabled: bool, skip_uac: bool) {
    if !enabled {
        remove_from_auto_startup();
        delete_scheduled_task();
    } else if skip_uac {
        remove_from_auto_startup();
        if !create_scheduled_task() {
            log::warn!("Could not create elevated Task Scheduler task; falling back to registry autostart");
            add_to_auto_startup();
        }
    } else {
        delete_scheduled_task();
        add_to_auto_startup();
    }
}
