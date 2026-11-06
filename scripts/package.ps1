<#
.SYNOPSIS
    Build and package BitTalker for Windows distribution.

.DESCRIPTION
    1. Compiles in release mode (`cargo build --release`)
    2. Downloads or copies TTS model assets directly under `target/release/assets`
    3. Bundles `bittalker.exe` + `assets/` into a portable `dist/bittalker-v<version>-windows-x64.zip`
#>

param(
    [switch]$SkipAssetDownload,
    [string]$OutputDir = "dist"
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$projectRoot = Split-Path -Parent $scriptDir
Set-Location $projectRoot

Write-Host "=== 1. Building BitTalker (Release) ===" -ForegroundColor Cyan
cargo build --release --bins

$releaseDir = Join-Path $projectRoot "target\release"
$releaseAssets = Join-Path $releaseDir "assets"

Write-Host "`n=== 2. Ensuring Assets under [exe_dir]/assets ===" -ForegroundColor Cyan
if (-not $SkipAssetDownload) {
    if (-not (Test-Path (Join-Path $releaseAssets "onnx\tts.json"))) {
        # Check if debug assets exist first to save bandwidth
        $debugAssets = Join-Path $projectRoot "target\debug\assets"
        if (Test-Path (Join-Path $debugAssets "onnx\tts.json")) {
            Write-Host "Copying cached assets from debug build..."
            Copy-Item -Path $debugAssets -Destination $releaseAssets -Recurse -Force
        } else {
            Write-Host "Downloading assets directly from HuggingFace..."
            & "$releaseDir\download-assets.exe" $releaseAssets
        }
    } else {
        Write-Host "Assets already present in target/release/assets."
    }
}

Write-Host "`n=== 3. Packaging Distribution Bundle ===" -ForegroundColor Cyan
$distDir = Join-Path $projectRoot $OutputDir
$bundleName = "bittalker-windows-x64"
$bundleDir = Join-Path $distDir $bundleName

if (Test-Path $bundleDir) {
    Remove-Item $bundleDir -Recurse -Force
}
New-Item -ItemType Directory -Path $bundleDir -Force | Out-Null

# Copy executable, dlls, and assets
Copy-Item (Join-Path $releaseDir "bittalker.exe") $bundleDir
Copy-Item (Join-Path $releaseDir "download-assets.exe") $bundleDir

# Bundle CPU-only ONNX Runtime DLL (no DirectML.dll needed!)
$ortDll = Join-Path $releaseDir "onnxruntime.dll"
if (-not (Test-Path $ortDll)) {
    $debugOrt = Join-Path $projectRoot "target\debug\onnxruntime.dll"
    if (Test-Path $debugOrt) {
        Copy-Item $debugOrt $releaseDir
    }
}
if (Test-Path $ortDll) {
    Copy-Item $ortDll $bundleDir
    Write-Host "Bundled onnxruntime.dll into package."
}

if (Test-Path $releaseAssets) {
    Copy-Item $releaseAssets $bundleDir -Recurse -Force
}

# Create README for end-users
@"
BitTalker (Rust) - Real-time Bitcoin Price Monitor with Voice Alerts

Directory Structure:
  bittalker.exe          - Main GUI application
  download-assets.exe    - Standalone TTS models downloader
  assets/                - TTS models and voice style configs

How to run:
  Double-click `bittalker.exe` to run.
  If assets are missing or corrupted, run `download-assets.exe` to re-download.
"@ | Set-Content -Path (Join-Path $bundleDir "README.txt") -Encoding UTF8

# Compress to ZIP
$zipPath = Join-Path $distDir "$bundleName.zip"
if (Test-Path $zipPath) {
    Remove-Item $zipPath -Force
}
Write-Host "Creating archive: $zipPath"
Compress-Archive -Path "$bundleDir\*" -DestinationPath $zipPath -Force

Write-Host "`n[SUCCESS] Package successfully created at: $zipPath" -ForegroundColor Green
