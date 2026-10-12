//! Application lifecycle manager and coordinator.
//! Orchestrates startup, main message pump, tray interaction, and clean shutdown.

use crate::audio::manager::AudioManager;
use crate::audio::reconciliation::ReconciliationWorker;
use crate::audio::sound::SoundManager;
use crate::core::config::AppConfig;
use crate::core::diagnostics::DiagnosticsBuffer;
use crate::core::state::{AuthoritativeState, SharedState};
use crate::input::capture::HotkeyCaptureSession;
use crate::input::dispatch::ActionDispatcher;
use crate::input::hook::HookManager;
use crate::ui::icons::AppIcons;
use crate::ui::indicator::IndicatorWindow;
use crate::ui::settings::SettingsWindow;
use crate::ui::tray::{TrayIcon, ID_TRAY_EXIT, ID_TRAY_SETTINGS, WM_TRAY_CALLBACK};
use parking_lot::RwLock;
use std::sync::Arc;
use windows::core::w;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct AppLifecycle {
    hinstance: HINSTANCE,
    pub config: Arc<RwLock<AppConfig>>,
    pub state: SharedState,
    pub diagnostics: Arc<DiagnosticsBuffer>,
    pub sound_manager: Arc<SoundManager>,
    pub audio_manager: Arc<AudioManager>,
    pub reconciliation: Option<Arc<ReconciliationWorker>>,
    pub dispatcher: Arc<ActionDispatcher>,
    pub capture_session: Arc<HotkeyCaptureSession>,
    pub hook_manager: Arc<HookManager>,
    pub icons: Arc<AppIcons>,
    pub indicator: Arc<IndicatorWindow>,
    pub tray: Arc<TrayIcon>,
    pub main_hwnd: HWND,
}

unsafe impl Send for AppLifecycle {}
unsafe impl Sync for AppLifecycle {}

static COORDINATOR: parking_lot::Mutex<Option<Arc<AppLifecycle>>> = parking_lot::Mutex::new(None);

impl AppLifecycle {
    pub fn new() -> Result<Arc<Self>, windows::core::Error> {
        let hinstance = HINSTANCE(unsafe { GetModuleHandleW(None).unwrap_or_default() }.0);

        // 1. Load configuration
        let config = Arc::new(RwLock::new(AppConfig::load()));

        // 2. Authoritative state & diagnostics
        let state = Arc::new(AuthoritativeState::new());
        let diagnostics = Arc::new(DiagnosticsBuffer::new(100));

        // 3. Icons
        let icons = Arc::new(AppIcons::new(32));

        // 4. Sound Manager
        let sound_manager = SoundManager::new();

        // 5. Audio Manager (COM endpoint & volume controller)
        let audio_manager = AudioManager::new(state.clone());

        // Set initial mic volume if configured
        {
            let cfg = config.read();
            if cfg.mic_volume >= 0 {
                let _ = audio_manager.set_volume_percent(cfg.mic_volume as u8);
            }
        }

        // 6. Reconciliation Worker (30s background loop)
        let reconciliation = Some(ReconciliationWorker::start(
            audio_manager.clone(),
            state.clone(),
        ));

        // 7. Action Dispatcher
        let dispatcher = ActionDispatcher::new(
            state.clone(),
            audio_manager.clone(),
            sound_manager.clone(),
            diagnostics.clone(),
            config.clone(),
        );

        // 8. Hotkey Capture Session
        let capture_session = Arc::new(HotkeyCaptureSession::new());

        // 9. Low-level Input Hook Manager
        let hook_manager =
            HookManager::new(config.clone(), dispatcher.clone(), capture_session.clone());

        // 10. Hidden coordinator window for message pump & tray callbacks
        let class_name = w!("EasyMicCoordinatorWindow");
        let main_hwnd = unsafe {
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(coordinator_wndproc),
                hInstance: hinstance,
                lpszClassName: class_name,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);

            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("EasyMicCoordinator"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                None,
                hinstance,
                None,
            )?
        };

        // 11. Indicator Window
        let indicator = IndicatorWindow::create(
            hinstance,
            state.clone(),
            audio_manager.clone(),
            config.clone(),
            icons.clone(),
        )?;

        // Indicator window subscribes directly to StateChangeEvent::NotificationPosted

        // 12. Tray Icon
        let tray = TrayIcon::new(main_hwnd, state.clone(), icons.clone());

        let lifecycle = Arc::new(Self {
            hinstance,
            config,
            state,
            diagnostics,
            sound_manager,
            audio_manager,
            reconciliation,
            dispatcher,
            capture_session,
            hook_manager,
            icons,
            indicator,
            tray,
            main_hwnd,
        });

        *COORDINATOR.lock() = Some(lifecycle.clone());

        log::info!("EasyMic lifecycle initialized successfully");
        Ok(lifecycle)
    }

    /// Runs the main Win32 message pump until WM_QUIT.
    pub fn run_message_loop(&self) {
        log::info!("Entering main message pump");
        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        log::info!("Exited main message pump");
    }

    pub fn shutdown(&self) {
        log::info!("Shutting down EasyMic");
        if let Some(ref r) = self.reconciliation {
            r.stop();
        }
        self.tray.remove_icon();
        self.hook_manager.shutdown();
        self.sound_manager.shutdown();

        // Save window position
        let (x, y) = self.indicator.get_position();
        {
            let mut cfg = self.config.write();
            cfg.window_pos_x = x;
            cfg.window_pos_y = y;
            let _ = cfg.save();
        }
        log::info!("Shutdown complete");
    }
}

unsafe extern "system" fn coordinator_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_CALLBACK => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                let coord = COORDINATOR.lock().clone();
                if let Some(life) = coord {
                    if let Some(cmd) = life.tray.show_context_menu() {
                        handle_tray_command(hwnd, cmd, &life);
                    }
                }
            } else if event == WM_LBUTTONDBLCLK {
                let coord = COORDINATOR.lock().clone();
                if let Some(life) = coord {
                    handle_tray_command(hwnd, ID_TRAY_SETTINGS, &life);
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = wparam.0 & 0xFFFF;
            let coord = COORDINATOR.lock().clone();
            if let Some(life) = coord {
                handle_tray_command(hwnd, cmd, &life);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn handle_tray_command(_hwnd: HWND, cmd: usize, life: &AppLifecycle) {
    match cmd {
        ID_TRAY_SETTINGS => {
            SettingsWindow::open(
                life.hinstance,
                life.main_hwnd,
                life.config.clone(),
                life.state.clone(),
                life.audio_manager.clone(),
                life.sound_manager.clone(),
                life.hook_manager.clone(),
                life.indicator.clone(),
                life.diagnostics.clone(),
                life.capture_session.clone(),
            );
        }
        ID_TRAY_EXIT => unsafe {
            PostQuitMessage(0);
        },
        _ => {}
    }
}
