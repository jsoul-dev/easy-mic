//! Asynchronous sound playback system and sound catalog.
//! Supports bundled sounds ("Mute", "Unmute", "Tick", "Enable", "Disable")
//! and custom sound cues ("on.wav", "off.wav", and arbitrary WAV paths).

use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Media::Audio::{
    waveOutSetVolume, PlaySoundW, HWAVEOUT, SND_ASYNC, SND_FLAGS, SND_MEMORY, SND_NODEFAULT,
};

// Embedded assets
const BYTES_MUTE: &[u8] = include_bytes!("../../../src/Resources/mute.wav");
const BYTES_UNMUTE: &[u8] = include_bytes!("../../../src/Resources/unmute.wav");
const BYTES_ENABLE: &[u8] = include_bytes!("../../../src/Resources/enable.wav");
const BYTES_DISABLE: &[u8] = include_bytes!("../../../src/Resources/disable.wav");

// Embedded sound cues
const BYTES_ON: &[u8] = include_bytes!("../../../src/Resources/on.wav");
const BYTES_OFF: &[u8] = include_bytes!("../../../src/Resources/off.wav");

#[derive(Debug, Clone)]
pub struct BundledSound {
    pub key: &'static str,
    pub title: &'static str,
    pub bytes: &'static [u8],
}

pub const BUNDLED_SOUNDS: &[BundledSound] = &[
    BundledSound {
        key: "Off",
        title: "Off chime",
        bytes: BYTES_OFF,
    },
    BundledSound {
        key: "On",
        title: "On chime",
        bytes: BYTES_ON,
    },
    BundledSound {
        key: "Mute",
        title: "Discord mute",
        bytes: BYTES_MUTE,
    },
    BundledSound {
        key: "Unmute",
        title: "Discord unmute",
        bytes: BYTES_UNMUTE,
    },
    BundledSound {
        key: "Disable",
        title: "Disable",
        bytes: BYTES_DISABLE,
    },
    BundledSound {
        key: "Enable",
        title: "Enable",
        bytes: BYTES_ENABLE,
    },
];

pub fn find_bundled_sound(key: &str) -> Option<&'static BundledSound> {
    let k = key.trim();
    if k.eq_ignore_ascii_case("off") || k.eq_ignore_ascii_case("off.wav") {
        return BUNDLED_SOUNDS.iter().find(|s| s.key == "Off");
    }
    if k.eq_ignore_ascii_case("on") || k.eq_ignore_ascii_case("on.wav") {
        return BUNDLED_SOUNDS.iter().find(|s| s.key == "On");
    }
    BUNDLED_SOUNDS
        .iter()
        .find(|s| s.key.eq_ignore_ascii_case(k))
}

struct PlayCommand {
    sound_source: String,
    volume_percent: u8,
}

pub struct SoundManager {
    sender: Sender<PlayCommand>,
    running: Arc<AtomicBool>,
    worker_handle: Mutex<Option<JoinHandle<()>>>,
}

impl SoundManager {
    pub fn new() -> Arc<Self> {
        let (sender, receiver) = bounded::<PlayCommand>(32);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = thread::Builder::new()
            .name("easymic-sound-worker".to_string())
            .spawn(move || {
                Self::playback_loop(receiver, running_clone);
            })
            .expect("Failed to spawn sound playback thread");

        Arc::new(Self {
            sender,
            running,
            worker_handle: Mutex::new(Some(handle)),
        })
    }

    /// Asynchronously requests sound playback.
    /// NEVER blocks or delays the caller.
    pub fn play(&self, sound_source: &str, volume_percent: u8) {
        if sound_source.is_empty() || volume_percent == 0 {
            return;
        }

        let cmd = PlayCommand {
            sound_source: sound_source.to_string(),
            volume_percent,
        };

        // try_send ensures this call never blocks, even if queue is saturated
        if let Err(e) = self.sender.try_send(cmd) {
            log::warn!("Sound playback queue busy: {}", e);
        }
    }

    fn playback_loop(receiver: Receiver<PlayCommand>, running: Arc<AtomicBool>) {
        // Cache memory buffers for playing custom files so they stay valid during async playback
        let mut cached_buffers: HashMap<String, Vec<u8>> = HashMap::new();

        while running.load(Ordering::Relaxed) {
            match receiver.recv() {
                Ok(cmd) => {
                    Self::execute_play(&cmd.sound_source, cmd.volume_percent, &mut cached_buffers);
                }
                Err(_) => break, // Channel closed
            }
        }
    }

    fn execute_play(
        sound_source: &str,
        volume_percent: u8,
        cached_buffers: &mut HashMap<String, Vec<u8>>,
    ) {
        let clamped_vol = volume_percent.min(100);
        let level = (clamped_vol as u32 * 0xFFFF) / 100;
        let dword_vol = level | (level << 16);

        unsafe {
            let _ = waveOutSetVolume(HWAVEOUT::default(), dword_vol);
        }

        // 1. Check if it's a bundled sound or Off / On
        if let Some(bundled) = find_bundled_sound(sound_source) {
            Self::play_static_bytes(bundled.bytes);
            return;
        }

        // 2. Resolve as disk file
        if let Some(path) = Self::resolve_sound_path(sound_source) {
            if let Ok(data) = fs::read(&path) {
                Self::play_memory_buffer(&data, sound_source, cached_buffers);
                return;
            }
        }

        // 3. Fallback: if sound_source is "on.wav" or ends with "on.wav" or "Unmute"
        let lower = sound_source.to_ascii_lowercase();
        if lower.ends_with("on.wav") || lower == "on" || lower == "unmute" {
            Self::play_static_bytes(BYTES_ON);
            return;
        }
        if lower.ends_with("off.wav") || lower == "off" || lower == "mute" {
            Self::play_static_bytes(BYTES_OFF);
            return;
        }

        log::debug!(
            "Could not find sound file '{}', staying silent",
            sound_source
        );
    }

    fn play_static_bytes(bytes: &'static [u8]) {
        unsafe {
            let flags: SND_FLAGS = SND_ASYNC | SND_MEMORY | SND_NODEFAULT;
            let ptr = PCWSTR(bytes.as_ptr() as *const u16);
            let _ = PlaySoundW(ptr, HMODULE::default(), flags);
        }
    }

    fn play_memory_buffer(data: &[u8], key: &str, cached_buffers: &mut HashMap<String, Vec<u8>>) {
        // Keep buffer alive in cache for the duration of async playback
        cached_buffers.insert(key.to_string(), data.to_vec());
        if let Some(buf) = cached_buffers.get(key) {
            unsafe {
                let flags: SND_FLAGS = SND_ASYNC | SND_MEMORY | SND_NODEFAULT;
                let ptr = PCWSTR(buf.as_ptr() as *const u16);
                let _ = PlaySoundW(ptr, HMODULE::default(), flags);
            }
        }
    }

    pub fn resolve_sound_path(source: &str) -> Option<PathBuf> {
        let direct = Path::new(source);
        if direct.exists() && direct.is_file() {
            return Some(direct.to_path_buf());
        }

        // Check relative to executable directory
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let exe_relative = parent.join(source);
                if exe_relative.exists() && exe_relative.is_file() {
                    return Some(exe_relative);
                }
                let in_sounds = parent.join("sounds").join(source);
                if in_sounds.exists() && in_sounds.is_file() {
                    return Some(in_sounds);
                }
            }
        }

        // Check relative to current working directory
        let in_cwd_sounds = Path::new("sounds").join(source);
        if in_cwd_sounds.exists() && in_cwd_sounds.is_file() {
            return Some(in_cwd_sounds);
        }

        None
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
        let mut handle = self.worker_handle.lock();
        if let Some(h) = handle.take() {
            // Wake up thread if waiting
            let _ = self.sender.try_send(PlayCommand {
                sound_source: String::new(),
                volume_percent: 0,
            });
            let _ = h.join();
        }
    }
}

impl Drop for SoundManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_sounds_presence() {
        assert!(find_bundled_sound("Off").is_some());
        assert!(find_bundled_sound("On").is_some());
        assert!(find_bundled_sound("Mute").is_some());
        assert!(find_bundled_sound("Unmute").is_some());
        assert!(find_bundled_sound("Disable").is_some());
        assert!(find_bundled_sound("Enable").is_some());
        assert!(find_bundled_sound("on.wav").is_some());
        assert!(find_bundled_sound("off.wav").is_some());
        assert!(find_bundled_sound("Tick").is_none());
    }

    #[test]
    fn test_sound_manager_queue_nonblocking() {
        let sm = SoundManager::new();
        // Sending commands should be non-blocking
        sm.play("Off", 50);
        sm.play("Nonexistent", 100);
        sm.shutdown();
    }
}
