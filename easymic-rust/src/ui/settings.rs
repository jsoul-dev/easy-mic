//! Settings window and Action editor for EasyMic.
//! Provides native Win32 tabbed settings interface:
//! 1. General (Autostart, UAC, Updates)
//! 2. Indicator (State, Size, Threshold, On-Top, Capture exclusion, Drag/Position)
//! 3. Sounds (Mic volume, Keep volume, Bell volume, Cues selection & test playback)
//! 4. Hotkeys (Multi-press window, Actions list, Interactive Hotkey capture, Edit/Add/Remove)
//! 5. About (Version, Configuration file shortcuts, GitHub repository)

use crate::app::registry;
use crate::audio::manager::AudioManager;
use crate::audio::sound::SoundManager;
use crate::core::actions::ALL_BUILT_IN_ACTIONS;
use crate::core::config::{ActionConfig, AppConfig, IndicatorState};
use crate::core::diagnostics::DiagnosticsBuffer;
use crate::core::state::SharedState;
use crate::input::capture::HotkeyCaptureSession;
use crate::input::hook::HookManager;
use crate::input::keys::get_hotkey_name;
use crate::ui::indicator::IndicatorWindow;
use parking_lot::RwLock;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{BOOL, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

const TBM_GETPOS: u32 = WM_USER;

static UI_NORMAL_FONT: parking_lot::Mutex<Option<isize>> = parking_lot::Mutex::new(None);
static UI_BOLD_FONT: parking_lot::Mutex<Option<isize>> = parking_lot::Mutex::new(None);
static UI_TITLE_FONT: parking_lot::Mutex<Option<isize>> = parking_lot::Mutex::new(None);

pub fn get_ui_normal_font() -> HFONT {
    let mut guard = UI_NORMAL_FONT.lock();
    if let Some(font_val) = *guard {
        return HFONT(font_val as *mut _);
    }
    unsafe {
        let font = CreateFontW(
            -13,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            CLEARTYPE_QUALITY.0 as u32,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        );
        *guard = Some(font.0 as isize);
        font
    }
}

pub fn get_ui_bold_font() -> HFONT {
    let mut guard = UI_BOLD_FONT.lock();
    if let Some(font_val) = *guard {
        return HFONT(font_val as *mut _);
    }
    unsafe {
        let font = CreateFontW(
            -13,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            CLEARTYPE_QUALITY.0 as u32,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        );
        *guard = Some(font.0 as isize);
        font
    }
}

pub fn get_ui_title_font() -> HFONT {
    let mut guard = UI_TITLE_FONT.lock();
    if let Some(font_val) = *guard {
        return HFONT(font_val as *mut _);
    }
    unsafe {
        let font = CreateFontW(
            -20,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            CLEARTYPE_QUALITY.0 as u32,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        );
        *guard = Some(font.0 as isize);
        font
    }
}

unsafe extern "system" fn set_font_callback(child: HWND, lparam: LPARAM) -> BOOL {
    let font = HFONT(lparam.0 as *mut _);
    let _ = SendMessageW(child, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    BOOL(1)
}

pub fn apply_ui_font_recursive(hwnd: HWND, font: HFONT) {
    unsafe {
        let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        let _ = EnumChildWindows(hwnd, Some(set_font_callback), LPARAM(font.0 as isize));
    }
}

#[inline]
unsafe fn get_ctrl(parent: HWND, id: usize) -> HWND {
    GetDlgItem(parent, id as i32).unwrap_or_default()
}

#[inline]
fn ws_child_vis_i(extra: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | (extra as u32))
}

#[inline]
fn ws_child_vis_u(extra: u32) -> WINDOW_STYLE {
    WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | extra)
}

// Control IDs
const IDC_TAB_CONTROL: usize = 101;
const IDC_BTN_OK: usize = 102;
const IDC_BTN_CANCEL: usize = 103;
const IDC_BTN_APPLY: usize = 104;

// General tab
const IDC_CHK_AUTOSTART: usize = 201;
const IDC_CHK_SKIP_UAC: usize = 202;
const IDC_CHK_UPDATES: usize = 203;

// Indicator tab
const IDC_CMB_INDICATOR_STATE: usize = 301;
const IDC_TRK_INDICATOR_SIZE: usize = 302;
const IDC_LBL_INDICATOR_SIZE: usize = 303;
const IDC_TRK_INDICATOR_THRESH: usize = 304;
const IDC_LBL_INDICATOR_THRESH: usize = 305;
const IDC_CHK_CAPTURE_EXCLUDE: usize = 306;
const IDC_CHK_ON_TOP: usize = 307;
const IDC_CHK_HIDE_INACTIVE: usize = 308;
const IDC_CHK_NOTIFICATIONS: usize = 309;
const IDC_BTN_RESET_POS: usize = 310;

// Sounds tab
const IDC_TRK_MIC_VOLUME: usize = 401;
const IDC_LBL_MIC_VOLUME: usize = 402;
const IDC_CHK_KEEP_VOLUME: usize = 403;
const IDC_TRK_BELL_VOLUME: usize = 404;
const IDC_LBL_BELL_VOLUME: usize = 405;
const IDC_CMB_MUTE_SOUND: usize = 406;
const IDC_BTN_TEST_MUTE: usize = 407;
const IDC_CMB_UNMUTE_SOUND: usize = 408;
const IDC_BTN_TEST_UNMUTE: usize = 409;
const IDC_BTN_BROWSE_MUTE: usize = 410;
const IDC_BTN_BROWSE_UNMUTE: usize = 411;

// Hotkeys tab
const IDC_TRK_MULTIPRESS: usize = 501;
const IDC_LBL_MULTIPRESS: usize = 502;
const IDC_LST_ACTIONS: usize = 503;
const IDC_BTN_ADD_ACTION: usize = 504;
const IDC_BTN_EDIT_ACTION: usize = 505;
const IDC_BTN_DELETE_ACTION: usize = 506;

// About tab
const IDC_BTN_GITHUB: usize = 601;
const IDC_BTN_OPEN_CONFIG: usize = 602;
const IDC_BTN_OPEN_FOLDER: usize = 603;

// Action Dialog control IDs
const IDC_ACT_NAME: usize = 701;
const IDC_ACT_TYPE: usize = 702;
const IDC_ACT_HOTKEY_TXT: usize = 703;
const IDC_ACT_BTN_CAPTURE: usize = 704;
const IDC_ACT_BTN_CLEAR_HOTKEY: usize = 705;
const IDC_ACT_PRESSES: usize = 706;
const IDC_ACT_CHK_RELEASE: usize = 707;
const IDC_ACT_CHK_BLOCK: usize = 708;
const IDC_ACT_CHK_TAP: usize = 709;
const IDC_ACT_CMB_SOUND: usize = 710;
const IDC_ACT_BTN_TEST_SOUND: usize = 717;
const IDC_ACT_BTN_BROWSE_SOUND: usize = 718;
const IDC_ACT_CHK_NOTIF: usize = 712;
const IDC_ACT_TXT_NOTIF: usize = 713;
const IDC_ACT_TXT_COMMAND: usize = 714;
const IDC_ACT_BTN_SAVE: usize = 715;
const IDC_ACT_BTN_CANCEL: usize = 716;

const WM_HOTKEY_CAPTURED: u32 = WM_USER + 501;
const REPO_URL_W: PCWSTR = w!("https://github.com/k1tbyte/Easymic");

pub struct SettingsWindow {
    hwnd: HWND,
    config: Arc<RwLock<AppConfig>>,
    orig_config: AppConfig,
    state: SharedState,
    audio: Arc<AudioManager>,
    sound: Arc<SoundManager>,
    hooks: Arc<HookManager>,
    indicator: Arc<IndicatorWindow>,
    capture_session: Arc<HotkeyCaptureSession>,
    active_tab: usize,
    tab_panels: [HWND; 5],
    autostart_initial: bool,
    autostart_pending: bool,
}

unsafe impl Send for SettingsWindow {}
unsafe impl Sync for SettingsWindow {}

static CURRENT_SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);

impl SettingsWindow {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        hinstance: HINSTANCE,
        parent: HWND,
        config: Arc<RwLock<AppConfig>>,
        state: SharedState,
        audio: Arc<AudioManager>,
        sound: Arc<SoundManager>,
        hooks: Arc<HookManager>,
        indicator: Arc<IndicatorWindow>,
        _diagnostics: Arc<DiagnosticsBuffer>,
        capture_session: Arc<HotkeyCaptureSession>,
    ) {
        let existing_val = CURRENT_SETTINGS_HWND.load(Ordering::Acquire);
        if existing_val != 0 {
            let existing = HWND(existing_val as *mut _);
            if unsafe { IsWindow(existing).as_bool() } {
                unsafe {
                    let _ = ShowWindow(existing, SW_RESTORE);
                    let _ = SetForegroundWindow(existing);
                }
                return;
            }
        }

        let orig = config.read().clone();
        let autostart = registry::is_in_auto_startup() || config.read().start_with_windows;

        // Enable indicator dragging while settings dialog is open
        indicator.toggle_interactivity(true);

        let window = Box::new(Self {
            hwnd: HWND::default(),
            config,
            orig_config: orig,
            state,
            audio,
            sound,
            hooks,
            indicator,
            capture_session,
            active_tab: 0,
            tab_panels: [HWND::default(); 5],
            autostart_initial: autostart,
            autostart_pending: autostart,
        });

        let state_ptr = Box::into_raw(window);

        unsafe {
            let class_name = w!("EasyMicSettingsWindow");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(settings_wndproc),
                hInstance: hinstance,
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as isize as *mut _),
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);

            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_APPWINDOW,
                class_name,
                w!("EasyMic Settings"),
                WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                640,
                520,
                parent,
                None,
                hinstance,
                Some(state_ptr as *mut _),
            )
            .expect("Failed to create SettingsWindow");

            CURRENT_SETTINGS_HWND.store(hwnd.0 as isize, Ordering::Release);

            // Center window on screen
            let mut rc = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rc);
            let w = rc.right - rc.left;
            let h = rc.bottom - rc.top;
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_w - w) / 2;
            let y = (screen_h - h) / 2;
            let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
        }
    }

    fn apply_changes(&mut self) {
        // Read live sound cue selections from comboboxes
        let h_mute_cmb = unsafe { get_ctrl(self.tab_panels[2], IDC_CMB_MUTE_SOUND) };
        let mute_txt = get_combo_selected_text(h_mute_cmb);
        let h_unmute_cmb = unsafe { get_ctrl(self.tab_panels[2], IDC_CMB_UNMUTE_SOUND) };
        let unmute_txt = get_combo_selected_text(h_unmute_cmb);

        // Save indicator position from live window
        let (pos_x, pos_y) = self.indicator.get_position();
        {
            let mut cfg = self.config.write();
            if !mute_txt.is_empty() {
                cfg.mute_sound_source = mute_txt.clone();
                cfg.recent_sounds.insert(mute_txt);
            }
            if !unmute_txt.is_empty() {
                cfg.unmute_sound_source = unmute_txt.clone();
                cfg.recent_sounds.insert(unmute_txt);
            }
            cfg.window_pos_x = pos_x;
            cfg.window_pos_y = pos_y;
            cfg.start_with_windows = self.autostart_pending;
            let _ = cfg.save();
        }

        // Apply autostart registry setting
        if self.autostart_pending != self.autostart_initial {
            if self.autostart_pending {
                registry::add_to_auto_startup();
            } else {
                registry::remove_from_auto_startup();
            }
            self.autostart_initial = self.autostart_pending;
        }

        // Sync volume to audio device if mic volume is configured
        let cfg = self.config.read();
        if cfg.mic_volume >= 0 {
            let _ = self.audio.set_volume_percent(cfg.mic_volume as u8);
        }

        // Refresh hook bindings
        self.hooks.update_bindings();

        // Re-render indicator
        self.indicator.render();
    }

    fn cancel_changes(&mut self) {
        // Restore original config
        *self.config.write() = self.orig_config.clone();
        let _ = self.orig_config.save();
        self.hooks.update_bindings();
        self.indicator.render();
    }
}

unsafe extern "system" fn settings_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = lparam.0 as *const CREATESTRUCTW;
            if !cs.is_null() && !(*cs).lpCreateParams.is_null() {
                let state_ptr = (*cs).lpCreateParams as *mut SettingsWindow;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
                let win = &mut *state_ptr;
                win.hwnd = hwnd;
                init_settings_controls(hwnd, win);
            }
            LRESULT(0)
        }
        WM_NOTIFY => {
            let pnmh = lparam.0 as *const NMHDR;
            if !pnmh.is_null() && (*pnmh).idFrom == IDC_TAB_CONTROL && (*pnmh).code == TCN_SELCHANGE
            {
                let tab_ctrl = (*pnmh).hwndFrom;
                let cur_sel =
                    SendMessageW(tab_ctrl, TCM_GETCURSEL, WPARAM(0), LPARAM(0)).0 as usize;
                let win_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
                if !win_ptr.is_null() {
                    let win = &mut *win_ptr;
                    win.active_tab = cur_sel;
                    for (i, &panel) in win.tab_panels.iter().enumerate() {
                        let show = if i == cur_sel { SW_SHOW } else { SW_HIDE };
                        let _ = ShowWindow(panel, show);
                    }
                }
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut _);
            unsafe {
                SetBkMode(hdc, TRANSPARENT);
                LRESULT(GetSysColorBrush(COLOR_BTNFACE).0 as isize)
            }
        }
        WM_CTLCOLORDLG => unsafe { LRESULT(GetSysColorBrush(COLOR_BTNFACE).0 as isize) },
        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
            let win_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
            if !win_ptr.is_null() {
                let win = &mut *win_ptr;
                handle_settings_command(hwnd, id, code, lparam, win);
            }
            LRESULT(0)
        }
        WM_HSCROLL => {
            let win_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
            if !win_ptr.is_null() {
                let win = &mut *win_ptr;
                handle_trackbar_scroll(HWND(lparam.0 as *mut _), win);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let win_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
            if !win_ptr.is_null() {
                let win = &mut *win_ptr;
                win.cancel_changes();
                win.indicator.toggle_interactivity(false);
            }
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            CURRENT_SETTINGS_HWND.store(0, Ordering::Release);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let win_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
            if !win_ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let _ = Box::from_raw(win_ptr);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn init_settings_controls(hwnd: HWND, win: &mut SettingsWindow) {
    let hinst = HINSTANCE(unsafe { GetModuleHandleW(None).unwrap_or_default() }.0);

    unsafe {
        // Tab Control
        let tab_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            WC_TABCONTROLW,
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            12,
            12,
            600,
            420,
            hwnd,
            HMENU(IDC_TAB_CONTROL as isize as *mut _),
            hinst,
            None,
        )
        .expect("TabControl");

        let tabs = [
            w!("General"),
            w!("Indicator"),
            w!("Sounds"),
            w!("Hotkeys"),
            w!("About"),
        ];

        for (i, &name) in tabs.iter().enumerate() {
            let mut tie = TCITEMW {
                mask: TCIF_TEXT,
                pszText: PWSTR(name.as_ptr() as *mut u16),
                ..Default::default()
            };
            let _ = SendMessageW(
                tab_ctrl,
                TCM_INSERTITEMW,
                WPARAM(i),
                LPARAM(&mut tie as *mut _ as isize),
            );
        }

        // Bottom dialog buttons: OK, Cancel, Apply
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("OK"),
            ws_child_vis_i(BS_DEFPUSHBUTTON),
            330,
            444,
            85,
            28,
            hwnd,
            HMENU(IDC_BTN_OK as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Cancel"),
            ws_child_vis_i(BS_PUSHBUTTON),
            425,
            444,
            85,
            28,
            hwnd,
            HMENU(IDC_BTN_CANCEL as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Apply"),
            ws_child_vis_i(BS_PUSHBUTTON),
            520,
            444,
            85,
            28,
            hwnd,
            HMENU(IDC_BTN_APPLY as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Panels inside tab control
        let px = 20;
        let py = 45;
        let pw = 580;
        let ph = 375;

        let panel_class = w!("EasyMicTabPanel");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(panel_wndproc),
            hInstance: hinst,
            lpszClassName: panel_class,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as isize as *mut _),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        for i in 0..5 {
            let p = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                panel_class,
                PCWSTR::null(),
                WS_CHILD | if i == 0 { WS_VISIBLE } else { WINDOW_STYLE(0) },
                px,
                py,
                pw,
                ph,
                hwnd,
                None,
                hinst,
                None,
            )
            .unwrap();
            win.tab_panels[i] = p;
        }

        build_general_panel(win.tab_panels[0], win, hinst);
        build_indicator_panel(win.tab_panels[1], win, hinst);
        build_sounds_panel(win.tab_panels[2], win, hinst);
        build_hotkeys_panel(win.tab_panels[3], win, hinst);
        build_about_panel(win.tab_panels[4], win, hinst);

        // Apply modern Segoe UI font to all dialog and panel controls
        apply_ui_font_recursive(hwnd, get_ui_normal_font());
        for &panel in win.tab_panels.iter() {
            apply_ui_font_recursive(panel, get_ui_normal_font());
        }
    }
}

unsafe extern "system" fn panel_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND | WM_HSCROLL => {
            // Forward commands to parent settings window
            if let Ok(parent) = GetParent(hwnd) {
                SendMessageW(parent, msg, wparam, lparam)
            } else {
                LRESULT(0)
            }
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut _);
            unsafe {
                SetBkMode(hdc, TRANSPARENT);
                LRESULT(GetSysColorBrush(COLOR_BTNFACE).0 as isize)
            }
        }
        WM_CTLCOLORDLG => unsafe { LRESULT(GetSysColorBrush(COLOR_BTNFACE).0 as isize) },
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn build_general_panel(panel: HWND, win: &SettingsWindow, hinst: HINSTANCE) {
    let cfg = win.config.read();
    unsafe {
        // Autostart checkbox
        let h_auto = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Start EasyMic automatically with Windows"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            20,
            400,
            24,
            panel,
            HMENU(IDC_CHK_AUTOSTART as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        let _ = SendMessageW(
            h_auto,
            BM_SETCHECK,
            WPARAM(if win.autostart_initial { 1 } else { 0 }),
            LPARAM(0),
        );

        // Skip UAC
        let h_uac = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Run with highest privileges (Skip UAC prompt)"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            60,
            400,
            24,
            panel,
            HMENU(IDC_CHK_SKIP_UAC as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        let _ = SendMessageW(
            h_uac,
            BM_SETCHECK,
            WPARAM(if cfg.is_skip_uac_enabled { 1 } else { 0 }),
            LPARAM(0),
        );

        // Automatic updates
        let h_upd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Check for updates on startup"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            100,
            400,
            24,
            panel,
            HMENU(IDC_CHK_UPDATES as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        let _ = SendMessageW(
            h_upd,
            BM_SETCHECK,
            WPARAM(if cfg.is_updates_enabled { 1 } else { 0 }),
            LPARAM(0),
        );
    }
}

fn build_indicator_panel(panel: HWND, win: &SettingsWindow, hinst: HINSTANCE) {
    let cfg = win.config.read();
    unsafe {
        // State combo
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Indicator State:"),
            ws_child_vis_i(0),
            20,
            20,
            150,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_state = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("COMBOBOX"),
            PCWSTR::null(),
            ws_child_vis_i(CBS_DROPDOWNLIST),
            180,
            18,
            200,
            120,
            panel,
            HMENU(IDC_CMB_INDICATOR_STATE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_state,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w!("Hidden").as_ptr() as isize),
        );
        SendMessageW(
            h_state,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w!("Muted").as_ptr() as isize),
        );
        SendMessageW(
            h_state,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w!("Muted or talking").as_ptr() as isize),
        );
        let sel = match cfg.indicator_state {
            IndicatorState::Hidden => 0,
            IndicatorState::Muted => 1,
            IndicatorState::MutedOrTalk => 2,
        };
        SendMessageW(h_state, CB_SETCURSEL, WPARAM(sel), LPARAM(0));

        // Size slider (10 to 32)
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Indicator Size:"),
            ws_child_vis_i(0),
            20,
            60,
            150,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_sz = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            TRACKBAR_CLASSW,
            PCWSTR::null(),
            ws_child_vis_u(TBS_AUTOTICKS),
            180,
            58,
            200,
            30,
            panel,
            HMENU(IDC_TRK_INDICATOR_SIZE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(h_sz, TBM_SETRANGEMIN, WPARAM(0), LPARAM(10));
        SendMessageW(h_sz, TBM_SETRANGEMAX, WPARAM(0), LPARAM(32));
        SendMessageW(
            h_sz,
            TBM_SETPOS,
            WPARAM(1),
            LPARAM(cfg.indicator_size as isize),
        );
        let sz_txt: Vec<u16> = format!("{} px", cfg.indicator_size)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(sz_txt.as_ptr()),
            ws_child_vis_i(0),
            390,
            60,
            60,
            20,
            panel,
            HMENU(IDC_LBL_INDICATOR_SIZE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Talking threshold slider (0 to 100%)
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Voice Threshold:"),
            ws_child_vis_i(0),
            20,
            100,
            150,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_th = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            TRACKBAR_CLASSW,
            PCWSTR::null(),
            ws_child_vis_u(TBS_AUTOTICKS),
            180,
            98,
            200,
            30,
            panel,
            HMENU(IDC_TRK_INDICATOR_THRESH as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(h_th, TBM_SETRANGEMIN, WPARAM(0), LPARAM(0));
        SendMessageW(h_th, TBM_SETRANGEMAX, WPARAM(0), LPARAM(100));
        let thresh_val = (cfg.indicator_volume_threshold * 100.0).round() as isize;
        SendMessageW(h_th, TBM_SETPOS, WPARAM(1), LPARAM(thresh_val));
        let th_txt: Vec<u16> = format!("{} %", thresh_val)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(th_txt.as_ptr()),
            ws_child_vis_i(0),
            390,
            100,
            60,
            20,
            panel,
            HMENU(IDC_LBL_INDICATOR_THRESH as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Checkboxes
        let h_cap = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Exclude indicator from screen capture (OBS, Discord)"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            140,
            450,
            24,
            panel,
            HMENU(IDC_CHK_CAPTURE_EXCLUDE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_cap,
            BM_SETCHECK,
            WPARAM(if cfg.exclude_from_capture { 1 } else { 0 }),
            LPARAM(0),
        );

        let h_top = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Always keep on top"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            175,
            450,
            24,
            panel,
            HMENU(IDC_CHK_ON_TOP as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_top,
            BM_SETCHECK,
            WPARAM(if cfg.on_top_exclusive { 1 } else { 0 }),
            LPARAM(0),
        );

        let h_hid = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Hide indicator when inactive"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            210,
            450,
            24,
            panel,
            HMENU(IDC_CHK_HIDE_INACTIVE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_hid,
            BM_SETCHECK,
            WPARAM(if cfg.hide_when_inactive { 1 } else { 0 }),
            LPARAM(0),
        );

        let h_not = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Show text overlay notifications on action"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            245,
            450,
            24,
            panel,
            HMENU(IDC_CHK_NOTIFICATIONS as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_not,
            BM_SETCHECK,
            WPARAM(if cfg.notifications_enabled { 1 } else { 0 }),
            LPARAM(0),
        );

        // Drag tip & Reset position button
        CreateWindowExW(WINDOW_EX_STYLE::default(), w!("STATIC"), w!("Tip: While this Settings window is open, you can drag the on-screen pill indicator directly with your mouse to position it!"), ws_child_vis_i(0), 20, 285, 520, 36, panel, None, hinst, None).unwrap();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Reset to Center"),
            ws_child_vis_i(BS_PUSHBUTTON),
            20,
            330,
            140,
            28,
            panel,
            HMENU(IDC_BTN_RESET_POS as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
    }
}

fn build_sounds_panel(panel: HWND, win: &SettingsWindow, hinst: HINSTANCE) {
    let cfg = win.config.read();
    unsafe {
        // Mic volume slider
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Microphone Volume:"),
            ws_child_vis_i(0),
            20,
            20,
            150,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_mic = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            TRACKBAR_CLASSW,
            PCWSTR::null(),
            ws_child_vis_u(TBS_AUTOTICKS),
            180,
            18,
            200,
            30,
            panel,
            HMENU(IDC_TRK_MIC_VOLUME as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(h_mic, TBM_SETRANGEMIN, WPARAM(0), LPARAM(0));
        SendMessageW(h_mic, TBM_SETRANGEMAX, WPARAM(0), LPARAM(100));
        let mic_vol = if cfg.mic_volume >= 0 {
            cfg.mic_volume as isize
        } else {
            win.state.volume_percent() as isize
        };
        SendMessageW(h_mic, TBM_SETPOS, WPARAM(1), LPARAM(mic_vol));
        let mic_txt: Vec<u16> = format!("{} %", mic_vol)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(mic_txt.as_ptr()),
            ws_child_vis_i(0),
            390,
            20,
            60,
            20,
            panel,
            HMENU(IDC_LBL_MIC_VOLUME as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Keep volume
        let h_keep = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Lock and keep microphone volume level"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            60,
            400,
            24,
            panel,
            HMENU(IDC_CHK_KEEP_VOLUME as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_keep,
            BM_SETCHECK,
            WPARAM(if cfg.is_mic_keep_volume { 1 } else { 0 }),
            LPARAM(0),
        );

        // Bell volume slider
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Sound Cues Volume:"),
            ws_child_vis_i(0),
            20,
            100,
            150,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_bell = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            TRACKBAR_CLASSW,
            PCWSTR::null(),
            ws_child_vis_u(TBS_AUTOTICKS),
            180,
            98,
            200,
            30,
            panel,
            HMENU(IDC_TRK_BELL_VOLUME as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(h_bell, TBM_SETRANGEMIN, WPARAM(0), LPARAM(0));
        SendMessageW(h_bell, TBM_SETRANGEMAX, WPARAM(0), LPARAM(100));
        SendMessageW(
            h_bell,
            TBM_SETPOS,
            WPARAM(1),
            LPARAM(cfg.bell_volume as isize),
        );
        let bell_txt: Vec<u16> = format!("{} %", cfg.bell_volume)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(bell_txt.as_ptr()),
            ws_child_vis_i(0),
            390,
            100,
            60,
            20,
            panel,
            HMENU(IDC_LBL_BELL_VOLUME as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Mute cue sound
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Mute Sound Cue:"),
            ws_child_vis_i(0),
            20,
            140,
            145,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_mute_cmb = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("COMBOBOX"),
            PCWSTR::null(),
            ws_child_vis_i(CBS_DROPDOWNLIST),
            170,
            138,
            190,
            150,
            panel,
            HMENU(IDC_CMB_MUTE_SOUND as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        populate_sound_combo(h_mute_cmb, &cfg.mute_sound_source, Some(&cfg.recent_sounds));
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Test"),
            ws_child_vis_i(BS_PUSHBUTTON),
            370,
            136,
            60,
            26,
            panel,
            HMENU(IDC_BTN_TEST_MUTE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Browse..."),
            ws_child_vis_i(BS_PUSHBUTTON),
            440,
            136,
            85,
            26,
            panel,
            HMENU(IDC_BTN_BROWSE_MUTE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Unmute cue sound
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Unmute Sound Cue:"),
            ws_child_vis_i(0),
            20,
            185,
            145,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_unmute_cmb = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("COMBOBOX"),
            PCWSTR::null(),
            ws_child_vis_i(CBS_DROPDOWNLIST),
            170,
            183,
            190,
            150,
            panel,
            HMENU(IDC_CMB_UNMUTE_SOUND as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        populate_sound_combo(
            h_unmute_cmb,
            &cfg.unmute_sound_source,
            Some(&cfg.recent_sounds),
        );
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Test"),
            ws_child_vis_i(BS_PUSHBUTTON),
            370,
            181,
            60,
            26,
            panel,
            HMENU(IDC_BTN_TEST_UNMUTE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Browse..."),
            ws_child_vis_i(BS_PUSHBUTTON),
            440,
            181,
            85,
            26,
            panel,
            HMENU(IDC_BTN_BROWSE_UNMUTE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
    }
}

pub fn get_combo_selected_text(h_cmb: HWND) -> String {
    unsafe {
        let sel = SendMessageW(h_cmb, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if sel < 0 {
            return String::new();
        }
        let len = SendMessageW(h_cmb, CB_GETLBTEXTLEN, WPARAM(sel as usize), LPARAM(0)).0;
        if len <= 0 {
            return String::new();
        }
        let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
        let res = SendMessageW(
            h_cmb,
            CB_GETLBTEXT,
            WPARAM(sel as usize),
            LPARAM(buf.as_mut_ptr() as isize),
        )
        .0;
        if res < 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

pub fn select_or_add_combo_item(h_cmb: HWND, text: &str) {
    unsafe {
        let count = SendMessageW(h_cmb, CB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
        for i in 0..count {
            let len = SendMessageW(h_cmb, CB_GETLBTEXTLEN, WPARAM(i as usize), LPARAM(0)).0;
            if len > 0 {
                let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
                let _ = SendMessageW(
                    h_cmb,
                    CB_GETLBTEXT,
                    WPARAM(i as usize),
                    LPARAM(buf.as_mut_ptr() as isize),
                );
                let item = String::from_utf16_lossy(&buf[..len as usize]);
                if item.eq_ignore_ascii_case(text) {
                    let _ = SendMessageW(h_cmb, CB_SETCURSEL, WPARAM(i as usize), LPARAM(0));
                    return;
                }
            }
        }
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let idx = SendMessageW(
            h_cmb,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        )
        .0;
        if idx >= 0 {
            let _ = SendMessageW(h_cmb, CB_SETCURSEL, WPARAM(idx as usize), LPARAM(0));
        }
    }
}

pub fn prompt_browse_sound(parent: HWND) -> Option<String> {
    let mut file_buf = [0u16; 512];
    let filter: Vec<u16> = "Audio Files (*.wav;*.mp3)\0*.wav;*.mp3\0All Files (*.*)\0*.*\0\0"
        .encode_utf16()
        .collect();

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: parent,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file_buf.as_mut_ptr()),
        nMaxFile: file_buf.len() as u32,
        lpstrTitle: w!("Select Sound Cue"),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        ..Default::default()
    };

    let success = unsafe { GetOpenFileNameW(&mut ofn).as_bool() };
    if success {
        let nul_pos = file_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(file_buf.len());
        let selected = String::from_utf16_lossy(&file_buf[..nul_pos]);
        let trimmed = selected.trim();
        if !trimmed.is_empty() {
            let path = std::path::Path::new(trimmed);
            let exe_dir = AppConfig::get_exe_dir();
            let sounds_dir = exe_dir.join("sounds");

            if let Ok(rel) = path.strip_prefix(&sounds_dir) {
                return Some(rel.to_string_lossy().to_string());
            } else if let Ok(rel) = path.strip_prefix(&exe_dir) {
                return Some(rel.to_string_lossy().to_string());
            }
            return Some(trimmed.to_string());
        }
    }
    None
}

fn populate_sound_combo(cmb: HWND, current: &str, recent: Option<&BTreeSet<String>>) {
    let mut options = vec![
        "off.wav".to_string(),
        "on.wav".to_string(),
        "Mute".to_string(),
        "Unmute".to_string(),
        "Tick".to_string(),
        "Enable".to_string(),
        "Disable".to_string(),
        "None".to_string(),
    ];

    // Discover custom sound files in sounds/ subfolder beside executable
    let exe_dir = AppConfig::get_exe_dir();
    let sounds_dir = exe_dir.join("sounds");
    if sounds_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&sounds_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext.eq_ignore_ascii_case("wav") || ext.eq_ignore_ascii_case("mp3") {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            if !options.iter().any(|o| o.eq_ignore_ascii_case(name)) {
                                options.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Add recent sounds
    if let Some(recent_set) = recent {
        for s in recent_set {
            if !s.is_empty() && !options.iter().any(|o| o.eq_ignore_ascii_case(s)) {
                options.push(s.clone());
            }
        }
    }

    // Add current selection if not present
    let curr_trimmed = current.trim();
    if !curr_trimmed.is_empty() && !options.iter().any(|o| o.eq_ignore_ascii_case(curr_trimmed)) {
        options.push(curr_trimmed.to_string());
    }

    unsafe {
        let _ = SendMessageW(cmb, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        let mut selected = 0;
        for (i, opt) in options.iter().enumerate() {
            let wide: Vec<u16> = opt.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SendMessageW(cmb, CB_ADDSTRING, WPARAM(0), LPARAM(wide.as_ptr() as isize));
            if curr_trimmed.eq_ignore_ascii_case(opt)
                || (curr_trimmed.to_ascii_lowercase().ends_with("off.wav") && opt == "off.wav")
                || (curr_trimmed.to_ascii_lowercase().ends_with("on.wav") && opt == "on.wav")
            {
                selected = i;
            }
        }
        let _ = SendMessageW(cmb, CB_SETCURSEL, WPARAM(selected), LPARAM(0));
    }
}

fn build_hotkeys_panel(panel: HWND, win: &SettingsWindow, hinst: HINSTANCE) {
    let cfg = win.config.read();
    unsafe {
        // Multi-press window
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Multi-press Window:"),
            ws_child_vis_i(0),
            20,
            15,
            150,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_mp = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            TRACKBAR_CLASSW,
            PCWSTR::null(),
            ws_child_vis_u(TBS_AUTOTICKS),
            180,
            12,
            200,
            30,
            panel,
            HMENU(IDC_TRK_MULTIPRESS as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(h_mp, TBM_SETRANGEMIN, WPARAM(0), LPARAM(100));
        SendMessageW(h_mp, TBM_SETRANGEMAX, WPARAM(0), LPARAM(600));
        SendMessageW(
            h_mp,
            TBM_SETPOS,
            WPARAM(1),
            LPARAM(cfg.multi_press_window_ms as isize),
        );
        let mp_txt: Vec<u16> = format!("{} ms", cfg.multi_press_window_ms)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(mp_txt.as_ptr()),
            ws_child_vis_i(0),
            390,
            15,
            70,
            20,
            panel,
            HMENU(IDC_LBL_MULTIPRESS as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Actions list
        let h_list = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("LISTBOX"),
            PCWSTR::null(),
            WINDOW_STYLE(
                WS_CHILD.0
                    | WS_VISIBLE.0
                    | WS_VSCROLL.0
                    | (LBS_NOTIFY as u32)
                    | (LBS_HASSTRINGS as u32),
            ),
            20,
            55,
            535,
            245,
            panel,
            HMENU(IDC_LST_ACTIONS as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        populate_actions_list(h_list, &cfg.actions);

        // Buttons
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("+ Add Action"),
            ws_child_vis_i(BS_PUSHBUTTON),
            20,
            315,
            110,
            28,
            panel,
            HMENU(IDC_BTN_ADD_ACTION as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Edit Action"),
            ws_child_vis_i(BS_PUSHBUTTON),
            140,
            315,
            110,
            28,
            panel,
            HMENU(IDC_BTN_EDIT_ACTION as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Delete Action"),
            ws_child_vis_i(BS_PUSHBUTTON),
            260,
            315,
            110,
            28,
            panel,
            HMENU(IDC_BTN_DELETE_ACTION as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
    }
}

fn populate_actions_list(h_list: HWND, actions: &[ActionConfig]) {
    unsafe {
        let _ = SendMessageW(h_list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for act in actions {
            let hk_name = if act.hotkey != 0 {
                get_hotkey_name(act.hotkey)
            } else {
                "None".to_string()
            };
            let extra = if act.presses > 1 {
                format!(" (x{})", act.presses)
            } else {
                "".to_string()
            };
            let rel = if act.on_release { " [Release]" } else { "" };
            let line = format!(
                "{}  —  Hotkey: {}{}{}  ({})",
                act.name,
                hk_name,
                extra,
                rel,
                if act.built_in.is_empty() {
                    "Command"
                } else {
                    &act.built_in
                }
            );
            let wide: Vec<u16> = line.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SendMessageW(
                h_list,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(wide.as_ptr() as isize),
            );
        }
        if !actions.is_empty() {
            let _ = SendMessageW(h_list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
        }
    }
}

fn build_about_panel(panel: HWND, win: &SettingsWindow, hinst: HINSTANCE) {
    let cfg = win.config.read();
    unsafe {
        // App Title
        let h_title = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("EasyMic"),
            ws_child_vis_i(0),
            25,
            20,
            300,
            28,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let _ = SendMessageW(
            h_title,
            WM_SETFONT,
            WPARAM(get_ui_title_font().0 as usize),
            LPARAM(1),
        );

        // Subtitle / Version
        let h_sub = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Version 1.3.0 (Native Rust Edition) • High-Performance Audio Manager"),
            ws_child_vis_i(0),
            25,
            50,
            520,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();
        let _ = SendMessageW(
            h_sub,
            WM_SETFONT,
            WPARAM(get_ui_bold_font().0 as usize),
            LPARAM(1),
        );

        // Description
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Engineered with zero-overhead Win32 input hooks, sub-millisecond hotkey response,\nand direct hardware COM audio endpoint synchronization."),
            ws_child_vis_i(0),
            25,
            75,
            520,
            36,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();

        // Active Configuration Group Box
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!(" Configuration & Status "),
            ws_child_vis_i(BS_GROUPBOX),
            20,
            125,
            540,
            165,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();

        // Config file location text
        let cfg_path = AppConfig::get_config_path();
        let cfg_display = format!("Config File:  {}", cfg_path.to_string_lossy());
        let cfg_wide: Vec<u16> = cfg_display
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(cfg_wide.as_ptr()),
            ws_child_vis_i(0),
            35,
            150,
            510,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();

        // Primary hotkey status
        let primary_hk = cfg
            .actions
            .iter()
            .find(|a| a.built_in == "Toggle mute")
            .map(|a| {
                if a.hotkey != 0 {
                    get_hotkey_name(a.hotkey)
                } else {
                    "Shift + Z (Default)".to_string()
                }
            })
            .unwrap_or_else(|| "Shift + Z (Default)".to_string());
        let hk_display = format!("Mute Toggle Hotkey:  {}", primary_hk);
        let hk_wide: Vec<u16> = hk_display
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(hk_wide.as_ptr()),
            ws_child_vis_i(0),
            35,
            175,
            510,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();

        // Audio state
        let is_muted = win.state.is_muted();
        let mic_status = if is_muted { "Muted" } else { "Unmuted (Live)" };
        let status_display = format!("Current Mic Status:  {}", mic_status);
        let status_wide: Vec<u16> = status_display
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(status_wide.as_ptr()),
            ws_child_vis_i(0),
            35,
            200,
            510,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();

        // Mode info
        let mode_display =
            "Input Subsystem:  Dedicated O(1) Hook Thread • Memory-mapped Portable Mode"
                .to_string();
        let mode_wide: Vec<u16> = mode_display
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(mode_wide.as_ptr()),
            ws_child_vis_i(0),
            35,
            225,
            510,
            20,
            panel,
            None,
            hinst,
            None,
        )
        .unwrap();

        // Action Buttons: Open config.ini, Open Folder, GitHub Repo
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Open config.ini"),
            ws_child_vis_i(BS_PUSHBUTTON),
            35,
            252,
            125,
            26,
            panel,
            HMENU(IDC_BTN_OPEN_CONFIG as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Open App Folder"),
            ws_child_vis_i(BS_PUSHBUTTON),
            165,
            252,
            130,
            26,
            panel,
            HMENU(IDC_BTN_OPEN_FOLDER as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("GitHub Repository"),
            ws_child_vis_i(BS_PUSHBUTTON),
            305,
            252,
            140,
            26,
            panel,
            HMENU(IDC_BTN_GITHUB as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
    }
}

fn handle_settings_command(
    hwnd: HWND,
    id: usize,
    _code: u16,
    _lparam: LPARAM,
    win: &mut SettingsWindow,
) {
    match id {
        IDC_BTN_OK => {
            win.apply_changes();
            win.indicator.toggle_interactivity(false);
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
        IDC_BTN_APPLY => {
            win.apply_changes();
        }
        IDC_BTN_CANCEL => {
            win.cancel_changes();
            win.indicator.toggle_interactivity(false);
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
        IDC_CHK_AUTOSTART => {
            win.autostart_pending = !win.autostart_pending;
        }
        IDC_CHK_SKIP_UAC => {
            let mut cfg = win.config.write();
            cfg.is_skip_uac_enabled = !cfg.is_skip_uac_enabled;
        }
        IDC_CHK_UPDATES => {
            let mut cfg = win.config.write();
            cfg.is_updates_enabled = !cfg.is_updates_enabled;
        }
        IDC_CMB_INDICATOR_STATE => {
            let h_cmb = unsafe { get_ctrl(win.tab_panels[1], IDC_CMB_INDICATOR_STATE) };
            let sel = unsafe { SendMessageW(h_cmb, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
            let mut cfg = win.config.write();
            cfg.indicator_state = match sel {
                0 => IndicatorState::Hidden,
                1 => IndicatorState::Muted,
                _ => IndicatorState::MutedOrTalk,
            };
            win.indicator.render();
        }
        IDC_CHK_CAPTURE_EXCLUDE => {
            let mut cfg = win.config.write();
            cfg.exclude_from_capture = !cfg.exclude_from_capture;
            win.indicator.render();
        }
        IDC_CHK_ON_TOP => {
            let mut cfg = win.config.write();
            cfg.on_top_exclusive = !cfg.on_top_exclusive;
            win.indicator.render();
        }
        IDC_CHK_HIDE_INACTIVE => {
            let mut cfg = win.config.write();
            cfg.hide_when_inactive = !cfg.hide_when_inactive;
            win.indicator.render();
        }
        IDC_CHK_NOTIFICATIONS => {
            let mut cfg = win.config.write();
            cfg.notifications_enabled = !cfg.notifications_enabled;
        }
        IDC_BTN_RESET_POS => {
            let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
            let x = (screen_w - 128) / 2;
            let y = screen_h - 120;
            win.indicator.update_position(x, y);
            win.indicator.render();
        }
        IDC_CHK_KEEP_VOLUME => {
            let mut cfg = win.config.write();
            cfg.is_mic_keep_volume = !cfg.is_mic_keep_volume;
        }
        IDC_BTN_TEST_MUTE => {
            let h_cmb = unsafe { get_ctrl(win.tab_panels[2], IDC_CMB_MUTE_SOUND) };
            let sel = get_combo_selected_text(h_cmb);
            let cue = if !sel.is_empty() {
                sel
            } else {
                win.config.read().mute_sound_source.clone()
            };
            win.sound.play(&cue, 100);
        }
        IDC_BTN_TEST_UNMUTE => {
            let h_cmb = unsafe { get_ctrl(win.tab_panels[2], IDC_CMB_UNMUTE_SOUND) };
            let sel = get_combo_selected_text(h_cmb);
            let cue = if !sel.is_empty() {
                sel
            } else {
                win.config.read().unmute_sound_source.clone()
            };
            win.sound.play(&cue, 100);
        }
        IDC_BTN_BROWSE_MUTE => {
            if let Some(path) = prompt_browse_sound(hwnd) {
                let h_cmb = unsafe { get_ctrl(win.tab_panels[2], IDC_CMB_MUTE_SOUND) };
                select_or_add_combo_item(h_cmb, &path);
                let mut cfg = win.config.write();
                cfg.mute_sound_source = path.clone();
                cfg.recent_sounds.insert(path.clone());
                drop(cfg);
                win.sound.play(&path, 100);
            }
        }
        IDC_BTN_BROWSE_UNMUTE => {
            if let Some(path) = prompt_browse_sound(hwnd) {
                let h_cmb = unsafe { get_ctrl(win.tab_panels[2], IDC_CMB_UNMUTE_SOUND) };
                select_or_add_combo_item(h_cmb, &path);
                let mut cfg = win.config.write();
                cfg.unmute_sound_source = path.clone();
                cfg.recent_sounds.insert(path.clone());
                drop(cfg);
                win.sound.play(&path, 100);
            }
        }
        IDC_CMB_MUTE_SOUND => {
            let h_cmb = unsafe { get_ctrl(win.tab_panels[2], IDC_CMB_MUTE_SOUND) };
            let text = get_combo_selected_text(h_cmb);
            if !text.is_empty() {
                let mut cfg = win.config.write();
                cfg.mute_sound_source = text.clone();
                cfg.recent_sounds.insert(text);
            }
        }
        IDC_CMB_UNMUTE_SOUND => {
            let h_cmb = unsafe { get_ctrl(win.tab_panels[2], IDC_CMB_UNMUTE_SOUND) };
            let text = get_combo_selected_text(h_cmb);
            if !text.is_empty() {
                let mut cfg = win.config.write();
                cfg.unmute_sound_source = text.clone();
                cfg.recent_sounds.insert(text);
            }
        }
        IDC_BTN_ADD_ACTION => {
            open_action_editor(hwnd, None, win);
        }
        IDC_BTN_EDIT_ACTION => {
            let h_list = unsafe { get_ctrl(win.tab_panels[3], IDC_LST_ACTIONS) };
            let sel = unsafe { SendMessageW(h_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
            if sel >= 0 {
                open_action_editor(hwnd, Some(sel as usize), win);
            }
        }
        IDC_BTN_DELETE_ACTION => {
            let h_list = unsafe { get_ctrl(win.tab_panels[3], IDC_LST_ACTIONS) };
            let sel = unsafe { SendMessageW(h_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
            if sel >= 0 {
                let mut cfg = win.config.write();
                if (sel as usize) < cfg.actions.len() {
                    cfg.actions.remove(sel as usize);
                    populate_actions_list(h_list, &cfg.actions);
                }
            }
        }
        IDC_BTN_OPEN_CONFIG => {
            let path = AppConfig::get_config_path();
            let path_str = path.to_string_lossy().to_string();
            let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                let _ = ShellExecuteW(
                    None,
                    w!("open"),
                    PCWSTR(path_wide.as_ptr()),
                    None,
                    None,
                    SW_SHOWNORMAL,
                );
            }
        }
        IDC_BTN_OPEN_FOLDER => {
            let dir = AppConfig::get_exe_dir();
            let dir_str = dir.to_string_lossy().to_string();
            let dir_wide: Vec<u16> = dir_str.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                let _ = ShellExecuteW(
                    None,
                    w!("open"),
                    PCWSTR(dir_wide.as_ptr()),
                    None,
                    None,
                    SW_SHOWNORMAL,
                );
            }
        }
        IDC_BTN_GITHUB => unsafe {
            let _ = ShellExecuteW(None, w!("open"), REPO_URL_W, None, None, SW_SHOWNORMAL);
        },
        _ => {}
    }
}

fn handle_trackbar_scroll(h_trk: HWND, win: &mut SettingsWindow) {
    let id = unsafe { GetWindowLongPtrW(h_trk, GWLP_ID) as usize };
    let pos = unsafe { SendMessageW(h_trk, TBM_GETPOS, WPARAM(0), LPARAM(0)).0 as i32 };

    match id {
        IDC_TRK_INDICATOR_SIZE => {
            win.config.write().indicator_size = pos as u8;
            let txt: Vec<u16> = format!("{} px", pos)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let h_lbl = unsafe { get_ctrl(win.tab_panels[1], IDC_LBL_INDICATOR_SIZE) };
            unsafe {
                let _ = SetWindowTextW(h_lbl, PCWSTR(txt.as_ptr()));
            }
            win.indicator.render();
        }
        IDC_TRK_INDICATOR_THRESH => {
            win.config.write().indicator_volume_threshold = pos as f32 / 100.0;
            let txt: Vec<u16> = format!("{} %", pos)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let h_lbl = unsafe { get_ctrl(win.tab_panels[1], IDC_LBL_INDICATOR_THRESH) };
            unsafe {
                let _ = SetWindowTextW(h_lbl, PCWSTR(txt.as_ptr()));
            }
        }
        IDC_TRK_MIC_VOLUME => {
            win.config.write().mic_volume = pos as i8;
            let txt: Vec<u16> = format!("{} %", pos)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let h_lbl = unsafe { get_ctrl(win.tab_panels[2], IDC_LBL_MIC_VOLUME) };
            unsafe {
                let _ = SetWindowTextW(h_lbl, PCWSTR(txt.as_ptr()));
            }
            let _ = win.audio.set_volume_percent(pos as u8);
        }
        IDC_TRK_BELL_VOLUME => {
            win.config.write().bell_volume = pos as i8;
            let txt: Vec<u16> = format!("{} %", pos)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let h_lbl = unsafe { get_ctrl(win.tab_panels[2], IDC_LBL_BELL_VOLUME) };
            unsafe {
                let _ = SetWindowTextW(h_lbl, PCWSTR(txt.as_ptr()));
            }
        }
        IDC_TRK_MULTIPRESS => {
            win.config.write().multi_press_window_ms = pos as u16;
            let txt: Vec<u16> = format!("{} ms", pos)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let h_lbl = unsafe { get_ctrl(win.tab_panels[3], IDC_LBL_MULTIPRESS) };
            unsafe {
                let _ = SetWindowTextW(h_lbl, PCWSTR(txt.as_ptr()));
            }
        }
        _ => {}
    }
}

// -------------------------------------------------------------------------
// Action Editor Dialog
// -------------------------------------------------------------------------

struct ActionEditorState {
    parent_settings: HWND,
    action_index: Option<usize>,
    action_config: ActionConfig,
    capture_session: Arc<HotkeyCaptureSession>,
    config: Arc<RwLock<AppConfig>>,
    sound: Arc<SoundManager>,
}

fn open_action_editor(parent: HWND, index: Option<usize>, win: &SettingsWindow) {
    let action_cfg = match index {
        Some(idx) => win
            .config
            .read()
            .actions
            .get(idx)
            .cloned()
            .unwrap_or_default(),
        None => ActionConfig {
            name: "New action".to_string(),
            built_in: "Toggle mute".to_string(),
            notification: "Mic {mic}".to_string(),
            sound_volume: 100,
            presses: 1,
            ..Default::default()
        },
    };

    let state = Box::new(ActionEditorState {
        parent_settings: parent,
        action_index: index,
        action_config: action_cfg,
        capture_session: win.capture_session.clone(),
        config: win.config.clone(),
        sound: win.sound.clone(),
    });

    let hinst = HINSTANCE(unsafe { GetModuleHandleW(None).unwrap_or_default() }.0);
    let state_ptr = Box::into_raw(state);

    unsafe {
        let class_name = w!("EasyMicActionEditor");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(action_editor_wndproc),
            hInstance: hinst,
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as isize as *mut _),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class_name,
            w!("Edit Action"),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            490,
            parent,
            None,
            hinst,
            Some(state_ptr as *mut _),
        )
        .expect("ActionEditor");

        let mut rc = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rc);
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - (rc.right - rc.left)) / 2;
        let y = (screen_h - (rc.bottom - rc.top)) / 2;
        let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
    }
}

unsafe extern "system" fn action_editor_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = lparam.0 as *const CREATESTRUCTW;
            if !cs.is_null() && !(*cs).lpCreateParams.is_null() {
                let state_ptr = (*cs).lpCreateParams as *mut ActionEditorState;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
                init_action_editor_controls(hwnd, &mut *state_ptr);
            }
            LRESULT(0)
        }
        WM_HOTKEY_CAPTURED => {
            let mask = (wparam.0 as u64) | ((lparam.0 as u64) << 32);
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ActionEditorState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                state.action_config.hotkey = mask;
                let hk_name = if mask != 0 {
                    get_hotkey_name(mask)
                } else {
                    "None".to_string()
                };
                let wide: Vec<u16> = hk_name.encode_utf16().chain(std::iter::once(0)).collect();
                let h_txt = get_ctrl(hwnd, IDC_ACT_HOTKEY_TXT);
                let _ = SetWindowTextW(h_txt, PCWSTR(wide.as_ptr()));
                let h_btn = get_ctrl(hwnd, IDC_ACT_BTN_CAPTURE);
                let _ = SetWindowTextW(h_btn, w!("Capture Hotkey"));
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut _);
            unsafe {
                SetBkMode(hdc, TRANSPARENT);
                LRESULT(GetSysColorBrush(COLOR_BTNFACE).0 as isize)
            }
        }
        WM_CTLCOLORDLG => unsafe { LRESULT(GetSysColorBrush(COLOR_BTNFACE).0 as isize) },
        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ActionEditorState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                handle_action_editor_command(hwnd, id, state);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ActionEditorState;
            if !state_ptr.is_null() {
                (*state_ptr).capture_session.cancel();
            }
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        WM_NCDESTROY => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ActionEditorState;
            if !state_ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let _ = Box::from_raw(state_ptr);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn init_action_editor_controls(hwnd: HWND, state: &mut ActionEditorState) {
    let hinst = HINSTANCE(unsafe { GetModuleHandleW(None).unwrap_or_default() }.0);
    let act = &state.action_config;

    unsafe {
        // Name
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Action Name:"),
            ws_child_vis_i(0),
            20,
            20,
            110,
            20,
            hwnd,
            None,
            hinst,
            None,
        )
        .unwrap();
        let name_wide: Vec<u16> = act.name.encode_utf16().chain(std::iter::once(0)).collect();
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(name_wide.as_ptr()),
            ws_child_vis_i(ES_AUTOHSCROLL),
            140,
            18,
            300,
            24,
            hwnd,
            HMENU(IDC_ACT_NAME as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Type
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Action Type:"),
            ws_child_vis_i(0),
            20,
            55,
            110,
            20,
            hwnd,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_type = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("COMBOBOX"),
            PCWSTR::null(),
            ws_child_vis_i(CBS_DROPDOWNLIST),
            140,
            53,
            300,
            160,
            hwnd,
            HMENU(IDC_ACT_TYPE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        for b in ALL_BUILT_IN_ACTIONS {
            let wide: Vec<u16> = b.title.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SendMessageW(
                h_type,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(wide.as_ptr() as isize),
            );
        }
        let _ = SendMessageW(
            h_type,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w!("Custom command").as_ptr() as isize),
        );
        let sel = if act.built_in.is_empty() {
            ALL_BUILT_IN_ACTIONS.len()
        } else {
            ALL_BUILT_IN_ACTIONS
                .iter()
                .position(|b| b.key == act.built_in)
                .unwrap_or(0)
        };
        let _ = SendMessageW(h_type, CB_SETCURSEL, WPARAM(sel), LPARAM(0));

        // Hotkey & Capture
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Hotkey:"),
            ws_child_vis_i(0),
            20,
            95,
            110,
            20,
            hwnd,
            None,
            hinst,
            None,
        )
        .unwrap();
        let hk_str = if act.hotkey != 0 {
            get_hotkey_name(act.hotkey)
        } else {
            "None".to_string()
        };
        let hk_wide: Vec<u16> = hk_str.encode_utf16().chain(std::iter::once(0)).collect();
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(hk_wide.as_ptr()),
            ws_child_vis_i(ES_READONLY),
            140,
            93,
            150,
            24,
            hwnd,
            HMENU(IDC_ACT_HOTKEY_TXT as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Capture Hotkey"),
            ws_child_vis_i(BS_PUSHBUTTON),
            300,
            92,
            100,
            26,
            hwnd,
            HMENU(IDC_ACT_BTN_CAPTURE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Clear"),
            ws_child_vis_i(BS_PUSHBUTTON),
            405,
            92,
            45,
            26,
            hwnd,
            HMENU(IDC_ACT_BTN_CLEAR_HOTKEY as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Presses
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Presses:"),
            ws_child_vis_i(0),
            20,
            135,
            110,
            20,
            hwnd,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_prs = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("COMBOBOX"),
            PCWSTR::null(),
            ws_child_vis_i(CBS_DROPDOWNLIST),
            140,
            133,
            100,
            100,
            hwnd,
            HMENU(IDC_ACT_PRESSES as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_prs,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w!("1x (Single)").as_ptr() as isize),
        );
        SendMessageW(
            h_prs,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w!("2x (Double)").as_ptr() as isize),
        );
        SendMessageW(
            h_prs,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w!("3x (Triple)").as_ptr() as isize),
        );
        let prs_sel = (act.presses.clamp(1, 3) - 1) as usize;
        SendMessageW(h_prs, CB_SETCURSEL, WPARAM(prs_sel), LPARAM(0));

        // Options Checkboxes
        let h_rel = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Trigger on release"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            175,
            180,
            22,
            hwnd,
            HMENU(IDC_ACT_CHK_RELEASE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_rel,
            BM_SETCHECK,
            WPARAM(if act.on_release { 1 } else { 0 }),
            LPARAM(0),
        );

        let h_blk = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Block key (suppress)"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            210,
            175,
            180,
            22,
            hwnd,
            HMENU(IDC_ACT_CHK_BLOCK as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_blk,
            BM_SETCHECK,
            WPARAM(if act.block { 1 } else { 0 }),
            LPARAM(0),
        );

        let h_tap = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Tap only"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            390,
            175,
            100,
            22,
            hwnd,
            HMENU(IDC_ACT_CHK_TAP as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_tap,
            BM_SETCHECK,
            WPARAM(if act.tap_only { 1 } else { 0 }),
            LPARAM(0),
        );

        // Sound Feedback
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Sound Cue:"),
            ws_child_vis_i(0),
            20,
            215,
            110,
            20,
            hwnd,
            None,
            hinst,
            None,
        )
        .unwrap();
        let h_snd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("COMBOBOX"),
            PCWSTR::null(),
            ws_child_vis_i(CBS_DROPDOWNLIST),
            140,
            213,
            160,
            150,
            hwnd,
            HMENU(IDC_ACT_CMB_SOUND as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        populate_sound_combo(h_snd, &act.sound, Some(&state.config.read().recent_sounds));

        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Test"),
            ws_child_vis_i(BS_PUSHBUTTON),
            310,
            211,
            55,
            26,
            hwnd,
            HMENU(IDC_ACT_BTN_TEST_SOUND as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Browse..."),
            ws_child_vis_i(BS_PUSHBUTTON),
            370,
            211,
            70,
            26,
            hwnd,
            HMENU(IDC_ACT_BTN_BROWSE_SOUND as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Notification template
        let h_ntf_chk = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Show overlay notification:"),
            ws_child_vis_i(BS_AUTOCHECKBOX),
            20,
            255,
            200,
            22,
            hwnd,
            HMENU(IDC_ACT_CHK_NOTIF as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        SendMessageW(
            h_ntf_chk,
            BM_SETCHECK,
            WPARAM(if act.show_notification { 1 } else { 0 }),
            LPARAM(0),
        );
        let notif_wide: Vec<u16> = act
            .notification
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(notif_wide.as_ptr()),
            ws_child_vis_i(ES_AUTOHSCROLL),
            20,
            280,
            420,
            24,
            hwnd,
            HMENU(IDC_ACT_TXT_NOTIF as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Command / Args
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Command line / Args:"),
            ws_child_vis_i(0),
            20,
            320,
            180,
            20,
            hwnd,
            None,
            hinst,
            None,
        )
        .unwrap();
        let cmd_val = if act.built_in.is_empty() {
            &act.command
        } else {
            &act.args
        };
        let cmd_wide: Vec<u16> = cmd_val.encode_utf16().chain(std::iter::once(0)).collect();
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(cmd_wide.as_ptr()),
            ws_child_vis_i(ES_AUTOHSCROLL),
            20,
            345,
            420,
            24,
            hwnd,
            HMENU(IDC_ACT_TXT_COMMAND as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Save & Cancel buttons
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Save"),
            ws_child_vis_i(BS_DEFPUSHBUTTON),
            250,
            400,
            90,
            28,
            hwnd,
            HMENU(IDC_ACT_BTN_SAVE as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Cancel"),
            ws_child_vis_i(BS_PUSHBUTTON),
            350,
            400,
            90,
            28,
            hwnd,
            HMENU(IDC_ACT_BTN_CANCEL as isize as *mut _),
            hinst,
            None,
        )
        .unwrap();

        // Apply modern Segoe UI font to action editor controls
        apply_ui_font_recursive(hwnd, get_ui_normal_font());
    }
}

fn handle_action_editor_command(hwnd: HWND, id: usize, state: &mut ActionEditorState) {
    match id {
        IDC_ACT_BTN_CAPTURE => {
            let h_btn = unsafe { get_ctrl(hwnd, IDC_ACT_BTN_CAPTURE) };
            let h_txt = unsafe { get_ctrl(hwnd, IDC_ACT_HOTKEY_TXT) };
            unsafe {
                let _ = SetWindowTextW(h_btn, w!("Listening..."));
                let _ = SetWindowTextW(h_txt, w!("[ Press key/button... ]"));
            }

            let hwnd_u = hwnd.0 as usize;
            state.capture_session.start(
                Arc::new(move |_mask| {}),
                Arc::new(move |final_mask| {
                    let wparam = WPARAM((final_mask & 0xFFFF_FFFF) as usize);
                    let lparam = LPARAM(((final_mask >> 32) & 0xFFFF_FFFF) as isize);
                    unsafe {
                        let _ = PostMessageW(
                            HWND(hwnd_u as *mut _),
                            WM_HOTKEY_CAPTURED,
                            wparam,
                            lparam,
                        );
                    }
                }),
            );
        }
        IDC_ACT_BTN_CLEAR_HOTKEY => {
            state.action_config.hotkey = 0;
            let h_txt = unsafe { get_ctrl(hwnd, IDC_ACT_HOTKEY_TXT) };
            unsafe {
                let _ = SetWindowTextW(h_txt, w!("None"));
            }
        }
        IDC_ACT_BTN_TEST_SOUND => {
            let h_snd = unsafe { get_ctrl(hwnd, IDC_ACT_CMB_SOUND) };
            let sel = get_combo_selected_text(h_snd);
            if !sel.is_empty() && sel != "None" {
                state.sound.play(&sel, 100);
            }
        }
        IDC_ACT_BTN_BROWSE_SOUND => {
            if let Some(path) = prompt_browse_sound(hwnd) {
                let h_snd = unsafe { get_ctrl(hwnd, IDC_ACT_CMB_SOUND) };
                select_or_add_combo_item(h_snd, &path);
                state.sound.play(&path, 100);
            }
        }
        IDC_ACT_BTN_SAVE => {
            // Read input fields
            unsafe {
                let mut buf = [0u16; 256];

                // Name
                let h_name = get_ctrl(hwnd, IDC_ACT_NAME);
                let len = GetWindowTextW(h_name, &mut buf);
                state.action_config.name = String::from_utf16_lossy(&buf[..len as usize]);

                // Type
                let h_type = get_ctrl(hwnd, IDC_ACT_TYPE);
                let sel = SendMessageW(h_type, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as usize;
                if sel < ALL_BUILT_IN_ACTIONS.len() {
                    state.action_config.built_in = ALL_BUILT_IN_ACTIONS[sel].key.to_string();
                } else {
                    state.action_config.built_in.clear();
                }

                // Presses
                let h_prs = get_ctrl(hwnd, IDC_ACT_PRESSES);
                let prs_sel = SendMessageW(h_prs, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as usize;
                state.action_config.presses = (prs_sel + 1) as u8;

                // Release
                let h_rel = get_ctrl(hwnd, IDC_ACT_CHK_RELEASE);
                state.action_config.on_release =
                    SendMessageW(h_rel, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;

                // Block
                let h_blk = get_ctrl(hwnd, IDC_ACT_CHK_BLOCK);
                state.action_config.block =
                    SendMessageW(h_blk, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;

                // Tap
                let h_tap = get_ctrl(hwnd, IDC_ACT_CHK_TAP);
                state.action_config.tap_only =
                    SendMessageW(h_tap, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;

                // Sound
                let h_snd = get_ctrl(hwnd, IDC_ACT_CMB_SOUND);
                let sound_text = get_combo_selected_text(h_snd);
                state.action_config.sound = if sound_text.is_empty() || sound_text == "None" {
                    String::new()
                } else {
                    sound_text
                };

                // Notification
                let h_not_chk = get_ctrl(hwnd, IDC_ACT_CHK_NOTIF);
                state.action_config.show_notification =
                    SendMessageW(h_not_chk, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;
                let h_not_txt = get_ctrl(hwnd, IDC_ACT_TXT_NOTIF);
                let n_len = GetWindowTextW(h_not_txt, &mut buf);
                state.action_config.notification = String::from_utf16_lossy(&buf[..n_len as usize]);

                // Command / Args
                let h_cmd = get_ctrl(hwnd, IDC_ACT_TXT_COMMAND);
                let c_len = GetWindowTextW(h_cmd, &mut buf);
                let cmd_str = String::from_utf16_lossy(&buf[..c_len as usize]);
                if state.action_config.built_in.is_empty() {
                    state.action_config.command = cmd_str;
                } else {
                    state.action_config.args = cmd_str;
                }
            }

            // Save to AppConfig
            {
                let mut cfg = state.config.write();
                if let Some(idx) = state.action_index {
                    if idx < cfg.actions.len() {
                        cfg.actions[idx] = state.action_config.clone();
                    }
                } else {
                    cfg.actions.push(state.action_config.clone());
                }
            }

            // Refresh parent settings window hotkey list
            unsafe {
                let win_ptr =
                    GetWindowLongPtrW(state.parent_settings, GWLP_USERDATA) as *mut SettingsWindow;
                if !win_ptr.is_null() {
                    let win = &*win_ptr;
                    let h_list = get_ctrl(win.tab_panels[3], IDC_LST_ACTIONS);
                    if !h_list.is_invalid() && h_list.0 as isize != 0 {
                        let cfg = win.config.read();
                        populate_actions_list(h_list, &cfg.actions);
                    }
                }
            }

            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
        IDC_ACT_BTN_CANCEL => {
            state.capture_session.cancel();
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
        _ => {}
    }
}
