<#
.SYNOPSIS
  Replace the dev build's database with the newest snapshot of the real one,
  so features can be tried against realistic data without touching it.

.DESCRIPTION
  Reads %APPDATA%\com.journ.pomopipen\backups (written by the everyday app on
  every start) and copies the newest snapshot to
  %APPDATA%\com.journ.pomopipen.dev\pomopipen.db. The real data is only read.
#>
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$stableBackups = Join-Path $env:APPDATA 'com.journ.pomopipen\backups'
$devDir = Join-Path $env:APPDATA 'com.journ.pomopipen.dev'

$devRunning = Get-Process -Name 'pomopipen' -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -like "$repo\*" }
if ($devRunning) { throw 'Quit the dev build (PomoPipen Dev) first.' }

$latest = Get-ChildItem -Path $stableBackups -Filter 'pomopipen-*.db' -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1
if (-not $latest) { throw "No snapshot in $stableBackups yet - open the everyday PomoPipen once." }

New-Item -ItemType Directory -Force -Path $devDir | Out-Null
# A stale WAL next to a replaced database would corrupt it.
foreach ($f in 'pomopipen.db-wal', 'pomopipen.db-shm') {
    Remove-Item (Join-Path $devDir $f) -ErrorAction SilentlyContinue
}
Copy-Item $latest.FullName (Join-Path $devDir 'pomopipen.db') -Force
Write-Host "Dev database replaced with $($latest.Name)"
