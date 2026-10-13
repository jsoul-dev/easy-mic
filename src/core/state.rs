//! Central authoritative state model for EasyMic.

use crossbeam_channel::{Receiver, Sender};
use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum StateChangeEvent {
    MuteChanged { muted: bool, silent: bool },
    VolumeChanged { percent: u8 },
    BellChanged { enabled: bool },
    DeviceChanged { has_device: bool, name: String },
    SessionCountChanged { count: i32 },
    NotificationPosted { text: String },
}

pub struct AuthoritativeState {
    mic_muted: AtomicBool,
    mic_volume_percent: AtomicU8,
    bell_enabled: AtomicBool,
    has_device: AtomicBool,
    device_name: RwLock<String>,
    active_sessions_count: AtomicI32,
    endpoint_generation: AtomicU64,
    listeners: Mutex<Vec<Sender<StateChangeEvent>>>,
}

impl Default for AuthoritativeState {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthoritativeState {
    pub fn new() -> Self {
        Self {
            mic_muted: AtomicBool::new(false),
            mic_volume_percent: AtomicU8::new(100),
            bell_enabled: AtomicBool::new(true),
            has_device: AtomicBool::new(false),
            device_name: RwLock::new(String::from("No device")),
            active_sessions_count: AtomicI32::new(0),
            endpoint_generation: AtomicU64::new(1),
            listeners: Mutex::new(Vec::new()),
        }
    }

    pub fn subscribe(&self) -> Receiver<StateChangeEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut listeners = self.listeners.lock();
        listeners.push(tx);
        rx
    }

    fn broadcast(&self, event: StateChangeEvent) {
        let mut listeners = self.listeners.lock();
        listeners.retain(|tx| tx.send(event.clone()).is_ok());
    }

    pub fn current_generation(&self) -> u64 {
        self.endpoint_generation.load(Ordering::Acquire)
    }

    pub fn increment_generation(&self) -> u64 {
        self.endpoint_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn is_muted(&self) -> bool {
        self.mic_muted.load(Ordering::Acquire)
    }

    pub fn volume_percent(&self) -> u8 {
        self.mic_volume_percent.load(Ordering::Acquire)
    }

    pub fn is_bell_enabled(&self) -> bool {
        self.bell_enabled.load(Ordering::Acquire)
    }

    pub fn has_device(&self) -> bool {
        self.has_device.load(Ordering::Acquire)
    }

    pub fn device_name(&self) -> String {
        self.device_name.read().clone()
    }

    pub fn active_sessions(&self) -> i32 {
        self.active_sessions_count.load(Ordering::Acquire)
    }

    /// Authoritative update of microphone mute state.
    /// If generation does not match the active generation, update is dropped as obsolete.
    pub fn publish_mute_state(&self, muted: bool, generation: u64, silent: bool) -> bool {
        let current_gen = self.endpoint_generation.load(Ordering::Acquire);
        if generation != 0 && generation < current_gen {
            log::debug!(
                "Dropping obsolete mute update (gen {} < {})",
                generation,
                current_gen
            );
            return false;
        }

        let old = self.mic_muted.swap(muted, Ordering::Release);
        if old != muted || !silent {
            self.broadcast(StateChangeEvent::MuteChanged { muted, silent });
        }
        true
    }

    pub fn publish_volume_percent(&self, percent: u8) {
        let old = self.mic_volume_percent.swap(percent, Ordering::Release);
        if old != percent {
            self.broadcast(StateChangeEvent::VolumeChanged { percent });
        }
    }

    pub fn publish_bell_enabled(&self, enabled: bool) {
        let old = self.bell_enabled.swap(enabled, Ordering::Release);
        if old != enabled {
            self.broadcast(StateChangeEvent::BellChanged { enabled });
        }
    }

    pub fn publish_device(&self, has_device: bool, name: String, generation: u64) {
        self.endpoint_generation
            .store(generation, Ordering::Release);
        self.has_device.store(has_device, Ordering::Release);
        *self.device_name.write() = name.clone();
        self.broadcast(StateChangeEvent::DeviceChanged { has_device, name });
    }

    pub fn publish_sessions_count(&self, count: i32) {
        self.active_sessions_count.store(count, Ordering::Release);
        self.broadcast(StateChangeEvent::SessionCountChanged { count });
    }

    pub fn post_notification(&self, text: String) {
        self.broadcast(StateChangeEvent::NotificationPosted { text });
    }
}

pub type SharedState = Arc<AuthoritativeState>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_broadcast() {
        let state = AuthoritativeState::new();
        let rx = state.subscribe();

        assert!(!state.is_muted());
        state.publish_mute_state(true, 1, false);
        assert!(state.is_muted());

        let event = rx.try_recv().expect("event received");
        assert_eq!(
            event,
            StateChangeEvent::MuteChanged {
                muted: true,
                silent: false
            }
        );
    }

    #[test]
    fn test_obsolete_generation_dropped() {
        let state = AuthoritativeState::new();
        state.increment_generation(); // generation is now 2
        let rx = state.subscribe();

        // Update with older generation 1 should be dropped
        let accepted = state.publish_mute_state(true, 1, false);
        assert!(!accepted);
        assert!(!state.is_muted());
        assert!(rx.try_recv().is_err());
    }
}
