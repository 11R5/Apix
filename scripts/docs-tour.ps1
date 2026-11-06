# Takes the user guide's pictures in every language:
#
#   powershell -ExecutionPolicy Bypass -File scripts\docs-tour.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\docs-tour.ps1 -Languages en,tr -Only main,network
#
# It builds Apix and the example servers, starts the servers, then runs the
# desktop app once per language with APIX_TOUR=Docs\tour\scenes.toml: the
# app sets up each scene and writes its name into a "ready" file, this script
# takes the picture of the window (PrintWindow, so other windows in front do
# not matter) and deletes the file. The app keeps its settings in a fresh
# folder (APIX_CONFIG_DIR), so yours are not touched. The pictures go to
# Docs\<language>\images\<scene>.png.
param(
    [string[]]$Languages = @("en", "tr", "de", "es", "fr", "ru", "ja", "zh-CN", "ar"),
    [string[]]$Only = @(),
    [switch]$NoBuild
)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$work = Join-Path $env:TEMP "apix-tour"
New-Item -ItemType Directory -Force $work | Out-Null

if (-not $NoBuild) {
    cargo build --release -p apix-desktop -p apix-cli -p shrink-png
    if ($LASTEXITCODE) { throw "build failed" }
    cargo build -p apix-http --example ws_server --example sse_server --example graphql_server --example socketio_server --example soap_server
    cargo build -p apix-grpc --example grpc_server
    cargo build -p apix-proto --example mqtt_broker
    cargo build -p apix-route --example demo_geo
    if ($LASTEXITCODE) { throw "building the example servers failed" }
}

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class TourWin {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

# The window without its invisible resize border, even when covered.
function Save-Window([IntPtr]$hwnd, [string]$file) {
    $outer = New-Object TourWin+RECT
    $inner = New-Object TourWin+RECT
    [TourWin]::GetWindowRect($hwnd, [ref]$outer) | Out-Null
    [TourWin]::DwmGetWindowAttribute($hwnd, 9, [ref]$inner, 16) | Out-Null
    $w = $outer.Right - $outer.Left; $h = $outer.Bottom - $outer.Top
    $full = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($full)
    $dc = $g.GetHdc()
    [TourWin]::PrintWindow($hwnd, $dc, 2) | Out-Null   # PW_RENDERFULLCONTENT
    $g.ReleaseHdc($dc); $g.Dispose()
    $crop = New-Object System.Drawing.Rectangle ($inner.Left - $outer.Left), ($inner.Top - $outer.Top), ($inner.Right - $inner.Left), ($inner.Bottom - $inner.Top)
    $shot = $full.Clone($crop, $full.PixelFormat)
    $shot.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    $shot.Dispose(); $full.Dispose()
}

# Made once for every language: a git repository with changes, an
# encrypted copy and a demonstration map database for the route.
$apix = Join-Path $repo "target\release\apix.exe"
$demo = Join-Path $repo "Docs\tour\demo"
$gitDemo = Join-Path $work "git-demo"
if (-not (Test-Path $gitDemo)) {
    Copy-Item -Recurse $demo $gitDemo
    git -C $gitDemo init -q -b main
    git -C $gitDemo -c user.name="Apix docs" -c user.email="docs@example.com" add -A
    git -C $gitDemo -c user.name="Apix docs" -c user.email="docs@example.com" commit -q -m "Demo API"
    git -C $gitDemo checkout -q -b signup
    $create = Join-Path $gitDemo "users\create-user.apix"
    [System.IO.File]::WriteAllText($create, ([System.IO.File]::ReadAllText($create) -replace "ada@example.com", "ada.lovelace@example.com"), (New-Object System.Text.UTF8Encoding $false))
    [System.IO.File]::WriteAllText((Join-Path $gitDemo "users\delete-user.apix"), "name = `"Delete user`"`nmethod = `"DELETE`"`nurl = `"{{base_url}}/users/{{user_id}}`"`n", (New-Object System.Text.UTF8Encoding $false))
}
$encrypted = Join-Path $work "encrypted"
if (-not (Test-Path $encrypted)) {
    $env:APIX_PASSWORD = "docs-tour"
    & $apix encrypt $demo --out $encrypted | Out-Null
    Remove-Item Env:APIX_PASSWORD
}
$geo = Join-Path $work "geo.mmdb"
if (-not (Test-Path $geo)) {
    # The real route to the demo API's server, placed on a made-up path.
    $hops = tracert -d -h 14 -w 700 jsonplaceholder.typicode.com | Select-String -Pattern "\d+\.\d+\.\d+\.\d+\s*$" | ForEach-Object { ($_.Line.Trim() -split "\s+")[-1] }
    $places = @("41.01,28.98,Istanbul,TR", "41.01,28.98,Istanbul,TR", "42.70,23.32,Sofia,BG", "44.43,26.10,Bucharest,RO", "48.21,16.37,Vienna,AT", "50.11,8.68,Frankfurt,DE", "50.11,8.68,Frankfurt,DE", "52.37,4.90,Amsterdam,NL", "51.51,-0.13,London,GB", "51.51,-0.13,London,GB")
    $geoArgs = @($geo)
    $i = 0
    foreach ($ip in $hops) {
        if ($ip -match "^(10|127|192\.168|172\.(1[6-9]|2\d|3[01]))\.") { continue }
        $geoArgs += "$ip=$($places[[Math]::Min($i, $places.Count - 1)])"
        $i++
    }
    & (Join-Path $repo "target\debug\examples\demo_geo.exe") @geoArgs | Out-Null
}

# The example servers the demo collection talks to.
$servers = @()
$examples = Join-Path $repo "target\debug\examples"
foreach ($name in "ws_server", "sse_server", "graphql_server", "socketio_server", "soap_server", "grpc_server", "mqtt_broker") {
    $servers += Start-Process -FilePath (Join-Path $examples "$name.exe") -WindowStyle Hidden -PassThru
}
Start-Sleep -Seconds 2

$utf8 = New-Object System.Text.UTF8Encoding $false
try {
    foreach ($lang in $Languages) {
        $config = Join-Path $work "config-$lang"
        $out = Join-Path $work "out-$lang"
        Remove-Item -Recurse -Force $config, $out -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force $config, $out | Out-Null
        $settings = "language = `"$lang`"`ntransparency = 0`nsetup_shown = true`ncheck_updates = false`nmonitors_in_app = false`nai_assist = true`nai_provider = `"anthropic`"`n"
        [System.IO.File]::WriteAllText((Join-Path $config "settings.toml"), $settings, $utf8)
        New-Item -ItemType Directory -Force (Join-Path $config "plugins"), (Join-Path $config "geo") | Out-Null
        Copy-Item (Join-Path $repo "plugins\*.wasm") (Join-Path $config "plugins")
        Copy-Item $geo (Join-Path $config "geo\demo.mmdb")

        $env:APIX_TOUR_WORK = $work
        $env:APIX_CONFIG_DIR = $config
        $env:APIX_TOUR = Join-Path $repo "Docs\tour\scenes.toml"
        $env:APIX_TOUR_OUT = $out
        $env:APIX_TOUR_ONLY = ($Only -join ",")
        $app = Start-Process -FilePath (Join-Path $repo "target\release\apix-desktop.exe") -ArgumentList (Join-Path $repo "Docs\tour\demo") -WorkingDirectory $repo -RedirectStandardError (Join-Path $out "app.err") -PassThru
        $ready = Join-Path $out "ready"
        $taken = 0
        while (-not $app.HasExited) {
            if (Test-Path $ready) {
                Start-Sleep -Milliseconds 150
                $name = (Get-Content $ready -Raw).Trim()
                $app.Refresh()
                Save-Window $app.MainWindowHandle (Join-Path $out "$name.png")
                Remove-Item $ready
                $taken++
                Write-Host "$lang  $name"
            }
            Start-Sleep -Milliseconds 100
        }
        $images = Join-Path $repo "Docs\$lang\images"
        New-Item -ItemType Directory -Force $images | Out-Null
        # 256 colours: the same to the eye, a third smaller.
        $pictures = @(Get-ChildItem $out -Filter *.png | ForEach-Object { $_.FullName })
        if ($pictures.Count) { & (Join-Path $repo "target\release\shrink-png.exe") @pictures }
        Get-ChildItem $out -Filter *.png | Copy-Item -Destination $images -Force
        Write-Host "$lang`: $taken pictures"
        # What the tour wrote into the demo collection (captured requests).
        Remove-Item -Recurse -Force (Join-Path $demo "captured") -ErrorAction SilentlyContinue
    }
}
finally {
    Remove-Item Env:APIX_CONFIG_DIR, Env:APIX_TOUR, Env:APIX_TOUR_OUT, Env:APIX_TOUR_ONLY, Env:APIX_TOUR_WORK -ErrorAction SilentlyContinue
    foreach ($s in $servers) { Stop-Process -Id $s.Id -Force -ErrorAction SilentlyContinue }
}
