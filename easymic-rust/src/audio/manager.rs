//! Audio manager: enumerates default audio capture endpoint, manages device switches,
//! and provides safe generation-tracked access to the active AudioDeviceController.

use super::controller::{AudioDeviceController, AudioError};
use crate::core::state::SharedState;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::core::{implement, PCWSTR};
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, EDataFlow, ERole, IMMDeviceEnumerator,
    IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, DEVICE_STATE,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

#[implement(IMMNotificationClient)]
struct DeviceNotificationClient {
    tx: crossbeam_channel::Sender<()>,
}

impl IMMNotificationClient_Impl for DeviceNotificationClient_Impl {
    fn OnDeviceStateChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _dwnewstate: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _pwstrdefaultdeviceid: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flow == eCapture && (role == eCommunications || role == eConsole) {
            let _ = self.tx.try_send(());
        }
        Ok(())
    }
    fn OnPropertyValueChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _key: &PROPERTYKEY,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

pub struct AudioManager {
    state: SharedState,
    enumerator: Option<IMMDeviceEnumerator>,
    active_controller: RwLock<Arc<AudioDeviceController>>,
    notification_client: Option<IMMNotificationClient>,
    running: Arc<AtomicBool>,
}

unsafe impl Send for AudioManager {}
unsafe impl Sync for AudioManager {}

impl AudioManager {
    pub fn new(state: SharedState) -> Arc<Self> {
        let dummy = Arc::new(AudioDeviceController::new(
            state.clone(),
            state.current_generation(),
        ));
        let running = Arc::new(AtomicBool::new(true));

        let mut enumerator = None;
        let mut notification_client = None;
        let (change_tx, change_rx) = crossbeam_channel::bounded::<()>(8);

        unsafe {
            if let Ok(enum_obj) =
                CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            {
                let client_impl = DeviceNotificationClient { tx: change_tx };
                let client: IMMNotificationClient = client_impl.into();
                let _ = enum_obj.RegisterEndpointNotificationCallback(&client);
                notification_client = Some(client);
                enumerator = Some(enum_obj);
            } else {
                log::error!("Failed to create IMMDeviceEnumerator");
            }
        }

        let manager = Arc::new(Self {
            state: state.clone(),
            enumerator,
            active_controller: RwLock::new(dummy),
            notification_client,
            running: running.clone(),
        });

        // Initialize default device initially
        manager.reinitialize_default_device();

        // Spawn background thread to handle default device changes with debouncing
        let manager_weak = Arc::downgrade(&manager);
        thread::Builder::new()
            .name("easymic-device-watcher".to_string())
            .spawn(move || {
                while running.load(Ordering::Relaxed) {
                    if let Ok(()) = change_rx.recv() {
                        // Windows announces new default before it can actually be activated
                        thread::sleep(Duration::from_millis(150));
                        // Drain any coalesced changes
                        while change_rx.try_recv().is_ok() {}

                        if let Some(mgr) = manager_weak.upgrade() {
                            log::info!("Default capture device change detected, reinitializing");
                            mgr.reinitialize_default_device();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            })
            .expect("Failed to spawn device watcher thread");

        manager
    }

    pub fn reinitialize_default_device(&self) {
        let enumerator = match &self.enumerator {
            Some(e) => e,
            None => return,
        };

        let new_gen = self.state.increment_generation();
        let mut new_controller = AudioDeviceController::new(self.state.clone(), new_gen);

        unsafe {
            // Prefer eCommunications for capture (mic), fallback to eConsole
            let device_res = enumerator
                .GetDefaultAudioEndpoint(eCapture, eCommunications)
                .or_else(|_| enumerator.GetDefaultAudioEndpoint(eCapture, eConsole));

            match device_res {
                Ok(device) => {
                    if let Err(e) = new_controller.init_from_device(device) {
                        log::error!("Failed to initialize device: {}", e);
                    } else {
                        let arc_ctrl = Arc::new(new_controller);
                        let mut active = self.active_controller.write();
                        *active = arc_ctrl;
                        log::info!(
                            "Successfully initialized audio capture device: {}",
                            active.device_name()
                        );
                    }
                }
                Err(e) => {
                    log::warn!("No capture device found: {}", e);
                    self.state
                        .publish_device(false, "No device".to_string(), new_gen);
                }
            }
        }
    }

    pub fn active_controller(&self) -> Arc<AudioDeviceController> {
        self.active_controller.read().clone()
    }

    pub fn toggle_mute(&self) -> Result<bool, AudioError> {
        self.active_controller().toggle_mute()
    }

    pub fn set_mute(&self, mute: bool) -> Result<bool, AudioError> {
        self.active_controller().set_mute(mute)
    }

    pub fn refresh_mute_state(&self) -> Result<bool, AudioError> {
        self.active_controller().refresh_mute_state()
    }

    pub fn shift_volume(&self, delta: i32) -> Result<u8, AudioError> {
        self.active_controller().shift_volume(delta)
    }

    pub fn set_volume_percent(&self, percent: u8) -> Result<(), AudioError> {
        self.active_controller().set_volume_percent(percent)
    }

    pub fn get_volume_percent(&self) -> Result<u8, AudioError> {
        self.active_controller().get_volume_percent()
    }

    pub fn get_peak(&self) -> f32 {
        self.active_controller().get_peak()
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
        if let (Some(enumerator), Some(client)) = (&self.enumerator, &self.notification_client) {
            unsafe {
                let _ = enumerator.UnregisterEndpointNotificationCallback(client);
            }
        }
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}
