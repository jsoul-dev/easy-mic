# Easy-Mic

Native Windows microphone indicator and push-to-talk utility rewritten in pure Rust (2021 edition) using `windows-rs`.
Priorities in order: input latency and minimal idle footprint (runs all day in the background), binary size (~1.3MB), memory safety.

## Build

```powershell
cargo build --release               # Outputs to target/release/Easy-Mic.exe
cargo test --release                # Runs unit and integration tests
cargo run --release                 # Builds and launches Easy-Mic
```

## Architecture & Layout

- `src/main.rs`: Entry point, logging setup, panic hook, crash reporting, message loop.
- `src/app/`: Single instance coordinator, registry startup integration, crash handler.
- `src/audio/`: WASAPI audio endpoints (`IAudioEndpointVolume`), background gain enforcement, and embedded WAV cues.
- `src/input/`: Low-level keyboard and mouse hooks, tap detection, key combination parser, and action dispatcher.
- `src/ui/`: Tray icon, DPI-aware on-screen overlay indicator, and Win32 Settings dialog.
- `src/core/`: Configuration management (`config.ini`), state representation, actions, and token interpolation.
- `resources/`: Application manifest, ICO files, and embedded WAV sound cues.
