//! Action dispatch worker and execution engine.
//! Handles multi-press window timing, prompt execution, non-blocking audio control,
//! and asynchronous sound feedback.

use crate::audio::manager::AudioManager;
use crate::audio::sound::SoundManager;
use crate::core::actions::{find_built_in, BuiltInId};
use crate::core::config::{ActionConfig, AppConfig};
use crate::core::diagnostics::{next_correlation_id, DiagnosticsBuffer, LatencyRecord};
use crate::core::state::SharedState;
use crate::core::tokens::{replace_tokens, TOKEN_STDOUT};
use crate::input::keys::get_hotkey_name;
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::{Mutex, RwLock};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ActionTrigger {
    pub mask: u64,
    pub is_release: bool,
    pub alone: bool,
    pub timestamp: Instant,
}

struct PendingPress {
    mask: u64,
    count: u8,
    deadline: Instant,
    first_timestamp: Instant,
}

pub struct ActionDispatcher {
    _state: SharedState,
    _audio_manager: Arc<AudioManager>,
    _sound_manager: Arc<SoundManager>,
    _diagnostics: Arc<DiagnosticsBuffer>,
    _config: Arc<RwLock<AppConfig>>,
    sender: Sender<ActionTrigger>,
    running: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl ActionDispatcher {
    pub fn new(
        state: SharedState,
        audio_manager: Arc<AudioManager>,
        sound_manager: Arc<SoundManager>,
        diagnostics: Arc<DiagnosticsBuffer>,
        config: Arc<RwLock<AppConfig>>,
    ) -> Arc<Self> {
        let (sender, receiver) = bounded::<ActionTrigger>(64);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let state_c = state.clone();
        let audio_c = audio_manager.clone();
        let sound_c = sound_manager.clone();
        let diag_c = diagnostics.clone();
        let cfg_c = config.clone();

        let handle = thread::Builder::new()
            .name("easymic-action-dispatcher".to_string())
            .spawn(move || {
                Self::worker_loop(
                    receiver,
                    running_clone,
                    state_c,
                    audio_c,
                    sound_c,
                    diag_c,
                    cfg_c,
                );
            })
            .expect("Failed to spawn action dispatcher thread");

        Arc::new(Self {
            _state: state,
            _audio_manager: audio_manager,
            _sound_manager: sound_manager,
            _diagnostics: diagnostics,
            _config: config,
            sender,
            running,
            handle: Mutex::new(Some(handle)),
        })
    }

    pub fn trigger(&self, trigger: ActionTrigger) {
        // try_send prevents the hook thread from ever blocking
        let _ = self.sender.try_send(trigger);
    }

    fn worker_loop(
        receiver: Receiver<ActionTrigger>,
        running: Arc<AtomicBool>,
        state: SharedState,
        audio: Arc<AudioManager>,
        sound: Arc<SoundManager>,
        diag: Arc<DiagnosticsBuffer>,
        config: Arc<RwLock<AppConfig>>,
    ) {
        let mut pending: Option<PendingPress> = None;

        while running.load(Ordering::Relaxed) {
            let timeout = if let Some(ref p) = pending {
                let now = Instant::now();
                if now >= p.deadline {
                    // Deadline passed: resolve multi-press
                    Self::execute_matched_press(
                        p.mask,
                        p.count,
                        p.first_timestamp,
                        &state,
                        &audio,
                        &sound,
                        &diag,
                        &config,
                    );
                    pending = None;
                    Duration::from_millis(100)
                } else {
                    p.deadline.saturating_duration_since(now)
                }
            } else {
                Duration::from_millis(100)
            };

            match receiver.recv_timeout(timeout) {
                Ok(trigger) => {
                    let now = Instant::now();
                    let cfg = config.read();
                    let window_ms = cfg.multi_press_window_ms;

                    // Find matching actions for this mask
                    let matched_actions: Vec<&ActionConfig> = cfg
                        .actions
                        .iter()
                        .filter(|a| a.hotkey == trigger.mask)
                        .collect();

                    if matched_actions.is_empty() {
                        continue;
                    }

                    // Key release handling (Push-to-talk release or OnRelease actions)
                    if trigger.is_release {
                        for action in matched_actions {
                            let built_in = find_built_in(&action.built_in);
                            if let Some(bi) = built_in {
                                if bi.id == BuiltInId::PushToTalk {
                                    // PTT key up: mute
                                    let _ = audio.set_mute(true);
                                    sound.play(&cfg.mute_sound_source, cfg.bell_volume as u8);
                                    continue;
                                }
                            }
                            if action.on_release && (!action.tap_only || trigger.alone) {
                                Self::execute_single_action(
                                    action,
                                    trigger.timestamp,
                                    &state,
                                    &audio,
                                    &sound,
                                    &diag,
                                    &cfg,
                                );
                            }
                        }
                        continue;
                    }

                    // Press event handling:
                    let max_presses = matched_actions.iter().map(|a| a.presses).max().unwrap_or(1);

                    // If only 1 press is configured and no higher press count exists:
                    // FIRE IMMEDIATELY! Zero delay!
                    if max_presses <= 1 {
                        if let Some(p) = pending.take() {
                            Self::execute_matched_press(
                                p.mask,
                                p.count,
                                p.first_timestamp,
                                &state,
                                &audio,
                                &sound,
                                &diag,
                                &config,
                            );
                        }
                        for action in matched_actions {
                            if !action.on_release {
                                Self::execute_single_action(
                                    action,
                                    trigger.timestamp,
                                    &state,
                                    &audio,
                                    &sound,
                                    &diag,
                                    &cfg,
                                );
                            }
                        }
                        continue;
                    }

                    // Multi-press counting:
                    if let Some(mut p) = pending.take() {
                        if p.mask == trigger.mask && now < p.deadline {
                            p.count += 1;
                            if p.count >= max_presses {
                                Self::execute_matched_press(
                                    p.mask,
                                    p.count,
                                    p.first_timestamp,
                                    &state,
                                    &audio,
                                    &sound,
                                    &diag,
                                    &config,
                                );
                            } else {
                                p.deadline = now + Duration::from_millis(window_ms as u64);
                                pending = Some(p);
                            }
                            continue;
                        } else {
                            // Different hotkey or expired: resolve previous
                            Self::execute_matched_press(
                                p.mask,
                                p.count,
                                p.first_timestamp,
                                &state,
                                &audio,
                                &sound,
                                &diag,
                                &config,
                            );
                        }
                    }

                    // Start new multi-press window
                    pending = Some(PendingPress {
                        mask: trigger.mask,
                        count: 1,
                        deadline: now + Duration::from_millis(window_ms as u64),
                        first_timestamp: trigger.timestamp,
                    });
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Check deadline in next loop iteration
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_matched_press(
        mask: u64,
        count: u8,
        timestamp: Instant,
        state: &SharedState,
        audio: &Arc<AudioManager>,
        sound: &Arc<SoundManager>,
        diag: &Arc<DiagnosticsBuffer>,
        config: &Arc<RwLock<AppConfig>>,
    ) {
        let cfg = config.read();
        for action in &cfg.actions {
            if action.hotkey == mask && action.presses == count && !action.on_release {
                Self::execute_single_action(action, timestamp, state, audio, sound, diag, &cfg);
            }
        }
    }

    fn execute_single_action(
        action: &ActionConfig,
        input_timestamp: Instant,
        state: &SharedState,
        audio: &Arc<AudioManager>,
        sound: &Arc<SoundManager>,
        diag: &Arc<DiagnosticsBuffer>,
        cfg: &AppConfig,
    ) {
        let corr_id = next_correlation_id();
        let mut record = LatencyRecord::new(corr_id, &action.name, input_timestamp);
        record.t_hotkey_recognized = Some(input_timestamp);
        record.t_action_queued = Some(input_timestamp);
        record.t_worker_started = Some(Instant::now());

        if let Some(built_in) = find_built_in(&action.built_in) {
            match built_in.id {
                BuiltInId::ToggleMute => {
                    record.t_endpoint_call_begin = Some(Instant::now());
                    match audio.toggle_mute() {
                        Ok(new_muted) => {
                            let now = Instant::now();
                            record.t_endpoint_call_end = Some(now);
                            record.t_mute_verified = Some(now);

                            // Play appropriate state sound cue asynchronously
                            record.t_sound_requested = Some(now);
                            let cue = if new_muted {
                                &cfg.mute_sound_source
                            } else {
                                &cfg.unmute_sound_source
                            };
                            sound.play(cue, cfg.bell_volume as u8);
                            record.t_sound_started = Some(Instant::now());

                            if action.show_notification && cfg.notifications_enabled {
                                let template = if !action.notification.is_empty() {
                                    &action.notification
                                } else {
                                    built_in.default_notification
                                };
                                let text = replace_tokens(
                                    template,
                                    &action.name,
                                    &get_hotkey_name(action.hotkey),
                                    state.volume_percent(),
                                    new_muted,
                                    state.is_bell_enabled(),
                                    None,
                                );
                                state.post_notification(text);
                                record.t_ui_posted = Some(Instant::now());
                            }
                        }
                        Err(e) => {
                            log::error!("Microphone toggle failed: {}", e);
                        }
                    }
                }
                BuiltInId::PushToTalk => {
                    // PTT press: unmute
                    let _ = audio.set_mute(false);
                    sound.play(&cfg.unmute_sound_source, cfg.bell_volume as u8);
                }
                BuiltInId::MicVolumeUp => {
                    if let Ok(new_vol) = audio.shift_volume(10) {
                        sound.play(&action.sound, action.sound_volume);
                        if action.show_notification && cfg.notifications_enabled {
                            let text = replace_tokens(
                                if !action.notification.is_empty() {
                                    &action.notification
                                } else {
                                    built_in.default_notification
                                },
                                &action.name,
                                &get_hotkey_name(action.hotkey),
                                new_vol,
                                state.is_muted(),
                                state.is_bell_enabled(),
                                None,
                            );
                            state.post_notification(text);
                        }
                    }
                }
                BuiltInId::MicVolumeDown => {
                    if let Ok(new_vol) = audio.shift_volume(-10) {
                        sound.play(&action.sound, action.sound_volume);
                        if action.show_notification && cfg.notifications_enabled {
                            let text = replace_tokens(
                                if !action.notification.is_empty() {
                                    &action.notification
                                } else {
                                    built_in.default_notification
                                },
                                &action.name,
                                &get_hotkey_name(action.hotkey),
                                new_vol,
                                state.is_muted(),
                                state.is_bell_enabled(),
                                None,
                            );
                            state.post_notification(text);
                        }
                    }
                }
                BuiltInId::ToggleBellSound => {
                    let new_bell = !state.is_bell_enabled();
                    state.publish_bell_enabled(new_bell);
                    sound.play(&action.sound, action.sound_volume);
                    if action.show_notification && cfg.notifications_enabled {
                        let text = replace_tokens(
                            if !action.notification.is_empty() {
                                &action.notification
                            } else {
                                built_in.default_notification
                            },
                            &action.name,
                            &get_hotkey_name(action.hotkey),
                            state.volume_percent(),
                            state.is_muted(),
                            new_bell,
                            None,
                        );
                        state.post_notification(text);
                    }
                }
                BuiltInId::SwitchLanguage => {
                    Self::switch_input_language();
                    sound.play(&action.sound, action.sound_volume);
                }
            }
        } else if !action.command.is_empty() {
            // Custom command execution
            let cmd_str = action.command.clone();
            let notify_str = action.notification.clone();
            let show_notify = action.show_notification && cfg.notifications_enabled;
            let sound_src = action.sound.clone();
            let sound_vol = action.sound_volume;
            let action_name = action.name.clone();
            let hotkey = action.hotkey;
            let state_c = state.clone();

            sound.play(&sound_src, sound_vol);

            // Execute in background thread so worker is never blocked
            thread::spawn(move || {
                if notify_str.contains(TOKEN_STDOUT) {
                    let output = Command::new("cmd").args(["/C", &cmd_str]).output();
                    let out_text = match output {
                        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
                        Err(e) => format!("Error: {}", e),
                    };
                    if show_notify {
                        let formatted = replace_tokens(
                            &notify_str,
                            &action_name,
                            &get_hotkey_name(hotkey),
                            state_c.volume_percent(),
                            state_c.is_muted(),
                            state_c.is_bell_enabled(),
                            Some(&out_text),
                        );
                        state_c.post_notification(formatted);
                    }
                } else {
                    let _ = Command::new("cmd").args(["/C", &cmd_str]).spawn();
                    if show_notify && !notify_str.is_empty() {
                        let formatted = replace_tokens(
                            &notify_str,
                            &action_name,
                            &get_hotkey_name(hotkey),
                            state_c.volume_percent(),
                            state_c.is_muted(),
                            state_c.is_bell_enabled(),
                            None,
                        );
                        state_c.post_notification(formatted);
                    }
                }
            });
        }

        diag.record(record);
    }

    #[allow(clippy::manual_dangling_ptr)]
    fn switch_input_language() {
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                ActivateKeyboardLayout, HKL, KLF_ACTIVATE,
            };
            let _ = ActivateKeyboardLayout(HKL(1_isize as *mut _), KLF_ACTIVATE);
        }
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
        let mut handle = self.handle.lock();
        if let Some(h) = handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for ActionDispatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}
