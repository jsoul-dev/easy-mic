//! Interactive hotkey combination capture for Settings window binding.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;

pub type PreviewCallback = Arc<dyn Fn(u64) + Send + Sync>;
pub type DoneCallback = Arc<dyn Fn(u64) + Send + Sync>;

pub struct HotkeyCaptureSession {
    active: AtomicBool,
    pending_done: AtomicBool,
    captured_mask: AtomicU64,
    preview_cb: Mutex<Option<PreviewCallback>>,
    done_cb: Mutex<Option<DoneCallback>>,
}

impl Default for HotkeyCaptureSession {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyCaptureSession {
    pub fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            pending_done: AtomicBool::new(false),
            captured_mask: AtomicU64::new(0),
            preview_cb: Mutex::new(None),
            done_cb: Mutex::new(None),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn start(&self, on_preview: PreviewCallback, on_done: DoneCallback) {
        *self.preview_cb.lock() = Some(on_preview);
        *self.done_cb.lock() = Some(on_done);
        self.captured_mask.store(0, Ordering::Release);
        self.pending_done.store(false, Ordering::Release);
        self.active.store(true, Ordering::Release);
    }

    pub fn cancel(&self) {
        self.active.store(false, Ordering::Release);
        self.pending_done.store(false, Ordering::Release);
        *self.preview_cb.lock() = None;
        *self.done_cb.lock() = None;
    }

    pub fn on_event(&self, vk_code: u8, is_pressed: bool, sequence_mask: u64) {
        if !self.active.load(Ordering::Acquire) || self.pending_done.load(Ordering::Acquire) {
            return;
        }

        if is_pressed {
            self.captured_mask.store(sequence_mask, Ordering::Release);
            if let Some(ref cb) = *self.preview_cb.lock() {
                cb(sequence_mask);
            }
        } else {
            // Key released
            let current = self.captured_mask.load(Ordering::Acquire);
            if current == 0 && vk_code != (VK_ESCAPE.0 as u8) {
                return;
            }

            let final_mask = if vk_code == (VK_ESCAPE.0 as u8) && sequence_mask == 0 {
                0
            } else {
                current
            };

            self.pending_done.store(true, Ordering::Release);
            self.active.store(false, Ordering::Release);

            if let Some(cb) = self.done_cb.lock().take() {
                cb(final_mask);
            }
            *self.preview_cb.lock() = None;
        }
    }
}
