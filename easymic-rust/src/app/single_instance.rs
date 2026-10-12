//! Single instance application guard using a named Win32 Mutex.

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

pub const MUTEX_NAME: windows::core::PCWSTR = w!("Easymic-8963D562-E35B-492A-A3D2-5FD724CE24B2");

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

impl SingleInstanceGuard {
    /// Attempts to acquire the single-instance mutex.
    /// Returns `Some(guard)` if this is the first running instance.
    /// Returns `None` if an instance of Easymic is already running.
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
                log::info!("Another instance of Easymic is already running. Exiting.");
                let _ = CloseHandle(handle);
                return None;
            }

            Some(Self { handle })
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
