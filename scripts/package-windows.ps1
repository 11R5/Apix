# Builds the Windows release files into dist/:
#   Apix-<version>-windows-x64-setup.exe  the installer (Inno Setup 6)
#   Apix-<version>-windows-x64.zip        the same two programs, unpacked
#   apix-cli-<version>-windows-x64.zip    the command line alone (for the GitHub Action)
# Usage (from the repository root): scripts\package-windows.ps1 [-Version 1.2.3] [-NoBuild]
param(
    [string]$Version = "",
    [switch]$NoBuild
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $Version) {
    $Version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
}
if (-not $NoBuild) {
    cargo build --release -p apix-desktop -p apix-cli
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}
$bin = Join-Path $root "target\release"
$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Force $dist | Out-Null

$iscc = @(
    (Get-Command iscc -ErrorAction SilentlyContinue).Source,
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw "Inno Setup 6 (ISCC.exe) is not installed: winget install JRSoftware.InnoSetup" }
& $iscc "/DAppVersion=$Version" "/DBinDir=$bin" "/DOutDir=$dist" (Join-Path $root "packaging\windows\apix.iss")
if ($LASTEXITCODE -ne 0) { throw "iscc failed" }

$name = "Apix-$Version-windows-x64"
$stage = Join-Path $dist $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory $stage | Out-Null
Copy-Item "$bin\apix-desktop.exe", "$bin\apix.exe" $stage
Copy-Item LICENSE (Join-Path $stage "LICENSE.txt")
$zip = Join-Path $dist "$name.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Remove-Item -Recurse -Force $stage

# The command line alone, for the GitHub Action.
$cli = Join-Path $dist "apix-cli-$Version-windows-x64.zip"
if (Test-Path $cli) { Remove-Item $cli }
Compress-Archive -Path (Join-Path $bin "apix.exe") -DestinationPath $cli
Get-ChildItem $dist -Filter "*-$Version-windows-*" | ForEach-Object { "{0}  {1:N1} MB" -f $_.Name, ($_.Length / 1MB) }
