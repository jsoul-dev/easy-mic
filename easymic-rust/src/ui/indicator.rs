//! On-screen transparent layered pill indicator window and peak meter.

use super::icons::AppIcons;
use crate::audio::manager::AudioManager;
use crate::core::config::{AppConfig, IndicatorState};
use crate::core::state::{SharedState, StateChangeEvent};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use windows::core::w;
use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_INDICATOR_STATE: u32 = WM_USER + 201;
const WM_INDICATOR_NOTIF: u32 = WM_USER + 202;
const ID_PEAK_TIMER: usize = 1001;
const ID_NOTIF_TIMER: usize = 1002;
const PEAK_INTERVAL_MS: u32 = 150;
const NOTIF_DURATION_MS: u32 = 2000;

pub struct IndicatorWindow {
    hwnd: HWND,
    state: SharedState,
    audio: Arc<AudioManager>,
    config: Arc<RwLock<AppConfig>>,
    icons: Arc<AppIcons>,
    interactive: AtomicBool,
    pos_x: AtomicI32,
    pos_y: AtomicI32,
}

unsafe impl Send for IndicatorWindow {}
unsafe impl Sync for IndicatorWindow {}

impl IndicatorWindow {
    pub fn create(
        hinstance: HINSTANCE,
        state: SharedState,
        audio: Arc<AudioManager>,
        config: Arc<RwLock<AppConfig>>,
        icons: Arc<AppIcons>,
    ) -> Result<Arc<Self>, windows::core::Error> {
        let class_name = w!("EasyMicIndicatorWindow");

        unsafe {
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(indicator_wndproc),
                hInstance: hinstance,
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);

            let (init_x, init_y) = {
                let cfg = config.read();
                (cfg.window_pos_x, cfg.window_pos_y)
            };

            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
                class_name,
                w!("EasyMic Indicator"),
                WS_POPUP | WS_DISABLED,
                init_x,
                init_y,
                128,
                32,
                None,
                None,
                hinstance,
                None,
            )?;

            let window = Arc::new(Self {
                hwnd,
                state: state.clone(),
                audio,
                config,
                icons,
                interactive: AtomicBool::new(false),
                pos_x: AtomicI32::new(init_x),
                pos_y: AtomicI32::new(init_y),
            });

            // Store window pointer in GWLP_USERDATA
            let ptr = Arc::as_ptr(&window) as isize;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr);

            // Start state listener thread to trigger updates
            let hwnd_u = hwnd.0 as isize;
            let rx = state.subscribe();
            std::thread::Builder::new()
                .name("easymic-indicator-listener".to_string())
                .spawn(move || {
                    let target_hwnd = HWND(hwnd_u as *mut _);
                    while let Ok(event) = rx.recv() {
                        match event {
                            StateChangeEvent::NotificationPosted { text } => {
                                let boxed = Box::new(text);
                                let _ = PostMessageW(
                                    target_hwnd,
                                    WM_INDICATOR_NOTIF,
                                    WPARAM(0),
                                    LPARAM(Box::into_raw(boxed) as isize),
                                );
                            }
                            _ => {
                                let _ = PostMessageW(
                                    target_hwnd,
                                    WM_INDICATOR_STATE,
                                    WPARAM(0),
                                    LPARAM(0),
                                );
                            }
                        }
                    }
                })
                .expect("Failed to spawn indicator listener");

            // Initial render
            window.render();
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

            Ok(window)
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn toggle_interactivity(&self, interactive: bool) {
        self.interactive.store(interactive, Ordering::Release);
        unsafe {
            let mut ex_style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let mut style = GetWindowLongPtrW(self.hwnd, GWL_STYLE);

            if interactive {
                ex_style &= !(WS_EX_TRANSPARENT.0 as isize);
                style &= !(WS_DISABLED.0 as isize);
            } else {
                ex_style |= WS_EX_TRANSPARENT.0 as isize;
                style |= WS_DISABLED.0 as isize;
            }

            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex_style);
            SetWindowLongPtrW(self.hwnd, GWL_STYLE, style);
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        self.render();
    }

    pub fn get_position(&self) -> (i32, i32) {
        (
            self.pos_x.load(Ordering::Acquire),
            self.pos_y.load(Ordering::Acquire),
        )
    }

    pub fn update_position(&self, x: i32, y: i32) {
        self.pos_x.store(x, Ordering::Release);
        self.pos_y.store(y, Ordering::Release);
        let mut cfg = self.config.write();
        cfg.window_pos_x = x;
        cfg.window_pos_y = y;
        let _ = cfg.save();
    }

    pub fn render(&self) {
        self.render_with_text(None, false);
    }

    pub fn render_with_text(&self, notif_text: Option<&str>, is_active_voice: bool) {
        let cfg = self.config.read();
        let is_muted = self.state.is_muted();
        let has_device = self.state.has_device();

        // Check visibility rules
        let should_show = match cfg.indicator_state {
            IndicatorState::Hidden => false,
            IndicatorState::Muted => {
                is_muted
                    || !has_device
                    || notif_text.is_some()
                    || self.interactive.load(Ordering::Acquire)
            }
            IndicatorState::MutedOrTalk => true,
        };

        if !should_show {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            return;
        }

        let icon_sz = cfg.indicator_size as i32;
        let pill_h = icon_sz * 2;
        let font_sz = (pill_h as f32 * 0.46).clamp(11.0, 24.0) as i32;

        let has_text = notif_text.is_some();
        let text_content = notif_text.unwrap_or("");

        // Calculate text width
        let (text_w, total_w) = unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(screen_dc);

            let font = CreateFontW(
                -font_sz,
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
            let prev_font = SelectObject(mem_dc, font);

            let mut sz = SIZE { cx: 0, cy: 0 };
            if has_text {
                let wide: Vec<u16> = text_content.encode_utf16().collect();
                let _ = GetTextExtentPoint32W(mem_dc, &wide, &mut sz);
            }

            SelectObject(mem_dc, prev_font);
            let _ = DeleteObject(font);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);

            let padding = (pill_h as f32 * 0.25).max(6.0) as i32;
            let tw = if has_text { sz.cx + padding * 2 } else { 0 };
            let total = if has_text { pill_h + 8 + tw } else { pill_h };
            (tw, total)
        };

        // Render DIB section for UpdateLayeredWindow
        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(screen_dc);

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: total_w,
                    biHeight: -pill_h, // Top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap =
                CreateDIBSection(screen_dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();

            let prev_bmp = SelectObject(mem_dc, bitmap);

            // Fill 32-bit premultiplied ARGB buffer
            // Background pill color: RGBA(24, 27, 40, 220) -> Premultiplied:
            // R = 24 * 220 / 255 = 20
            // G = 27 * 220 / 255 = 23
            // B = 40 * 220 / 255 = 34
            // A = 220
            let bg_pixel = 0xDC_14_17_22u32; // ARGB (0xDC = 220, 0x14 = 20, 0x17 = 23, 0x22 = 34)
            let pixel_slice =
                std::slice::from_raw_parts_mut(bits as *mut u32, (total_w * pill_h) as usize);

            // Draw rounded pill 1 (Icon pill)
            let r = 10;
            for y in 0..pill_h {
                for x in 0..pill_h {
                    if is_inside_round_rect(x, y, 0, 0, pill_h, pill_h, r) {
                        pixel_slice[(y * total_w + x) as usize] = bg_pixel;
                    }
                }
            }

            // Draw rounded pill 2 (Text pill) if present
            if has_text {
                let tx_start = pill_h + 8;
                for y in 0..pill_h {
                    for x in tx_start..total_w {
                        if is_inside_round_rect(x, y, tx_start, 0, text_w, pill_h, r) {
                            pixel_slice[(y * total_w + x) as usize] = bg_pixel;
                        }
                    }
                }
            }

            // Draw Icon centered inside icon pill
            let icon_h = if is_active_voice {
                self.icons.mic_active
            } else if is_muted || !has_device {
                self.icons.mic_muted
            } else {
                self.icons.mic_unmuted
            };

            if let Some(hicon) = icon_h {
                let inset = (pill_h - icon_sz) / 2;
                let _ = DrawIconEx(
                    mem_dc, inset, inset, hicon, icon_sz, icon_sz, 0, None, DI_NORMAL,
                );
            }

            // Draw Text centered in text pill
            if has_text {
                let font = CreateFontW(
                    -font_sz,
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
                let prev_font = SelectObject(mem_dc, font);

                SetBkMode(mem_dc, TRANSPARENT);
                SetTextColor(mem_dc, COLORREF(0x00_F0_F0_F0)); // BGR whiteish

                let tx_start = pill_h + 8;
                let mut rect = RECT {
                    left: tx_start,
                    top: 0,
                    right: total_w,
                    bottom: pill_h,
                };

                let wide: Vec<u16> = text_content.encode_utf16().collect();
                DrawTextW(
                    mem_dc,
                    &mut wide.clone(),
                    &mut rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                SelectObject(mem_dc, prev_font);
                let _ = DeleteObject(font);
            }

            // Apply Display Affinity (exclude from screen recording if configured)
            if cfg.exclude_from_capture {
                let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity(
                    self.hwnd,
                    windows::Win32::UI::WindowsAndMessaging::WDA_EXCLUDEFROMCAPTURE,
                );
            } else {
                let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity(
                    self.hwnd,
                    windows::Win32::UI::WindowsAndMessaging::WDA_NONE,
                );
            }

            let pt_dst = POINT {
                x: self.pos_x.load(Ordering::Acquire),
                y: self.pos_y.load(Ordering::Acquire),
            };
            let pt_src = POINT { x: 0, y: 0 };
            let sz = SIZE {
                cx: total_w,
                cy: pill_h,
            };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            let _ = UpdateLayeredWindow(
                self.hwnd,
                screen_dc,
                Some(&pt_dst),
                Some(&sz),
                mem_dc,
                Some(&pt_src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );

            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);

            SelectObject(mem_dc, prev_bmp);
            let _ = DeleteObject(bitmap);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    }
}

fn is_inside_round_rect(px: i32, py: i32, rx: i32, ry: i32, rw: i32, rh: i32, rad: i32) -> bool {
    let x = px - rx;
    let y = py - ry;
    if x < 0 || y < 0 || x >= rw || y >= rh {
        return false;
    }
    // Check four corners
    if x < rad && y < rad {
        let dx = rad - x;
        let dy = rad - y;
        return dx * dx + dy * dy <= rad * rad;
    }
    if x >= rw - rad && y < rad {
        let dx = x - (rw - rad - 1);
        let dy = rad - y;
        return dx * dx + dy * dy <= rad * rad;
    }
    if x < rad && y >= rh - rad {
        let dx = rad - x;
        let dy = y - (rh - rad - 1);
        return dx * dx + dy * dy <= rad * rad;
    }
    if x >= rw - rad && y >= rh - rad {
        let dx = x - (rw - rad - 1);
        let dy = y - (rh - rad - 1);
        return dx * dx + dy * dy <= rad * rad;
    }
    true
}

// Window procedure for the indicator window
unsafe extern "system" fn indicator_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let window = &*(ptr as *const IndicatorWindow);

    match msg {
        WM_CREATE => {
            // Start peak timer (150ms)
            SetTimer(hwnd, ID_PEAK_TIMER, PEAK_INTERVAL_MS, None);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == ID_PEAK_TIMER {
                let cfg = window.config.read();
                if cfg.indicator_state == IndicatorState::MutedOrTalk && !window.state.is_muted() {
                    let peak = window.audio.get_peak();
                    let talking = peak > cfg.indicator_volume_threshold;
                    if talking {
                        window.render_with_text(None, true);
                    } else {
                        window.render();
                    }
                }
            } else if wparam.0 == ID_NOTIF_TIMER {
                let _ = KillTimer(hwnd, ID_NOTIF_TIMER);
                window.render();
            }
            LRESULT(0)
        }
        WM_INDICATOR_STATE => {
            window.render();
            LRESULT(0)
        }
        WM_INDICATOR_NOTIF => {
            let str_ptr = lparam.0 as *mut String;
            if !str_ptr.is_null() {
                let notif_text = *Box::from_raw(str_ptr);
                window.render_with_text(Some(&notif_text), false);
                SetTimer(hwnd, ID_NOTIF_TIMER, NOTIF_DURATION_MS, None);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            if window.interactive.load(Ordering::Acquire) {
                // Dragging the indicator window
                let _ = ReleaseCapture();
                let _ = SendMessageW(
                    hwnd,
                    WM_NCLBUTTONDOWN,
                    WPARAM(HTCAPTION as usize),
                    LPARAM(0),
                );
            }
            LRESULT(0)
        }
        WM_MOVE => {
            if window.interactive.load(Ordering::Acquire) {
                let mut rect = RECT::default();
                let _ = GetWindowRect(hwnd, &mut rect);
                window.update_position(rect.left, rect.top);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = KillTimer(hwnd, ID_PEAK_TIMER);
            let _ = KillTimer(hwnd, ID_NOTIF_TIMER);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
