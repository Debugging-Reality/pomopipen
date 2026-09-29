<#
.SYNOPSIS
  Build the current source and install it as the everyday PomoPipen.

.DESCRIPTION
  Dev builds (npm run tauri dev, plain tauri build) use the identifier
  com.journ.pomopipen.dev and therefore their own data folder. This script is
  the only thing that builds with tauri.stable.conf.json (identifier
  com.journ.pomopipen), i.e. the build that reads and writes the real records
  in %APPDATA%\com.journ.pomopipen.

  Safe to run while the everyday app is open: the running exe is renamed, not
  overwritten, and the new build takes over on the next launch. The previous
  build is kept as PomoPipen.prev.exe for rollback. The app itself snapshots
  its database into %APPDATA%\com.journ.pomopipen\backups on every start,
  before migrations run.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\publish-stable.ps1
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\publish-stable.ps1 -SkipChecks
#>
param(
    [string]$InstallDir = 'D:\Applications\PomoPipen',
    [switch]$SkipChecks
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$built = Join-Path $repo 'src-tauri\target\release\pomopipen.exe'
$stableConfig = Join-Path $repo 'src-tauri\tauri.stable.conf.json'
$exe = Join-Path $InstallDir 'PomoPipen.exe'
$prev = Join-Path $InstallDir 'PomoPipen.prev.exe'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'

# The Rust toolchain lives on D: (see docs/HANDOFF.md).
$env:RUSTUP_HOME = 'D:\DevTools\rustup'
$env:CARGO_HOME = 'D:\DevTools\cargo'
$env:PATH = "D:\DevTools\cargo\bin;$env:PATH"

function Invoke-Step([string]$name, [scriptblock]$block) {
    Write-Host "==> $name" -ForegroundColor Cyan
    & $block
    if ($LASTEXITCODE) { throw "$name failed (exit $LASTEXITCODE)" }
}

Push-Location $repo
try {
    if (-not $SkipChecks) {
        Invoke-Step 'npm run check' { npm run check }
        Invoke-Step 'cargo test' { cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 }
    }
    # A dev exe still running from target\release would block the linker.
    # Cargo hard-links it to deps\pomopipen.exe, so both names must go.
    # Windows allows renaming a running exe, so move locked ones aside.
    foreach ($path in $built, (Join-Path (Split-Path $built) 'deps\pomopipen.exe')) {
        if (Test-Path $path) {
            try { Remove-Item $path } catch { Move-Item $path "$path.in-use-$stamp" }
        }
    }
    Invoke-Step 'tauri build (stable identity)' {
        npm run tauri -- build --no-bundle --config $stableConfig
    }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
# Leftovers from earlier publishes; any still running are skipped and retried next time.
Get-ChildItem -Path $InstallDir -Filter 'PomoPipen.old-*.exe' | Remove-Item -ErrorAction SilentlyContinue
Get-ChildItem -Path (Split-Path $built) -Filter 'pomopipen.exe.in-use-*' -Recurse -Depth 1 |
    Remove-Item -ErrorAction SilentlyContinue
if (Test-Path $prev) { Move-Item $prev (Join-Path $InstallDir "PomoPipen.old-$stamp.exe") }
if (Test-Path $exe) { Move-Item $exe $prev }
Copy-Item $built $exe
# Never leave a stable-identity binary in target\: launching it by accident
# would open the real data. (Cargo just relinks it on the next build.)
Remove-Item $built, (Join-Path (Split-Path $built) 'deps\pomopipen.exe') -ErrorAction SilentlyContinue

# Start Menu shortcut (created once; left alone if it already exists).
$lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'PomoPipen.lnk'
if (-not (Test-Path $lnk)) {
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
    $shortcut.TargetPath = $exe
    $shortcut.WorkingDirectory = $InstallDir
    $shortcut.IconLocation = "$exe,0"
    $shortcut.Description = 'PomoPipen'
    $shortcut.Save()
    Write-Host "Created Start Menu shortcut: $lnk"
}

Write-Host ''
Write-Host "Installed $exe" -ForegroundColor Green
$running = Get-Process -Name 'PomoPipen' -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -like "$InstallDir\*" }
if ($running) {
    Write-Host 'The previous build is still open - quit it and reopen PomoPipen to switch.' -ForegroundColor Yellow
}
Write-Host 'Rollback: quit PomoPipen, delete PomoPipen.exe, rename PomoPipen.prev.exe to PomoPipen.exe.'
