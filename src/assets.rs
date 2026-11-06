//! Asset management and downloader for Supertonic TTS models.
//!
//! Downloads models directly from HuggingFace (`Supertone/supertonic-3`)
//! without needing Python or external tools. Supports build & distribution
//! packaging where `assets/` resides right next to the executable.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::{bail, Context, Result};

pub const HF_BASE_URL: &str = "https://huggingface.co/Supertone/supertonic-3/resolve/main";

pub const REQUIRED_FILES: &[&str] = &[
    "config.json",
    "onnx/tts.json",
    "onnx/unicode_indexer.json",
    "onnx/duration_predictor.onnx",
    "onnx/text_encoder.onnx",
    "onnx/vector_estimator.onnx",
    "onnx/vocoder.onnx",
    "voice_styles/F1.json",
    "voice_styles/F2.json",
    "voice_styles/F3.json",
    "voice_styles/F4.json",
    "voice_styles/F5.json",
    "voice_styles/M1.json",
    "voice_styles/M2.json",
    "voice_styles/M3.json",
    "voice_styles/M4.json",
    "voice_styles/M5.json",
];

/// Returns `Ok(())` if all required files exist in `assets_dir`.
pub fn verify_assets(assets_dir: &Path) -> Result<()> {
    let mut missing = Vec::new();
    for rel_path in REQUIRED_FILES {
        let p = assets_dir.join(rel_path);
        if !p.is_file() {
            missing.push(*rel_path);
        }
    }
    if !missing.is_empty() {
        bail!("Missing {} required asset(s): {:?}", missing.len(), missing);
    }
    Ok(())
}

/// Download all missing or incomplete assets to `assets_dir`.
pub fn download_all(assets_dir: &Path) -> Result<()> {
    download_all_with_progress(assets_dir, |_, _, _, _| ())
}

/// Download all missing or incomplete assets to `assets_dir` with a progress callback.
///
/// Callback signature: `(rel_path, file_index, total_files, file_progress_fraction)`
/// where `file_index` is 1-based (1..=total_files) and `file_progress_fraction` is 0.0..=1.0.
pub fn download_all_with_progress<F>(assets_dir: &Path, mut on_progress: F) -> Result<()>
where
    F: FnMut(&str, usize, usize, f32),
{
    println!("=== BitTalker Asset Downloader ===");
    println!("Target directory: {}", assets_dir.display());
    println!("Source: {}\n", HF_BASE_URL);

    fs::create_dir_all(assets_dir)?;
    fs::create_dir_all(assets_dir.join("onnx"))?;
    fs::create_dir_all(assets_dir.join("voice_styles"))?;

    let total_files = REQUIRED_FILES.len();
    for (i, rel_path) in REQUIRED_FILES.iter().enumerate() {
        let target_path = assets_dir.join(rel_path);
        let url = format!("{}/{}", HF_BASE_URL, rel_path);

        print!("[{}/{}] {} ... ", i + 1, total_files, rel_path);
        io::stdout().flush().ok();

        if target_path.is_file() {
            if let Ok(metadata) = fs::metadata(&target_path) {
                if metadata.len() > 0 {
                    println!("already present ({:.1} MB)", metadata.len() as f64 / 1_048_576.0);
                    on_progress(rel_path, i + 1, total_files, 1.0);
                    continue;
                }
            }
        }

        on_progress(rel_path, i + 1, total_files, 0.0);
        download_file(&url, &target_path, &mut |downloaded, total| {
            let pct = if let Some(tot) = total {
                if tot > 0 {
                    (downloaded as f32 / tot as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            on_progress(rel_path, i + 1, total_files, pct);
        })
        .with_context(|| format!("failed downloading {rel_path} from {url}"))?;
        on_progress(rel_path, i + 1, total_files, 1.0);
    }

    println!("\nVerification...");
    verify_assets(assets_dir)?;
    println!("All assets successfully verified in: {}", assets_dir.display());
    Ok(())
}

fn download_file(
    url: &str,
    dest: &Path,
    on_chunk: &mut dyn FnMut(u64, Option<u64>),
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_dest = dest.with_extension(format!("tmp.{}", std::process::id()));

    let mut response = ureq::get(url)
        .call()
        .with_context(|| format!("HTTP GET failed for {url}"))?;

    let content_len = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let mut reader = response.body_mut().as_reader();
    let mut file = File::create(&tmp_dest)?;

    let mut buffer = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;
    let start = Instant::now();
    let mut last_print = Instant::now();

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        downloaded += n as u64;

        if last_print.elapsed().as_millis() >= 100 {
            last_print = Instant::now();
            on_chunk(downloaded, content_len);
            if let Some(total) = content_len {
                let pct = (downloaded as f64 / total as f64) * 100.0;
                print!("\r... {:.1} MB / {:.1} MB ({:.0}%)",
                    downloaded as f64 / 1_048_576.0,
                    total as f64 / 1_048_576.0,
                    pct,
                );
            } else {
                print!("\r... {:.1} MB", downloaded as f64 / 1_048_576.0);
            }
            io::stdout().flush().ok();
        }
    }

    on_chunk(downloaded, content_len);
    file.flush()?;
    drop(file);

    // Atomically replace target
    fs::rename(&tmp_dest, dest)?;

    let elapsed = start.elapsed().as_secs_f64();
    let mb = downloaded as f64 / 1_048_576.0;
    let speed = if elapsed > 0.0 { mb / elapsed } else { 0.0 };
    println!("\r... done! ({:.1} MB, {:.1} MB/s)", mb, speed);

    Ok(())
}
