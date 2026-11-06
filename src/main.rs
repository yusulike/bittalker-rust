#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! BitTalker (Rust): real-time Bitcoin price monitor with desktop UI and
//! voice alerts.

use bittalker::assets;
use bittalker::paths;
use bittalker::settings;
use bittalker::app;
use eframe::egui;

fn app_icon() -> Option<std::sync::Arc<egui::IconData>> {
    let bytes = include_bytes!("../assets/icon.png");
    image::load_from_memory(bytes).ok().map(|img| {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        std::sync::Arc::new(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    })
}

fn main() -> eframe::Result<()> {
    // 1. Handle CLI commands
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--download-assets" | "-d" | "download-assets" => {
                let target = if args.len() > 2 {
                    std::path::PathBuf::from(&args[2])
                } else {
                    paths::default_assets_dir()
                };
                if let Err(e) = assets::download_all(&target) {
                    eprintln!("Asset download failed: {e:#}");
                    std::process::exit(1);
                }
                println!("\nAssets ready at: {}", target.display());
                return Ok(());
            }
            "--screenshot" => {
                let path = if args.len() > 2 {
                    std::path::PathBuf::from(&args[2])
                } else {
                    std::path::PathBuf::from("screenshot.png")
                };
                let _ = rustls::crypto::ring::default_provider().install_default();
                let options = eframe::NativeOptions {
                    viewport: egui::ViewportBuilder::default()
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_inner_size(egui::vec2(520.0, 100.0)),
                    ..Default::default()
                };
                return eframe::run_native(
                    "Bitcoin Ticker",
                    options,
                    Box::new(move |cc| Ok(Box::new(app::BitTalkerApp::new_with_options(cc, Some(path), Some(false), false)))),
                );
            }
            "--screenshot-ticker" => {
                let path = if args.len() > 2 {
                    std::path::PathBuf::from(&args[2])
                } else {
                    std::path::PathBuf::from("screenshot_ticker.png")
                };
                let _ = rustls::crypto::ring::default_provider().install_default();
                let options = eframe::NativeOptions {
                    viewport: egui::ViewportBuilder::default()
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_inner_size(egui::vec2(540.0, 45.0)),
                    ..Default::default()
                };
                return eframe::run_native(
                    "Bitcoin Ticker",
                    options,
                    Box::new(move |cc| Ok(Box::new(app::BitTalkerApp::new_with_options(cc, Some(path), Some(true), false)))),
                );
            }
            "--open-settings" => {
                let _ = rustls::crypto::ring::default_provider().install_default();
                let options = eframe::NativeOptions {
                    viewport: egui::ViewportBuilder::default()
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_inner_size(egui::vec2(520.0, 100.0)),
                    ..Default::default()
                };
                return eframe::run_native(
                    "Bitcoin Ticker",
                    options,
                    Box::new(|cc| Ok(Box::new(app::BitTalkerApp::new_with_options(cc, None, None, true)))),
                );
            }
            "--check-assets" => {
                let assets_dir = paths::assets_dir();
                match assets::verify_assets(&assets_dir) {
                    Ok(()) => {
                        println!("All assets verified at: {}", assets_dir.display());
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                }
            }
            "--help" | "-h" => {
                println!("BitTalker (Rust) - Bitcoin Real-time Monitor & Voice Alerts");
                println!("\nUsage:");
                println!("  bittalker                  Run GUI application");
                println!("  bittalker --download-assets [PATH]  Download TTS models to [PATH] (default: [exe_dir]/assets)");
                println!("  bittalker --check-assets   Verify asset files");
                return Ok(());
            }
            _ => {}
        }
    }

    // 2. Install the rustls crypto provider (ring) before any TLS connections (e.g. Binance WS)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 3. Check assets status
    let assets_dir = paths::assets_dir();
    if let Err(e) = assets::verify_assets(&assets_dir) {
        println!("[Assets Warning] {}", e);
        println!("[Assets Guide] To download models automatically from HuggingFace, run:");
        println!("               cargo run --bin download-assets");
        println!("               or: bittalker --download-assets\n");
    }

    // 4. Settings decide the initial window shape (ticker vs normal layout and
    // the always-on-top flag), mirroring MainWindow.__init__.
    let settings = settings::SettingsManager::new(paths::settings_path());
    let s = settings.get().clone();

    let mut viewport = egui::ViewportBuilder::default()
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size(if s.ticker_mode {
            egui::vec2(540.0, 45.0)
        } else {
            egui::vec2(520.0, 100.0)
        })
        .with_min_inner_size(egui::vec2(320.0, 40.0));
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let viewport = if s.always_on_top {
        viewport.with_always_on_top()
    } else {
        viewport
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Bitcoin Ticker",
        options,
        Box::new(|cc| Ok(Box::new(app::BitTalkerApp::new(cc)))),
    )
}
