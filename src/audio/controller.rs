//! Audio device controller managing Windows Core Audio (WASAPI) endpoint volume,
//! peak metering, mute state verification, and event notifications.

use crate::core::state::SharedState;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{implement, Result as WinResult, GUID};
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
    IAudioMeterInformation,
};
use windows::Win32::Media::Audio::{IMMDevice, AUDIO_VOLUME_NOTIFICATION_DATA};
use windows::Win32::System::Com::{CLSCTX_ALL, STGM_READ};
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("Device not initialized")]
    NotInitialized,
    #[error("Windows Core Audio COM error: {0}")]
    Com(#[from] windows::core::Error),
    #[error("Failed to verify mute state change: expected {expected}, actual {actual}")]
    VerificationFailed { expected: bool, actual: bool },
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeNotificationHandler {
    state: SharedState,
    generation: u64,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeNotificationHandler_Impl {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if pnotify.is_null() {
            return Ok(());
        }
        unsafe {
            let data = &*pnotify;
            let muted = data.bMuted.as_bool();
            let vol = (data.fMasterVolume * 100.0).clamp(0.0, 100.0).round() as u8;
            self.state.publish_mute_state(muted, self.generation, false);
            self.state.publish_volume_percent(vol);
        }
        Ok(())
    }
}

pub struct AudioDeviceController {
    state: SharedState,
    generation: u64,
    device: Option<IMMDevice>,
    volume_endpoint: Option<IAudioEndpointVolume>,
    meter_info: Option<IAudioMeterInformation>,
    callback: Option<IAudioEndpointVolumeCallback>,
    device_name: String,
    op_lock: Mutex<()>, // Serializes rapid mute/volume operations
    is_initialized: AtomicBool,
}

unsafe impl Send for AudioDeviceController {}
unsafe impl Sync for AudioDeviceController {}

impl AudioDeviceController {
    pub fn new(state: SharedState, generation: u64) -> Self {
        Self {
            state,
            generation,
            device: None,
            volume_endpoint: None,
            meter_info: None,
            callback: None,
            device_name: String::from("No device"),
            op_lock: Mutex::new(()),
            is_initialized: AtomicBool::new(false),
        }
    }

    /// Initializes from an activated IMMDevice.
    pub fn init_from_device(&mut self, device: IMMDevice) -> Result<(), AudioError> {
        let name = Self::query_device_name(&device).unwrap_or_else(|_| "Microphone".to_string());
        self.device_name = name.clone();

        unsafe {
            let volume_endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
            let meter_info: IAudioMeterInformation = device.Activate(CLSCTX_ALL, None)?;

            // Register volume notification callback
            let callback_impl = VolumeNotificationHandler {
                state: self.state.clone(),
                generation: self.generation,
            };
            let callback: IAudioEndpointVolumeCallback = callback_impl.into();
            volume_endpoint.RegisterControlChangeNotify(&callback)?;

            // Query initial mute state and volume level
            let initial_muted = volume_endpoint.GetMute()?.as_bool();
            let level_raw = volume_endpoint.GetMasterVolumeLevelScalar()?;
            let initial_vol = (level_raw * 100.0).clamp(0.0, 100.0).round() as u8;

            self.device = Some(device);
            self.volume_endpoint = Some(volume_endpoint);
            self.meter_info = Some(meter_info);
            self.callback = Some(callback);
            self.is_initialized.store(true, Ordering::Release);

            // Publish initial state authoritatively
            self.state.publish_device(true, name, self.generation);
            self.state
                .publish_mute_state(initial_muted, self.generation, true);
            self.state.publish_volume_percent(initial_vol);
        }

        Ok(())
    }

    fn query_device_name(device: &IMMDevice) -> WinResult<String> {
        unsafe {
            let prop_store = device.OpenPropertyStore(STGM_READ)?;
            // PKEY_Device_FriendlyName: {a45c254e-df1c-4efd-8020-67d146a850e0}, pid: 14
            let key = PROPERTYKEY {
                fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
                pid: 14,
            };
            let prop_var = prop_store.GetValue(&key as *const _)?;
            let name = prop_var.to_string();
            if name.is_empty() {
                Ok("Microphone".to_string())
            } else {
                Ok(name)
            }
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.is_initialized.load(Ordering::Acquire)
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Queries the actual Windows audio capture endpoint live mute state.
    /// Does NOT rely on any cached boolean.
    pub fn refresh_mute_state(&self) -> Result<bool, AudioError> {
        let endpoint = self
            .volume_endpoint
            .as_ref()
            .ok_or(AudioError::NotInitialized)?;
        unsafe {
            let live_muted = endpoint.GetMute()?.as_bool();
            self.state
                .publish_mute_state(live_muted, self.generation, true);
            Ok(live_muted)
        }
    }

    /// Authoritative microphone mute toggle operation:
    /// 1. Query live endpoint state via GetMute()
    /// 2. Determine target = !current
    /// 3. Request state via SetMute(target, null)
    /// 4. Query again via GetMute() to verify
    /// 5. Publish verified state
    ///
    /// Serialized via op_lock so rapid hotkey presses never race.
    pub fn toggle_mute(&self) -> Result<bool, AudioError> {
        let _guard = self.op_lock.lock();
        let endpoint = self
            .volume_endpoint
            .as_ref()
            .ok_or(AudioError::NotInitialized)?;

        unsafe {
            // 1. Live query
            let current = endpoint.GetMute()?.as_bool();

            // 2. Target opposite state
            let target = !current;

            // 3. Set target state
            endpoint.SetMute(target, std::ptr::null())?;

            // 4. Verify post-condition
            let verified = endpoint.GetMute()?.as_bool();

            if verified != target {
                return Err(AudioError::VerificationFailed {
                    expected: target,
                    actual: verified,
                });
            }

            // 5. Authoritatively publish verified state
            self.state
                .publish_mute_state(verified, self.generation, false);
            Ok(verified)
        }
    }

    /// Sets explicit mute state with verification (used for Push-to-Talk).
    pub fn set_mute(&self, target_mute: bool) -> Result<bool, AudioError> {
        let _guard = self.op_lock.lock();
        let endpoint = self
            .volume_endpoint
            .as_ref()
            .ok_or(AudioError::NotInitialized)?;

        unsafe {
            endpoint.SetMute(target_mute, std::ptr::null())?;

            let verified = endpoint.GetMute()?.as_bool();

            if verified != target_mute {
                return Err(AudioError::VerificationFailed {
                    expected: target_mute,
                    actual: verified,
                });
            }

            self.state
                .publish_mute_state(verified, self.generation, false);
            Ok(verified)
        }
    }

    /// Adjusts volume percent independently of mute.
    pub fn set_volume_percent(&self, percent: u8) -> Result<(), AudioError> {
        let _guard = self.op_lock.lock();
        let endpoint = self
            .volume_endpoint
            .as_ref()
            .ok_or(AudioError::NotInitialized)?;

        let clamped = percent.min(100);
        let scalar = clamped as f32 / 100.0;

        unsafe {
            endpoint.SetMasterVolumeLevelScalar(scalar, std::ptr::null())?;
            self.state.publish_volume_percent(clamped);
            Ok(())
        }
    }

    pub fn get_volume_percent(&self) -> Result<u8, AudioError> {
        let endpoint = self
            .volume_endpoint
            .as_ref()
            .ok_or(AudioError::NotInitialized)?;
        unsafe {
            let level = endpoint.GetMasterVolumeLevelScalar()?;
            let percent = (level * 100.0).clamp(0.0, 100.0).round() as u8;
            self.state.publish_volume_percent(percent);
            Ok(percent)
        }
    }

    pub fn shift_volume(&self, delta: i32) -> Result<u8, AudioError> {
        let current = self
            .get_volume_percent()
            .unwrap_or_else(|_| self.state.volume_percent());
        let target = (current as i32 + delta).clamp(0, 100) as u8;
        self.set_volume_percent(target)?;
        Ok(target)
    }

    /// Reads audio meter peak value (0.0 to 1.0) for the on-screen pill talking indicator.
    pub fn get_peak(&self) -> f32 {
        if let Some(meter) = &self.meter_info {
            unsafe {
                if let Ok(peak) = meter.GetPeakValue() {
                    return peak;
                }
            }
        }
        0.0
    }
}

impl Drop for AudioDeviceController {
    fn drop(&mut self) {
        if let (Some(endpoint), Some(callback)) = (&self.volume_endpoint, &self.callback) {
            unsafe {
                let _ = endpoint.UnregisterControlChangeNotify(callback);
            }
        }
    }
}
