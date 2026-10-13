# Easy-Mic

[![Rust](https://img.shields.io/badge/language-Rust_2021-DEA584?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Windows_10%2F11-0078D4?style=flat-square&logo=windows)](https://microsoft.com/windows)
[![License](https://img.shields.io/badge/license-Apache_2.0-blue?style=flat-square)](LICENSE.txt)
[![Fork](https://img.shields.io/badge/fork_of-k1tbyte%2Feasymic-orange?style=flat-square&logo=github)](https://github.com/k1tbyte/easymic)
[![Release](https://img.shields.io/github/v/release/jsoul-dev/easy-mic?style=flat-square)](https://github.com/jsoul-dev/easy-mic/releases)

**Easy-Mic** is a high-performance, ultra-lightweight desktop application for global, system-level microphone control on Windows. It eliminates the hassle of memorizing disjointed hotkeys across Teams, Discord, OBS, Slack, and games by centralizing microphone state management directly at the Windows hardware audio endpoint level.

> [!NOTE]  
> **Easy-Mic is a modern native Rust rewrite and enhancement of [k1tbyte/easymic](https://github.com/k1tbyte/easymic).**  
> Re-architected in pure Rust using `windows-rs` for zero-overhead WinAPI bindings, rock-solid memory safety, clean INI configuration, and sub-millisecond response latency.

---

## Features

### Universal Microphone Management
- **Hardware-Level Mute Synchronisation**: Communicates directly with Windows Core Audio COM endpoints (`IAudioEndpointVolume`). If another application or physical button toggles your mic, Easy-Mic reflects and maintains the authoritative state in real time.
- **Persistent Volume Enforcement**: Optional background volume keeper locks your hardware microphone gain at your configured level (e.g. 100%) against background apps that automatically adjust sensitivity.
- **Periodic Endpoint Reconciliation**: A non-intrusive 30-second background worker ensures device state integrity even across USB headset disconnects and sleep/wake cycles.

### Low-Overhead Global Hotkeys
- **Dedicated Input Subsystem**: Processes keyboard and mouse input on an isolated low-level hook thread, dispatching actions in sub-millisecond time without introducing input lag to games or apps.
- **Zero Mouse Lag Under Saturated CPU**: In the legacy C++ version, an always-active global low-level mouse hook (`WH_MOUSE_LL`) intercepted every single mouse event, causing severe cursor stuttering and lag whenever CPU usage spiked. Easy-Mic completely eliminates this by conditionally skipping the mouse hook unless mouse buttons are explicitly bound to actions.
- **Flexible Action Triggers**:
  - **Toggle Mute** — Instant one-touch state toggle (default: `Shift + Z`)
  - **Push to Talk / Mute** — Hold-to-talk or hold-to-mute with key release detection
  - **Multi-Press & Tap-Only** — Distinguish single, double, or triple taps, with modifiers that double as standard keys
  - **Custom Shell Commands** — Launch scripts or applications directly from hotkey events

### On-Screen Visual Indicator
- **State Overlay**: Floating status pill rendered over full-screen applications with zero flicker:
  - `Hidden` — No indicator overlay
  - `Muted` — Appears only when microphone is muted
  - `Muted or Talking` — Dynamic voice activity detection via real-time RMS audio analysis
- **Fully Customizable**:
  - Configurable pixel size (default `15px`)
  - Adjustable voice activity threshold
  - Interactive drag-and-drop repositioning while the Settings dialog is open
  - **Exclude from Capture**: Hides the overlay from OBS Studio, Discord screen share, and Windows Game Bar

### Crisp Audio Feedback
- **Embedded Audio Presets**: High-fidelity sound cues embedded directly within `Easy-Mic.exe` (no loose sound files required):
  - `Off`, `On`, `Mute`, `Unmute`, `Disable`, `Enable`, and `None`
- **Custom Sound Support**: Full support for personal `.wav` audio files with long-path hover tooltips in the settings interface.
- **Independent Volume Control**: Action and feedback sounds feature dedicated volume adjustment separate from master output.

### Human-Readable Configuration (`config.ini`)
- **Clean Standard INI**: No opaque binary blobs. Stored beside `Easy-Mic.exe` with structured sections: `[General]`, `[Indicator]`, `[Sounds]`, and `[Action.N]`.
- **Legacy Migration Support**: Seamlessly recognizes and imports legacy `conf.b` (Glaze BEVE binary) and `easymic.d` from original versions of EasyMic, migrating settings into `config.ini` automatically.

---

## Rust Architecture vs. Legacy C++

| Feature | Legacy C++ Version | Easy-Mic (Native Rust) |
| :--- | :--- | :--- |
| **Language** | C++23 | **Rust 2021 Edition** |
| **WinAPI Bindings** | Raw Win32 C headers | **`windows-rs` 0.58 (Type-safe COM & Win32)** |
| **Mouse Cursor Lag** | Global `WH_MOUSE_LL` hook lagged cursor when CPU saturated | **Zero mouse lag: `WH_MOUSE_LL` skipped unless mouse hotkeys bound** |
| **Config Format** | Binary Glaze BEVE (`conf.b`) | **Clean INI (`config.ini`) with auto-migration** |
| **Embedded Audio** | External folder dependencies | **Embedded directly in binary (`include_bytes!`)** |
| **Memory Footprint** | ~3–5 MB | **~1.5–2 MB Resident Set Size** |
| **Threading Model** | Ad-hoc threads | **Isolated COM MTA + dedicated hook/audio channels** |
| **Safety Guarantees** | Manual pointer/resource management | **Borrow checker + RAII COM smart pointers** |

---

## Quick Start

### 1. Download
Grab the latest release from the [Releases](https://github.com/jsoul-dev/easy-mic/releases) page:
* `Easy-Mic.exe` — Standalone portable executable (creates its default `config.ini` automatically beside the exe)

### 2. Run
Place `Easy-Mic.exe` in any folder of your choice and run it.  
Easy-Mic will appear in your **System Tray** (near the clock).

### 3. Configure
* **Double-click** the tray icon or **Right-click → Settings** to open the preferences window.
* Switch tabs to customize hotkeys, sounds, overlay size, and autostart behavior.
* While the Settings window is open, simply drag the on-screen microphone indicator to position it anywhere on your monitors.

---

## Building from Source

### Prerequisites
- **Windows 10 / 11**
- [Rust & Cargo](https://rustup.rs/) (version 1.75 or later)

### Build Steps

```powershell
# Clone the repository
git clone https://github.com/jsoul-dev/easy-mic.git
cd easy-mic

# Navigate to the Rust workspace
cd easymic-rust

# Build optimized release binary
cargo build --release

# The compiled executable is generated at:
# target/release/Easy-Mic.exe
```

### Running Test Suite

```powershell
cargo test --release
```

---

## Credits & Acknowledgments

* **Original Author**: [k1tbyte](https://github.com/k1tbyte) for the original [Easymic](https://github.com/k1tbyte/easymic) concept, icon designs, and audio assets.
* **Rust Rewrite & Rebranding**: [jsoul-dev](https://github.com/jsoul-dev) & the Antigravity Team for the ground-up Rust architecture, Win32 COM modernization, and user experience refinements.

---

## License

This project is licensed under the **Apache-2.0 License** — see the [LICENSE.txt](LICENSE.txt) file for details.
