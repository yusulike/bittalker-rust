Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

Add-Type @"
using System;
using System.Runtime.InteropServices;

public struct RECT {
    public int Left;
    public int Top;
    public int Right;
    public int Bottom;
}

public class Win32Window {
    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
}
"@

$artifactDir = "C:\Users\yusuk\.gemini\antigravity\brain\4c8359ec-c2cf-4bc0-b83e-573f3c38b354"
if (-not (Test-Path $artifactDir)) {
    New-Item -ItemType Directory -Path $artifactDir -Force | Out-Null
}

$appProc = Get-Process -Name "bittalker" -ErrorAction SilentlyContinue
$startedHere = $false

if (-not $appProc) {
    Write-Host "Starting bittalker.exe..."
    $appProc = Start-Process -FilePath "E:\myWork\bittalker-rust\target\debug\bittalker.exe" -PassThru
    $startedHere = $true
    Start-Sleep -Seconds 4
} else {
    Write-Host "bittalker.exe is already running (PID: $($appProc.Id))."
    Start-Sleep -Seconds 1
}

$hwnd = [Win32Window]::FindWindow($null, "Bitcoin Ticker")
if ($hwnd -eq [IntPtr]::Zero) {
    # Try finding by process MainWindowHandle
    $appProc = Get-Process -Name "bittalker" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($appProc -and $appProc.MainWindowHandle -ne [IntPtr]::Zero) {
        $hwnd = $appProc.MainWindowHandle
    }
}

$windowShotPath = Join-Path $artifactDir "window_screenshot.png"
$fullShotPath = Join-Path $artifactDir "full_screenshot.png"

# 1. Fullscreen Capture
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmpFull = New-Object System.Drawing.Bitmap($screen.Width, $screen.Height)
$gfxFull = [System.Drawing.Graphics]::FromImage($bmpFull)
$gfxFull.CopyFromScreen($screen.Location, [System.Drawing.Point]::Empty, $screen.Size)
$bmpFull.Save($fullShotPath, [System.Drawing.Imaging.ImageFormat]::Png)
$gfxFull.Dispose()
$bmpFull.Dispose()
Write-Host "Full screen captured to: $fullShotPath"

# 2. Window Capture
if ($hwnd -ne [IntPtr]::Zero) {
    [Win32Window]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 200

    $rect = New-Object RECT
    if ([Win32Window]::GetWindowRect($hwnd, [ref]$rect)) {
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top

        Write-Host "Window bounds: Left=$($rect.Left), Top=$($rect.Top), Width=$width, Height=$height"

        if ($width -gt 0 -and $height -gt 0) {
            # Add small padding around window to capture border/shadow
            $pad = 20
            $cropLeft = [Math]::Max(0, $rect.Left - $pad)
            $cropTop = [Math]::Max(0, $rect.Top - $pad)
            $cropWidth = $width + ($pad * 2)
            $cropHeight = $height + ($pad * 2)

            $bmpWin = New-Object System.Drawing.Bitmap($cropWidth, $cropHeight)
            $gfxWin = [System.Drawing.Graphics]::FromImage($bmpWin)
            $gfxWin.CopyFromScreen([System.Drawing.Point]::new($cropLeft, $cropTop), [System.Drawing.Point]::Empty, [System.Drawing.Size]::new($cropWidth, $cropHeight))
            $bmpWin.Save($windowShotPath, [System.Drawing.Imaging.ImageFormat]::Png)
            $gfxWin.Dispose()
            $bmpWin.Dispose()
            Write-Host "Window captured to: $windowShotPath"
        }
    }
} else {
    Write-Host "Could not find 'Bitcoin Ticker' window handle; using full screenshot."
}

if ($startedHere -and $appProc) {
    Write-Host "Stopping temporarily started bittalker.exe..."
    Stop-Process -Id $appProc.Id -Force -ErrorAction SilentlyContinue
}
