//! Single instance application guard using a named Win32 Mutex.

use windows::core::w;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, LPARAM, WPARAM,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, PostMessageW, SetForegroundWindow, ShowWindow, SW_RESTORE, WM_COMMAND,
};

pub const MUTEX_NAME: windows::core::PCWSTR = w!("Easy-Mic-8963D562-E35B-492A-A3D2-5FD724CE24B2");

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

impl SingleInstanceGuard {
    /// Attempts to acquire the single-instance mutex.
    /// Returns `Some(guard)` if this is the first running instance.
    /// Returns `None` if an instance of Easy-Mic is already running.
    pub fn acquire() -> Option<Self> {
        unsafe {
            let handle = match CreateMutexW(None, false, MUTEX_NAME) {
                Ok(h) => h,
                Err(e) => {
                    log::warn!("CreateMutexW failed: {:?}", e);
                    return None;
                }
            };

            let last_err = GetLastError();
            if last_err == ERROR_ALREADY_EXISTS {
                log::info!("Another instance of Easy-Mic is already running. Forwarding focus.");
                Self::activate_existing_instance();
                let _ = CloseHandle(handle);
                return None;
            }

            Some(Self { handle })
        }
    }

    /// Finds the existing instance's coordinator window or settings window
    /// and requests it to display/focus settings.
    fn activate_existing_instance() {
        unsafe {
            // First check if Settings window is already open
            let mut settings = FindWindowW(w!("Easy-MicSettingsWindow"), None).unwrap_or_default();
            if settings.0 as usize == 0 {
                settings = FindWindowW(w!("EasyMicSettingsWindow"), None).unwrap_or_default();
            }

            if settings.0 as usize != 0 {
                let _ = ShowWindow(settings, SW_RESTORE);
                let _ = SetForegroundWindow(settings);
                return;
            }

            // Otherwise, ask the coordinator window to open Settings (ID_TRAY_SETTINGS = 2001)
            let mut coord = FindWindowW(w!("Easy-MicCoordinatorWindow"), None).unwrap_or_default();
            if coord.0 as usize == 0 {
                coord = FindWindowW(w!("EasyMicCoordinatorWindow"), None).unwrap_or_default();
            }

            if coord.0 as usize != 0 {
                let _ = PostMessageW(coord, WM_COMMAND, WPARAM(2001), LPARAM(0));
            }
        }
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
