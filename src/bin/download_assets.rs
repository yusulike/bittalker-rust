//! Standalone asset downloader CLI for BitTalker.
//!
//! Usage:
//!   cargo run --bin download-assets
//!   cargo run --bin download-assets -- /path/to/custom/assets

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let target_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        bittalker::paths::default_assets_dir()
    };

    println!("BitTalker Asset Downloader");
    println!("Target assets location: {}\n", target_dir.display());

    bittalker::assets::download_all(&target_dir)?;

    println!("\nDownload complete! You can now run bittalker.");
    Ok(())
}
