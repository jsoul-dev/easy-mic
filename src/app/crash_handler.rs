//! Crash handler and panic hook for EasyMic.

use chrono::Local;
use std::path::PathBuf;
use windows::core::w;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

pub struct AppLogger;

impl log::Log for AppLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        false
    }

    fn log(&self, _record: &log::Record) {}

    fn flush(&self) {}
}

static LOGGER: AppLogger = AppLogger;

/// Determines the best location for easy-mic-crash.log, preferring the directory
/// beside config.ini and checking whether it is writable.
pub fn get_crash_log_path() -> PathBuf {
    let config_path = crate::core::config::AppConfig::get_config_path();
    if let Some(parent) = config_path.parent() {
        let candidate = parent.join("easy-mic-crash.log");
        if crate::core::config::AppConfig::is_path_writable(&candidate) {
            return candidate;
        }
    }

    // Fallback 1: beside executable if writable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("easy-mic-crash.log");
            if crate::core::config::AppConfig::is_path_writable(&candidate) {
                return candidate;
            }
        }
    }

    // Fallback 2: %TEMP%\easy-mic-crash.log
    std::env::temp_dir().join("easy-mic-crash.log")
}

pub fn init_logging_and_crash_handler() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Off);

    // Register panic hook
    std::panic::set_hook(Box::new(|panic_info| {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "Unknown panic payload"
        };

        let location = if let Some(loc) = panic_info.location() {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        } else {
            "unknown location".to_string()
        };

        let thread_name = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let backtrace = std::backtrace::Backtrace::capture();
        let panic_str = panic_info.to_string();

        let report = format!(
            "Easy-Mic Crash Report\nTime: {}\nThread: {}\nLocation: {}\nError: {}\nDetails: {}\n\nBacktrace:\n{:?}",
            now, thread_name, location, payload, panic_str, backtrace
        );

        log::error!("CRASH:\n{}", report);

        // Save to easy-mic-crash.log beside config.ini (or fallback)
        let crash_file = get_crash_log_path();
        let _ = std::fs::write(&crash_file, &report);

        // Show native message box
        let message_wide: Vec<u16> = format!(
            "Easy-Mic encountered an unexpected error and must terminate:\n\n{}\n\nDetails saved to:\n{}",
            payload,
            crash_file.display()
        )
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

        unsafe {
            let _ = MessageBoxW(
                None,
                windows::core::PCWSTR(message_wide.as_ptr()),
                w!("Easy-Mic Crash"),
                MB_OK | MB_ICONERROR,
            );
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_crash_log_path_resolves_and_writable() {
        let path = get_crash_log_path();
        assert!(path.ends_with("easy-mic-crash.log"));
        assert!(crate::core::config::AppConfig::is_path_writable(&path));
    }
}
