//! Embedded icon resources and HICON creation.

use windows::Win32::Foundation::BOOL;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, DestroyIcon, HICON, LR_DEFAULTCOLOR,
};

pub const BYTES_APP_ICON: &[u8] = include_bytes!("../../resources/appicon.ico");
pub const BYTES_MIC_MUTED: &[u8] = include_bytes!("../../resources/mic-muted.ico");
pub const BYTES_MIC_UNMUTED: &[u8] = include_bytes!("../../resources/mic-unmuted.ico");
pub const BYTES_MIC_ACTIVE: &[u8] = include_bytes!("../../resources/mic-active.ico");

/// Parses an ICO file buffer and creates an HICON for the requested size.
pub fn create_hicon_from_ico_bytes(bytes: &[u8], desired_size: i32) -> Option<HICON> {
    if bytes.len() < 6 {
        return None;
    }

    let id_type = u16::from_le_bytes([bytes[2], bytes[3]]);
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    if id_type != 1 || count == 0 {
        return None;
    }

    let mut best_index = 0;
    let mut best_diff = i32::MAX;

    for i in 0..count {
        let entry_offset = 6 + i * 16;
        if entry_offset + 16 > bytes.len() {
            break;
        }

        let width = match bytes[entry_offset] {
            0 => 256,
            w => w as i32,
        };
        let diff = (width - desired_size).abs();
        if diff < best_diff {
            best_diff = diff;
            best_index = i;
        }
    }

    let entry_offset = 6 + best_index * 16;
    let bytes_in_res = u32::from_le_bytes([
        bytes[entry_offset + 8],
        bytes[entry_offset + 9],
        bytes[entry_offset + 10],
        bytes[entry_offset + 11],
    ]) as usize;

    let image_offset = u32::from_le_bytes([
        bytes[entry_offset + 12],
        bytes[entry_offset + 13],
        bytes[entry_offset + 14],
        bytes[entry_offset + 15],
    ]) as usize;

    if image_offset + bytes_in_res > bytes.len() {
        return None;
    }

    let icon_data = &bytes[image_offset..image_offset + bytes_in_res];

    unsafe {
        let hicon = CreateIconFromResourceEx(
            icon_data,
            BOOL::from(true),
            0x00030000,
            desired_size,
            desired_size,
            LR_DEFAULTCOLOR,
        )
        .ok()?;
        Some(hicon)
    }
}

pub struct AppIcons {
    pub app_icon: Option<HICON>,
    pub mic_muted: Option<HICON>,
    pub mic_unmuted: Option<HICON>,
    pub mic_active: Option<HICON>,
}

unsafe impl Send for AppIcons {}
unsafe impl Sync for AppIcons {}

impl Default for AppIcons {
    fn default() -> Self {
        Self::new(32)
    }
}

impl AppIcons {
    pub fn new(size: i32) -> Self {
        Self {
            app_icon: create_hicon_from_ico_bytes(BYTES_APP_ICON, size),
            mic_muted: create_hicon_from_ico_bytes(BYTES_MIC_MUTED, size),
            mic_unmuted: create_hicon_from_ico_bytes(BYTES_MIC_UNMUTED, size),
            mic_active: create_hicon_from_ico_bytes(BYTES_MIC_ACTIVE, size),
        }
    }
}

impl Drop for AppIcons {
    fn drop(&mut self) {
        unsafe {
            if let Some(h) = self.app_icon.take() {
                let _ = DestroyIcon(h);
            }
            if let Some(h) = self.mic_muted.take() {
                let _ = DestroyIcon(h);
            }
            if let Some(h) = self.mic_unmuted.take() {
                let _ = DestroyIcon(h);
            }
            if let Some(h) = self.mic_active.take() {
                let _ = DestroyIcon(h);
            }
        }
    }
}
