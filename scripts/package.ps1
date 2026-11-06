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
            & "$releaseDir\bittalker.exe" --download-assets $releaseAssets
        }
    } else {
        Write-Host "Assets already present in target/release/assets."
    }
}

# Ensure icons are in release assets
if (Test-Path "$projectRoot\assets\icon.ico") {
    if (-not (Test-Path $releaseAssets)) {
        New-Item -ItemType Directory -Path $releaseAssets -Force | Out-Null
    }
    Copy-Item "$projectRoot\assets\icon.ico" $releaseAssets -Force
    Copy-Item "$projectRoot\assets\icon.png" $releaseAssets -Force
}

Write-Host "`n=== 3. Packaging Distribution Bundles ===" -ForegroundColor Cyan
$distDir = Join-Path $projectRoot $OutputDir
if (-not (Test-Path $distDir)) {
    New-Item -ItemType Directory -Path $distDir -Force | Out-Null
}

$ortDll = Join-Path $releaseDir "onnxruntime.dll"
if (-not (Test-Path $ortDll)) {
    $debugOrt = Join-Path $projectRoot "target\debug\onnxruntime.dll"
    if (Test-Path $debugOrt) {
        Copy-Item $debugOrt $releaseDir
    }
}

# --- 3A. Slim Bundle (without assets) ---
$slimName = "bittalker-windows-x64"
$slimDir = Join-Path $distDir $slimName
if (Test-Path $slimDir) {
    Remove-Item $slimDir -Recurse -Force
}
New-Item -ItemType Directory -Path $slimDir -Force | Out-Null

Copy-Item (Join-Path $releaseDir "bittalker.exe") $slimDir
if (Test-Path $ortDll) {
    Copy-Item $ortDll $slimDir
}

@'
BitTalker (Rust) - Real-time Bitcoin Price Monitor with Voice Alerts (Slim Edition)

Directory Structure:
  bittalker.exe          - Main GUI application
  onnxruntime.dll        - ONNX Runtime dynamic library
  README.txt             - This guide

Voice Models (TTS):
  This slim edition does NOT include the ~70MB voice models by default.
  To enable voice alerts:
    1. Open the application (`bittalker.exe`)
    2. Click the Settings icon (⚙)
    3. Click [⬇ Download Models (~70MB)]
    The models will be downloaded and activated automatically without restarting.
  Alternatively, you can run:
    bittalker.exe --download-assets
'@ | Set-Content -Path (Join-Path $slimDir "README.txt") -Encoding UTF8

$slimZip = Join-Path $distDir "$slimName.zip"
if (Test-Path $slimZip) {
    Remove-Item $slimZip -Force
}
Write-Host "Creating Slim archive: $slimZip"
Compress-Archive -Path "$slimDir\*" -DestinationPath $slimZip -Force

# --- 3B. Full Bundle (with assets) ---
$fullName = "bittalker-windows-x64-full"
$fullDir = Join-Path $distDir $fullName
if (Test-Path $fullDir) {
    Remove-Item $fullDir -Recurse -Force
}
New-Item -ItemType Directory -Path $fullDir -Force | Out-Null

Copy-Item (Join-Path $releaseDir "bittalker.exe") $fullDir
if (Test-Path $ortDll) {
    Copy-Item $ortDll $fullDir
}
if (Test-Path $releaseAssets) {
    Copy-Item $releaseAssets $fullDir -Recurse -Force
}

@'
BitTalker (Rust) - Real-time Bitcoin Price Monitor with Voice Alerts (Full Edition)

Directory Structure:
  bittalker.exe          - Main GUI application
  onnxruntime.dll        - ONNX Runtime dynamic library
  assets/                - TTS models and voice style configs
  README.txt             - This guide

How to run:
  Double-click `bittalker.exe` to run.
  All voice models are pre-installed in the `assets/` folder.
  If assets are ever corrupted or missing, you can re-download them in Settings (⚙)
  or run:
    bittalker.exe --download-assets
'@ | Set-Content -Path (Join-Path $fullDir "README.txt") -Encoding UTF8

$fullZip = Join-Path $distDir "$fullName.zip"
if (Test-Path $fullZip) {
    Remove-Item $fullZip -Force
}
Write-Host "Creating Full archive: $fullZip"
Compress-Archive -Path "$fullDir\*" -DestinationPath $fullZip -Force

Write-Host "`n[SUCCESS] Bundles successfully created:" -ForegroundColor Green
Write-Host "  Slim: $slimZip" -ForegroundColor Green
Write-Host "  Full: $fullZip" -ForegroundColor Green
