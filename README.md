# BitTalker (Rust)

A lightweight, high-performance real-time Bitcoin (BTC/USDT) desktop price monitor and voice alert application written in Rust, ported from the original Python implementation ([yusulike/bittalker](https://github.com/yusulike/bittalker)).

[한국어 문서 (README.ko.md)](README.ko.md)

---

## Overview

BitTalker tracks real-time Bitcoin trades directly from the Binance WebSocket stream. Whenever price crosses user-configured threshold intervals (e.g. every \$50, \$100, etc.), it synthesizes and plays natural voice announcements using an embedded ONNX Supertonic-3 TTS engine.

This repository is a pure Rust port designed to replace the original Python (PyQt6 + ONNX) version, delivering:
- **Zero Python Runtime Dependency**: Runs as a single native executable without Python or external interpreters.
- **Ultra-lightweight Footprint**: ~13 MB binary size with pure OpenGL (`glow`) backend and dynamically loaded CPU-only ONNX Runtime (no DirectML or bulky GPU runtime overhead).
- **Subsystem GUI Mode**: No black terminal/console window on launch in release builds.
- **Multi-Monitor Window Memory**: Restores exact window coordinates across restarts, including negative coordinate spaces in multi-display setups.
- **Relative Settings Placement**: Automatically positions the settings dialog adjacent to the main ticker window.

---

## Features

- **Real-Time Binance WebSocket Stream**: Direct tick-by-tick monitoring of `btcusdt@trade` with automatic reconnection.
- **Retro Desktop Digital Clock & Hourly Chime**:
  - Flip-clock card design showing Hours, Minutes, Seconds, AM/PM, and full date.
  - Live real-time clock updates in both Normal and Compact Ticker modes.
  - Pleasant hourly audio chime on every hour.
- **Embedded Supertonic-3 ONNX TTS**: On-device neural speech synthesis supporting 5 languages (Korean, English, Spanish, Portuguese, French) and 10 voice styles (F1–F5, M1–M5).
- **Configurable Alert Thresholds**: Customizable crossing intervals (e.g., every \$50, \$100, \$500) with anti-oscillation filtering to prevent duplicate triggers.
- **Frameless egui Desktop UI**:
  - Two switchable view modes: **Normal** (clock + price + controls) and **Compact Ticker** (single-line bar).
  - Drag-and-drop repositioning anywhere on the screen.
  - Multi-monitor coordinate memory across application restarts.
  - Always-on-top toggle and quick audio mute button.
  - Independent native settings dialog centered relative to the main window.
- **Built-in Model Downloader**: CLI tool and flag to fetch models directly from HuggingFace without requiring Python.

---

## Directory & Asset Layout

The application expects the `assets/` directory to reside directly adjacent to the executable:

```text
[Installation Directory]/
├── bittalker.exe          # Main GUI application
├── download-assets.exe    # Standalone asset downloader CLI
├── onnxruntime.dll        # CPU-only ONNX Runtime dynamic library
├── assets/                # TTS models and voice configurations
│   ├── config.json
│   ├── onnx/
│   │   ├── duration_predictor.onnx
│   │   ├── text_encoder.onnx
│   │   ├── tts.json
│   │   ├── unicode_indexer.json
│   │   ├── vector_estimator.onnx
│   │   └── vocoder.onnx
│   └── voice_styles/
│       ├── F1.json ~ F5.json
│       └── M1.json ~ M5.json
├── cache/                 # Audio cache for generated TTS clips (auto-generated)
└── settings.json          # User preferences and saved window position (auto-generated)
```

---

## Getting Started

### Prerequisites
- [Rust toolchain](https://rustup.rs/) (edition 2021)
- Windows 10/11 x64

### 1. Download TTS Model Assets (One-Time)
Download the required neural TTS model files (~250 MB) directly from HuggingFace:

```bash
cargo run --bin download-assets
```
*(By default, files are placed into `target/debug/assets` or `target/release/assets`.)*

Alternatively, use the main executable CLI flag:
```bash
cargo run -- --download-assets
```

### 2. Run Application
```bash
cargo run
```

### 3. Run Tests
Execute the full test suite (37 unit & integration tests):
```bash
cargo test
```

---

## Release Build & Packaging

Build an optimized release binary:
```bash
cargo build --release
```

To generate a standalone portable ZIP distribution:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

The script packages `target/release/bittalker.exe`, `onnxruntime.dll`, `download-assets.exe`, and the `assets/` directory into `dist/bittalker-windows-x64.zip`. Unzipping this archive gives you a fully functional, self-contained portable installation ready to run on any 64-bit Windows machine.

---

## CLI Options

| Option | Description |
|---|---|
| *(none)* | Launches the main GUI application |
| `--download-assets [PATH]` | Downloads TTS model files to the specified directory (default: `[exe_dir]/assets`) |
| `--check-assets` | Validates presence of all required model and configuration files |
| `--open-settings` | Launches the GUI and immediately opens the settings dialog |
| `--screenshot [PATH]` | Headless/automated screenshot of the normal UI layout |
| `--screenshot-ticker [PATH]` | Headless/automated screenshot of the compact ticker layout |
| `--help`, `-h` | Prints command line help |

---

## References

- [Supertonic](https://github.com/supertone-inc/supertonic)

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
