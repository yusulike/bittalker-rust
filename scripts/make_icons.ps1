Add-Type -AssemblyName System.Drawing

$src = "C:\Users\yusuk\.gemini\antigravity\brain\4c8359ec-c2cf-4bc0-b83e-573f3c38b354\bittalker_icon_1793962922907.jpg"
if (-not (Test-Path $src)) {
    Write-Error "Source image not found: $src"
    exit 1
}

$orig = [System.Drawing.Bitmap]::FromFile($src)

# 1. Save 256x256 PNG
$png256 = New-Object System.Drawing.Bitmap(256, 256)
$g = [System.Drawing.Graphics]::FromImage($png256)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
$g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
$g.DrawImage($orig, 0, 0, 256, 256)
$g.Dispose()

if (-not (Test-Path "assets")) {
    New-Item -ItemType Directory -Path "assets" | Out-Null
}
$png256.Save("assets/icon.png", [System.Drawing.Imaging.ImageFormat]::Png)
Write-Host "Created assets/icon.png (256x256)"

# Also copy to target/release/assets/icon.png if exists
if (Test-Path "target/release/assets") {
    Copy-Item "assets/icon.png" "target/release/assets/icon.png" -Force
}
if (Test-Path "target/debug/assets") {
    Copy-Item "assets/icon.png" "target/debug/assets/icon.png" -Force
}

# 2. Build multi-resolution ICO file
$sizes = @(256, 128, 64, 48, 32, 16)
$pngBytesList = @()

foreach ($sz in $sizes) {
    $bmp = New-Object System.Drawing.Bitmap($sz, $sz)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($orig, 0, 0, $sz, $sz)
    $g.Dispose()

    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $pngBytesList += ,@($sz, $ms.ToArray())
    $ms.Dispose()
}
$orig.Dispose()

# Binary write ICO header & entries
$icoStream = [System.IO.File]::Create("assets/icon.ico")
$bw = New-Object System.IO.BinaryWriter($icoStream)

# ICONDIR header: 0, 1 (icon), count
$bw.Write([uint16]0)
$bw.Write([uint16]1)
$bw.Write([uint16]$sizes.Count)

$offset = 6 + ($sizes.Count * 16)
foreach ($item in $pngBytesList) {
    $sz = $item[0]
    $data = $item[1]
    $bWidth = if ($sz -ge 256) { 0 } else { [byte]$sz }
    $bHeight = if ($sz -ge 256) { 0 } else { [byte]$sz }
    $bw.Write([byte]$bWidth)
    $bw.Write([byte]$bHeight)
    $bw.Write([byte]0) # colors
    $bw.Write([byte]0) # reserved
    $bw.Write([uint16]1) # color planes
    $bw.Write([uint16]32) # bpp
    $bw.Write([uint32]$data.Length)
    $bw.Write([uint32]$offset)
    $offset += $data.Length
}

foreach ($item in $pngBytesList) {
    $data = $item[1]
    $bw.Write($data)
}

$bw.Flush()
$bw.Close()
$icoStream.Close()
Write-Host "Created assets/icon.ico (multi-resolution)"
