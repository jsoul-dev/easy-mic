//! Dedicated low-level Windows keyboard and mouse hook thread.
//! Ensures latency-critical hook callbacks are O(1), minimal, non-blocking,
//! and that WH_MOUSE_LL is only installed when mouse buttons are actually bound.

use super::capture::HotkeyCaptureSession;
use super::dispatch::{ActionDispatcher, ActionTrigger};
use super::keys::{get_modifier_bit, mask_contains_mouse_button};
use crate::core::config::AppConfig;
use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;
use windows::Win32::Foundation::{HMODULE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// Shared state accessed inside the hook callbacks
static HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);
static SEQUENCE_MASK: AtomicU64 = AtomicU64::new(0);
static LAST_DOWN_VK: AtomicU8 = AtomicU8::new(0);
static KEYS_PRESSED: [AtomicBool; 256] = [const { AtomicBool::new(false) }; 256];
static BLOCKED_KEYS: [AtomicBool; 256] = [const { AtomicBool::new(false) }; 256];

// Fast lock-free bindings lookup table for the hook proc
#[derive(Clone, Default)]
struct BindingLookup {
    bindings: Vec<MaskInfo>,
}

#[derive(Clone)]
struct MaskInfo {
    mask: u64,
    has_press: bool,
    has_release: bool,
    block: bool,
}

static LOOKUP_TABLE: RwLock<Option<BindingLookup>> = RwLock::new(None);
static GLOBAL_DISPATCHER: Mutex<Option<Arc<ActionDispatcher>>> = Mutex::new(None);
static GLOBAL_CAPTURE: Mutex<Option<Arc<HotkeyCaptureSession>>> = Mutex::new(None);

pub struct HookManager {
    config: Arc<RwLock<AppConfig>>,
    _dispatcher: Arc<ActionDispatcher>,
    _capture_session: Arc<HotkeyCaptureSession>,
    thread_id: AtomicU32,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl HookManager {
    pub fn new(
        config: Arc<RwLock<AppConfig>>,
        dispatcher: Arc<ActionDispatcher>,
        capture_session: Arc<HotkeyCaptureSession>,
    ) -> Arc<Self> {
        *GLOBAL_DISPATCHER.lock() = Some(dispatcher.clone());
        *GLOBAL_CAPTURE.lock() = Some(capture_session.clone());

        let manager = Arc::new(Self {
            config: config.clone(),
            _dispatcher: dispatcher,
            _capture_session: capture_session,
            thread_id: AtomicU32::new(0),
            handle: Mutex::new(None),
        });

        manager.update_bindings();
        manager.start_hook_thread();
        manager
    }

    pub fn update_bindings(&self) {
        let cfg = self.config.read();
        let mut lookup = BindingLookup::default();

        for action in &cfg.actions {
            if action.hotkey != 0 {
                let mask = action.hotkey;
                if let Some(existing) = lookup.bindings.iter_mut().find(|b| b.mask == mask) {
                    if action.on_release {
                        existing.has_release = true;
                    } else {
                        existing.has_press = true;
                    }
                    existing.block |= action.block;
                } else {
                    lookup.bindings.push(MaskInfo {
                        mask,
                        has_press: !action.on_release,
                        has_release: action.on_release,
                        block: action.block,
                    });
                }
            }
        }

        *LOOKUP_TABLE.write() = Some(lookup);

        // Notify hook thread if mouse hook installation requirement changed
        let thread_id = self.thread_id.load(Ordering::Acquire);
        if thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_USER + 101, WPARAM(0), LPARAM(0));
            }
        }
    }

    fn start_hook_thread(&self) {
        let (ready_tx, ready_rx) = crossbeam_channel::bounded::<u32>(1);
        let config_clone = self.config.clone();

        let handle = thread::Builder::new()
            .name("easymic-input-hook".to_string())
            .spawn(move || {
                unsafe {
                    let tid = GetCurrentThreadId();
                    let _ = ready_tx.send(tid);

                    // Install WH_KEYBOARD_LL
                    let kbd_hook = SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(low_level_keyboard_proc),
                        HMODULE::default(),
                        0,
                    )
                    .expect("Failed to install WH_KEYBOARD_LL");

                    let mut mouse_hook: Option<HHOOK> = None;
                    let check_mouse = || {
                        let cfg = config_clone.read();
                        cfg.actions
                            .iter()
                            .any(|a| mask_contains_mouse_button(a.hotkey))
                    };

                    if check_mouse() {
                        if let Ok(hook) = SetWindowsHookExW(
                            WH_MOUSE_LL,
                            Some(low_level_mouse_proc),
                            HMODULE::default(),
                            0,
                        ) {
                            mouse_hook = Some(hook);
                            log::info!("Installed WH_MOUSE_LL (mouse button hotkey detected)");
                        }
                    } else {
                        log::info!("Skipping WH_MOUSE_LL (no mouse hotkeys configured)");
                    }

                    HOOK_ACTIVE.store(true, Ordering::Release);

                    // Windows Message pump on dedicated thread
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).into() {
                        if msg.message == WM_USER + 101 {
                            // Update mouse hook status dynamically
                            let needed = check_mouse();
                            if needed && mouse_hook.is_none() {
                                if let Ok(h) = SetWindowsHookExW(
                                    WH_MOUSE_LL,
                                    Some(low_level_mouse_proc),
                                    HMODULE::default(),
                                    0,
                                ) {
                                    mouse_hook = Some(h);
                                    log::info!("Dynamically installed WH_MOUSE_LL");
                                }
                            } else if !needed && mouse_hook.is_some() {
                                if let Some(h) = mouse_hook.take() {
                                    let _ = UnhookWindowsHookEx(h);
                                    log::info!("Dynamically uninstalled WH_MOUSE_LL");
                                }
                            }
                        }
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }

                    HOOK_ACTIVE.store(false, Ordering::Release);

                    if let Some(h) = mouse_hook {
                        let _ = UnhookWindowsHookEx(h);
                    }
                    let _ = UnhookWindowsHookEx(kbd_hook);
                    log::info!("Input hook thread stopped cleanly");
                }
            })
            .expect("Failed to spawn hook thread");

        if let Ok(tid) = ready_rx.recv() {
            self.thread_id.store(tid, Ordering::Release);
        }
        *self.handle.lock() = Some(handle);
    }

    pub fn shutdown(&self) {
        let tid = self.thread_id.swap(0, Ordering::AcqRel);
        if tid != 0 {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        let mut handle = self.handle.lock();
        if let Some(h) = handle.take() {
            let _ = h.join();
        }
        *LOOKUP_TABLE.write() = None;
        *GLOBAL_DISPATCHER.lock() = None;
        *GLOBAL_CAPTURE.lock() = None;
    }
}

impl Drop for HookManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[inline(always)]
fn on_key_down(vk: u8, now: Instant) -> bool {
    LAST_DOWN_VK.store(vk, Ordering::Release);
    KEYS_PRESSED[vk as usize].store(true, Ordering::Release);

    let mod_bit = get_modifier_bit(vk);
    let mut current_mask = SEQUENCE_MASK.load(Ordering::Acquire);

    if mod_bit != 0 {
        current_mask |= mod_bit as u64;
    } else {
        let modifiers = (current_mask & 0xFF) as u8;
        let seq = (current_mask & !0xFF) << 8;
        current_mask = seq | ((vk as u64) << 8) | (modifiers as u64);
    }
    SEQUENCE_MASK.store(current_mask, Ordering::Release);

    // Interactive capture mode check
    if let Some(ref capture) = *GLOBAL_CAPTURE.lock() {
        if capture.is_active() {
            capture.on_event(vk, true, current_mask);
            return false;
        }
    }

    // Check bindings
    let single_mask = ((vk as u64) << 8) | (current_mask & 0xFF);
    let mut blocked = false;

    if let Some(ref lookup) = *LOOKUP_TABLE.read() {
        let check_fire = |m: u64| -> bool {
            if let Some(b) = lookup.bindings.iter().find(|info| info.mask == m) {
                if b.has_press {
                    if let Some(ref disp) = *GLOBAL_DISPATCHER.lock() {
                        disp.trigger(ActionTrigger {
                            mask: m,
                            is_release: false,
                            alone: LAST_DOWN_VK.load(Ordering::Acquire) == vk,
                            timestamp: now,
                        });
                    }
                }
                return b.block;
            }
            false
        };

        blocked |= check_fire(single_mask);
        if current_mask != single_mask {
            blocked |= check_fire(current_mask);
        }
    }

    BLOCKED_KEYS[vk as usize].store(blocked, Ordering::Release);
    blocked
}

#[inline(always)]
fn on_key_up(vk: u8, now: Instant) -> bool {
    let was_alone = LAST_DOWN_VK.load(Ordering::Acquire) == vk;
    let current_mask = SEQUENCE_MASK.load(Ordering::Acquire);
    let single_mask = ((vk as u64) << 8) | (current_mask & 0xFF);

    // Interactive capture check
    if let Some(ref capture) = *GLOBAL_CAPTURE.lock() {
        if capture.is_active() {
            capture.on_event(vk, false, current_mask);
        }
    }

    // Release actions check
    if let Some(ref lookup) = *LOOKUP_TABLE.read() {
        let check_release = |m: u64| {
            if let Some(b) = lookup.bindings.iter().find(|info| info.mask == m) {
                if b.has_release {
                    if let Some(ref disp) = *GLOBAL_DISPATCHER.lock() {
                        disp.trigger(ActionTrigger {
                            mask: m,
                            is_release: true,
                            alone: was_alone,
                            timestamp: now,
                        });
                    }
                }
            }
        };
        check_release(single_mask);
        if current_mask != single_mask {
            check_release(current_mask);
        }
    }

    // Update sequence mask
    let mod_bit = get_modifier_bit(vk);
    let new_mask = if mod_bit != 0 {
        current_mask & !(mod_bit as u64)
    } else {
        let modifiers = (current_mask & 0xFF) as u8;
        let last_key = ((current_mask >> 8) & 0xFF) as u8;
        if last_key == vk {
            ((current_mask >> 16) << 8) | (modifiers as u64)
        } else {
            modifiers as u64
        }
    };
    SEQUENCE_MASK.store(new_mask, Ordering::Release);
    KEYS_PRESSED[vk as usize].store(false, Ordering::Release);

    BLOCKED_KEYS[vk as usize].swap(false, Ordering::AcqRel)
}

/// Low-level keyboard hook callback procedure.
///
/// # Safety
///
/// Must be invoked by Windows OS as a valid `HOOKPROC`. `l_param` must point to a valid
/// `KBDLLHOOKSTRUCT` when `n_code == HC_ACTION`.
pub unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code != HC_ACTION as i32 {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let kbd = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
    let vk = kbd.vkCode as u8;
    let msg = w_param.0 as u32;

    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
        let already_down = KEYS_PRESSED[vk as usize].load(Ordering::Acquire);
        if !already_down {
            let blocked = on_key_down(vk, Instant::now());
            if blocked {
                return LRESULT(1);
            }
        } else if BLOCKED_KEYS[vk as usize].load(Ordering::Acquire) {
            // Eat autorepeat for swallowed keys
            return LRESULT(1);
        }
    } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
        let blocked = on_key_up(vk, Instant::now());
        if blocked {
            return LRESULT(1);
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}

/// Low-level mouse hook callback procedure.
///
/// # Safety
///
/// Must be invoked by Windows OS as a valid `HOOKPROC`. `l_param` must point to a valid
/// `MSLLHOOKSTRUCT` when `n_code == HC_ACTION`.
pub unsafe extern "system" fn low_level_mouse_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code != HC_ACTION as i32 {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let msg = w_param.0 as u32;

    // IMMEDIATE SHORTCUT: Mouse moves and wheels return instantly with 0 overhead!
    if msg == WM_MOUSEMOVE || msg == WM_MOUSEWHEEL || msg == WM_MOUSEHWHEEL {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let mouse = &*(l_param.0 as *const MSLLHOOKSTRUCT);
    let mut vk: u8 = 0;
    let mut is_up = false;

    match msg {
        WM_LBUTTONDOWN => vk = VK_LBUTTON.0 as u8,
        WM_LBUTTONUP => {
            vk = VK_LBUTTON.0 as u8;
            is_up = true;
        }
        WM_RBUTTONDOWN => vk = VK_RBUTTON.0 as u8,
        WM_RBUTTONUP => {
            vk = VK_RBUTTON.0 as u8;
            is_up = true;
        }
        WM_MBUTTONDOWN => vk = VK_MBUTTON.0 as u8,
        WM_MBUTTONUP => {
            vk = VK_MBUTTON.0 as u8;
            is_up = true;
        }
        WM_XBUTTONDOWN => {
            let btn = (mouse.mouseData >> 16) + 4;
            vk = btn as u8;
        }
        WM_XBUTTONUP => {
            let btn = (mouse.mouseData >> 16) + 4;
            vk = btn as u8;
            is_up = true;
        }
        _ => {}
    }

    if vk != 0 {
        if !is_up {
            let already_down = KEYS_PRESSED[vk as usize].load(Ordering::Acquire);
            if !already_down {
                let blocked = on_key_down(vk, Instant::now());
                if blocked {
                    return LRESULT(1);
                }
            } else if BLOCKED_KEYS[vk as usize].load(Ordering::Acquire) {
                return LRESULT(1);
            }
        } else {
            let blocked = on_key_up(vk, Instant::now());
            if blocked {
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}
