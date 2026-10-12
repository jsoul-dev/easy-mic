//! Crash handler, panic hook, and in-memory log buffer for EasyMic.

use chrono::Local;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use windows::core::w;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: log::Level,
    pub target: String,
    pub message: String,
}

pub struct AppLogger {
    entries: RwLock<VecDeque<LogEntry>>,
    log_file_path: Option<PathBuf>,
}

static LOGGER: RwLock<Option<Arc<AppLogger>>> = RwLock::new(None);

impl AppLogger {
    pub fn new() -> Arc<Self> {
        let log_file_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("easymic.log")));

        Arc::new(Self {
            entries: RwLock::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)),
            log_file_path,
        })
    }

    pub fn get_entries(&self) -> Vec<LogEntry> {
        self.entries.read().iter().cloned().collect()
    }
}

impl log::Log for AppLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        let entry = LogEntry {
            timestamp: timestamp.clone(),
            level: record.level(),
            target: record.target().to_string(),
            message: format!("{}", record.args()),
        };

        {
            let mut entries = self.entries.write();
            if entries.len() >= MAX_LOG_ENTRIES {
                entries.pop_front();
            }
            entries.push_back(entry);
        }

        // Also write to log file if available
        if let Some(ref path) = self.log_file_path {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(
                    f,
                    "[{}] [{}] [{}] {}",
                    timestamp,
                    record.level(),
                    record.target(),
                    record.args()
                );
            }
        }
    }

    fn flush(&self) {}
}

pub fn get_logger() -> Option<Arc<AppLogger>> {
    LOGGER.read().clone()
}

pub fn init_logging_and_crash_handler() {
    let app_logger = AppLogger::new();
    *LOGGER.write() = Some(app_logger.clone());

    // Register with log crate
    let leaked_logger: &'static AppLoggerWrapper =
        Box::leak(Box::new(AppLoggerWrapper(app_logger)));
    let _ = log::set_logger(leaked_logger);
    log::set_max_level(log::LevelFilter::Info);

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

struct AppLoggerWrapper(Arc<AppLogger>);

impl log::Log for AppLoggerWrapper {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.0.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        self.0.log(record);
    }

    fn flush(&self) {
        self.0.flush();
    }
}
