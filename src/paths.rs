//! Data-directory resolution.
//!
//! Standard structure for build & distribution:
//!   [exe_dir]/
//!     bittalker.exe
//!     assets/
//!       config.json
//!       onnx/
//!         tts.json, *.onnx, unicode_indexer.json
//!       voice_styles/
//!         F1.json ... M5.json
//!     cache/
//!     settings.json
//!
//! Search order:
//! 1. `BITTALKER_ROOT` environment variable (explicit override)
//! 2. `[exe_dir]/assets` (standard release/distribution layout)
//! 3. `[cwd]/assets` (running from a custom directory)
//! 4. Parent directories of executable (dev mode: `target/debug/../../assets`)

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

fn has_assets(dir: &Path) -> bool {
    dir.join("assets").join("onnx").join("tts.json").is_file()
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The default location where assets are expected to be installed:
/// directly alongside the running executable: `[exe_dir]/assets`
pub fn default_assets_dir() -> PathBuf {
    exe_dir().join("assets")
}

fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    // 1. Explicit override
    if let Ok(root) = std::env::var("BITTALKER_ROOT") {
        out.push(PathBuf::from(root));
    }

    // 2. Standard layout: executable directory
    let exe_base = exe_dir();
    out.push(exe_base.clone());

    // 3. Current working directory
    if let Ok(cwd) = std::env::current_dir() {
        if cwd != exe_base {
            out.push(cwd);
        }
    }

    // 4. Dev mode fallback: climb parents of exe (e.g. target/debug/../../ -> project root)
    let mut dir = exe_base.parent().map(|p| p.to_path_buf());
    let mut depth = 0;
    while let Some(d) = dir {
        out.push(d.clone());
        dir = d.parent().map(|p| p.to_path_buf());
        depth += 1;
        if depth > 4 {
            break;
        }
    }

    out
}

/// The resolved project root holding `assets/`, `cache/` and `settings.json`.
/// Defaults to `exe_dir()` so assets are created/expected right next to the executable.
pub fn project_root() -> &'static Path {
    ROOT.get_or_init(|| {
        candidates()
            .into_iter()
            .find(|c| has_assets(c))
            .unwrap_or_else(exe_dir)
    })
}

pub fn assets_dir() -> PathBuf {
    project_root().join("assets")
}

pub fn onnx_dir() -> PathBuf {
    assets_dir().join("onnx")
}

pub fn voice_styles_dir() -> PathBuf {
    assets_dir().join("voice_styles")
}

pub fn cache_dir() -> PathBuf {
    project_root().join("cache")
}

pub fn settings_path() -> PathBuf {
    project_root().join("settings.json")
}

pub fn onnxruntime_dylib_path() -> Option<PathBuf> {
    let name = format!("{}onnxruntime{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX);
    let candidates = [
        exe_dir().join(&name),
        assets_dir().join(&name),
        onnx_dir().join(&name),
        project_root().join(&name),
        PathBuf::from("target/debug").join(&name),
        PathBuf::from("target/release").join(&name),
    ];
    for path in &candidates {
        if path.is_file() {
            return Some(path.clone());
        }
    }
    None
}
