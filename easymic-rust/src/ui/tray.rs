//! System tray icon, tooltip notifications, and context menu.

use super::icons::AppIcons;
use crate::core::state::{SharedState, StateChangeEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_TRAY_CALLBACK: u32 = WM_USER + 301;
pub const ID_TRAY_SETTINGS: usize = 2001;
pub const ID_TRAY_EXIT: usize = 2002;

pub struct TrayIcon {
    hwnd: HWND,
    state: SharedState,
    icons: Arc<AppIcons>,
    active: AtomicBool,
}

unsafe impl Send for TrayIcon {}
unsafe impl Sync for TrayIcon {}

impl TrayIcon {
    pub fn new(hwnd: HWND, state: SharedState, icons: Arc<AppIcons>) -> Arc<Self> {
        let tray = Arc::new(Self {
            hwnd,
            state: state.clone(),
            icons,
            active: AtomicBool::new(false),
        });

        tray.add_icon();

        // Listen for state changes to refresh tooltip and icon dynamically
        let tray_weak = Arc::downgrade(&tray);
        let rx = state.subscribe();
        std::thread::Builder::new()
            .name("easymic-tray-listener".to_string())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    match event {
                        StateChangeEvent::MuteChanged { .. }
                        | StateChangeEvent::VolumeChanged { .. }
                        | StateChangeEvent::DeviceChanged { .. } => {
                            if let Some(t) = tray_weak.upgrade() {
                                t.update_icon();
                            } else {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            })
            .expect("Failed to spawn tray listener");

        tray
    }

    fn get_tooltip(&self) -> Vec<u16> {
        let has_device = self.state.has_device();
        let name = self.state.device_name();
        let vol = self.state.volume_percent();

        let tip = if has_device {
            format!("Easymic - {} [{}%]", name, vol)
        } else {
            "Easymic - No device".to_string()
        };

        let mut wide: Vec<u16> = tip.encode_utf16().collect();
        wide.truncate(127);
        wide.push(0);
        wide
    }

    fn select_icon(&self) -> Option<HICON> {
        let is_muted = self.state.is_muted();
        let has_device = self.state.has_device();

        if is_muted || !has_device {
            self.icons.mic_muted
        } else {
            self.icons.mic_unmuted
        }
    }

    pub fn add_icon(&self) {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAY_CALLBACK,
                hIcon: self.select_icon().unwrap_or_default(),
                ..Default::default()
            };

            let tip = self.get_tooltip();
            let copy_len = tip.len().min(nid.szTip.len());
            nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);

            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
            self.active.store(true, Ordering::Release);
        }
    }

    pub fn update_icon(&self) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }

        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_TIP,
                hIcon: self.select_icon().unwrap_or_default(),
                ..Default::default()
            };

            let tip = self.get_tooltip();
            let copy_len = tip.len().min(nid.szTip.len());
            nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);

            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    pub fn show_context_menu(&self) -> Option<usize> {
        unsafe {
            let hmenu = CreatePopupMenu().ok()?;
            let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_SETTINGS, w!("Settings"));
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, w!("Exit"));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);

            let _ = SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_RETURNCMD | TPM_NONOTIFY,
                pt.x,
                pt.y,
                0,
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(hmenu);

            if cmd.0 != 0 {
                Some(cmd.0 as usize)
            } else {
                None
            }
        }
    }

    pub fn remove_icon(&self) {
        if self.active.swap(false, Ordering::AcqRel) {
            unsafe {
                let nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: self.hwnd,
                    uID: 1,
                    ..Default::default()
                };
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            }
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.remove_icon();
    }
}
