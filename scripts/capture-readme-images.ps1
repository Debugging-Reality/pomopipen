<#
.SYNOPSIS
  Regenerates the README screenshots in .github/images/ from the visual review
  harness (tests/preview, sample data only).

.DESCRIPTION
  Start the harness first:
    node node_modules/vite/bin/vite.js --config tests/preview/vite.config.js
  (it serves http://127.0.0.1:1422). Then run:
    powershell -ExecutionPolicy Bypass -File scripts\capture-readme-images.ps1

  Uses headless Microsoft Edge. Banners come from tests/preview/readme.html
  (English, Chinese, Japanese text); app screenshots are taken with the
  English and Chinese UI. No Classic Tomato timer photos appear: those are
  placeholders and must not be published.
#>
param(
    [string]$OutDir = (Join-Path $PSScriptRoot '..\.github\images'),
    [double]$Scale = 1.5
)
$ErrorActionPreference = 'Stop'
$edge = "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe"
if (-not (Test-Path $edge)) { $edge = "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe" }
$base = 'http://127.0.0.1:1422'
try { Invoke-WebRequest -UseBasicParsing -Uri $base -TimeoutSec 5 | Out-Null }
catch { throw "The preview harness is not running on $base (see the script header)." }

$profileDir = Join-Path $env:TEMP 'pomopipen-readme-capture'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path

# name | viewport | path + query
$shots = @()
foreach ($l in 'en', 'zh', 'ja') {
    $shots += "hero-$l|1280x640|/readme.html?slide=hero&lang=$l"
    $shots += "mini-$l|1280x440|/readme.html?slide=mini&lang=$l"
}
foreach ($l in 'en', 'zh') {
    $shots += "calendar-$l|1240x1000|/?view=calendar&chrome=none&lang=$l&bg=none"
    $shots += "stats-$l|1000x1032|/?view=stats&tab=charts&lang=$l&bg=none"
    $shots += "tasks-$l|620x800|/?view=tasks&lang=$l&bg=none"
    # Narrower than ~760 px the harness wraps the two windows and headless Edge
    # paints almost nothing, so shoot wider and crop to the two windows.
    $shots += "jots-$l|800x520|/?view=jots&chrome=none&lang=$l&bg=none&theme=cherry-soda|720"
    $shots += "sync-$l|620x640|/?view=sync&gcal=connected&chrome=none&lang=$l&bg=none"
}

Add-Type -AssemblyName System.Drawing
function Crop-Width([string]$file, [int]$cssWidth) {
    $src = [System.Drawing.Image]::FromFile($file)
    $rect = New-Object System.Drawing.Rectangle 0, 0, ([int]($cssWidth * $Scale)), $src.Height
    $bmp = (New-Object System.Drawing.Bitmap $src).Clone($rect, $src.PixelFormat)
    $src.Dispose()
    $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

foreach ($s in $shots) {
    $name, $size, $path, $cropWidth = $s -split '\|', 4
    $w, $h = $size -split 'x'
    $out = Join-Path $OutDir "$name.png"
    # --run-all-compositor-stages-before-draw: without it headless Edge sometimes
    # misses the bottom strip of the page.
    # Start-Process joins arguments with spaces, so quote the paths (the repo may live under one).
    $a = @('--headless=new', '--disable-gpu', '--hide-scrollbars', "--user-data-dir=`"$profileDir`"",
        "--force-device-scale-factor=$Scale", '--run-all-compositor-stages-before-draw',
        '--virtual-time-budget=8000', "--window-size=$w,$h", "--screenshot=`"$out`"", "`"$base$path`"")
    Start-Process -FilePath $edge -ArgumentList $a -Wait -WindowStyle Hidden
    if ($cropWidth) { Crop-Width $out ([int]$cropWidth) }
    '{0,-14} {1,6:N0} KB' -f $name, ((Get-Item $out).Length / 1KB)
}
