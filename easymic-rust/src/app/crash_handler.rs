//! Crash handler and panic hook for EasyMic.

use chrono::Local;
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

        let report = format!(
            "EasyMic Crash Report\nTime: {}\nThread: {}\nLocation: {}\nError: {}\n\nBacktrace:\n{:?}",
            now, thread_name, location, payload, backtrace
        );

        log::error!("CRASH:\n{}", report);

        // Save to easymic_crash.log
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let crash_file = dir.join("easymic_crash.log");
                let _ = std::fs::write(&crash_file, &report);
            }
        }

        // Show native message box
        let message_wide: Vec<u16> = format!(
            "EasyMic encountered an unexpected error and must terminate:\n\n{}\n\nDetails saved to easymic_crash.log",
            payload
        )
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

        unsafe {
            let _ = MessageBoxW(
                None,
                windows::core::PCWSTR(message_wide.as_ptr()),
                w!("EasyMic Crash"),
                MB_OK | MB_ICONERROR,
            );
        }
    }));
}
