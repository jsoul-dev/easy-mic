//! Periodic reconciliation worker.
//! Queries the Windows Core Audio endpoint every ~30 seconds to reconcile
//! any state divergence caused by physical microphone buttons or missed OS events.

use super::manager::AudioManager;
use crate::core::state::SharedState;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct ReconciliationWorker {
    running: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl ReconciliationWorker {
    pub const INTERVAL: Duration = Duration::from_secs(30);

    pub fn start(audio_manager: Arc<AudioManager>, state: SharedState) -> Arc<Self> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = thread::Builder::new()
            .name("easymic-reconciler".to_string())
            .spawn(move || {
                log::info!("Started 30-second audio state reconciliation worker");
                while running_clone.load(Ordering::Relaxed) {
                    // Sleep in 1-second chunks to exit promptly on shutdown
                    for _ in 0..Self::INTERVAL.as_secs() {
                        if !running_clone.load(Ordering::Relaxed) {
                            return;
                        }
                        thread::sleep(Duration::from_secs(1));
                    }

                    if !running_clone.load(Ordering::Relaxed) {
                        return;
                    }

                    // Query live endpoint mute state
                    match audio_manager.refresh_mute_state() {
                        Ok(actual_muted) => {
                            let published_muted = state.is_muted();
                            if actual_muted != published_muted {
                                log::warn!(
                                    "State discrepancy reconciled: published was {}, live endpoint is {}",
                                    published_muted,
                                    actual_muted
                                );
                                // Reconcile published state (silent update, don't play loud chime)
                                state.publish_mute_state(actual_muted, state.current_generation(), true);
                            }
                        }
                        Err(e) => {
                            log::debug!("Periodic reconciliation query skipped/failed: {}", e);
                        }
                    }
                }
                log::info!("Reconciliation worker stopped");
            })
            .expect("Failed to spawn reconciliation thread");

        Arc::new(Self {
            running,
            handle: Mutex::new(Some(handle)),
        })
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        let mut handle = self.handle.lock();
        if let Some(h) = handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for ReconciliationWorker {
    fn drop(&mut self) {
        self.stop();
    }
}
