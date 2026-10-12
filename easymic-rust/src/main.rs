#![windows_subsystem = "windows"]

pub mod app;
pub mod audio;
pub mod core;
pub mod input;
pub mod ui;

use app::crash_handler::init_logging_and_crash_handler;
use app::lifecycle::AppLifecycle;
use app::single_instance::SingleInstanceGuard;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, ICC_TAB_CLASSES,
    INITCOMMONCONTROLSEX,
};

fn main() {
    // 1. Initialize crash logging and panic hook
    init_logging_and_crash_handler();
    log::info!("EasyMic Rust starting up");

    // 2. Enforce single instance via named Win32 Mutex
    let _instance_guard = match SingleInstanceGuard::acquire() {
        Some(guard) => guard,
        None => {
            log::info!(
                "Another instance of EasyMic is already active. Terminating duplicate process."
            );
            return;
        }
    };

    // 3. Initialize Windows COM MTA
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    // 4. Initialize Windows Common Controls (Trackbars, TabControl)
    unsafe {
        let icce = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES | ICC_TAB_CLASSES | ICC_STANDARD_CLASSES,
        };
        let _ = InitCommonControlsEx(&icce);
    }

    // 5. Initialize application lifecycle and run main message loop
    match AppLifecycle::new() {
        Ok(lifecycle) => {
            lifecycle.run_message_loop();
            lifecycle.shutdown();
        }
        Err(err) => {
            log::error!(
                "Fatal error during EasyMic lifecycle initialization: {:?}",
                err
            );
        }
    }

    // 6. Clean COM uninitialization
    unsafe {
        CoUninitialize();
    }

    log::info!("EasyMic terminated cleanly");
}
