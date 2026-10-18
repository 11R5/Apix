<#
.SYNOPSIS
Measures Apix on this machine and prints a Markdown table for the README:
release binary size, time to the first frame, memory when idle, and how long
a large JSON response takes to appear on screen.

.DESCRIPTION
Runs the release build. The timings come from the app itself (APIX_BENCH, see
crates/apix-desktop/src/bench.rs); memory is read from the OS. Each timing is
the median of -Runs runs. The JSON test serves the body from the local
big_json_server example, so the network is not part of the number.

.EXAMPLE
powershell -File scripts/measure.ps1
pwsh scripts/measure.ps1 -NoBuild -Runs 7 -JsonMb 100
#>
param(
    [switch]$NoBuild,
    [int]$Runs = 5,
    [int]$JsonMb = 100
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$windows = $env:OS -eq 'Windows_NT'
$suffix = if ($windows) { '.exe' } else { '' }
$app = Join-Path 'target' (Join-Path 'release' "apix-desktop$suffix")
$server = Join-Path 'target' (Join-Path 'release' (Join-Path 'examples' "big_json_server$suffix"))

if (-not $NoBuild) {
    cargo build --release -p apix-desktop
    cargo build --release -p apix-http --example big_json_server
}

# The same digits on every system locale: 33.1, not 33,1.
function Format-Number([double]$value, [int]$decimals, [string]$unit) {
    $value.ToString("F$decimals", [System.Globalization.CultureInfo]::InvariantCulture) + " $unit"
}

function Get-Median([double[]]$values) {
    $sorted = $values | Sort-Object
    $sorted[[int][math]::Floor(($sorted.Count - 1) / 2)]
}

# Runs the app in one APIX_BENCH mode and returns the reported milliseconds.
function Invoke-Bench([string]$mode, [string]$expect) {
    $out = [System.IO.Path]::GetTempFileName()
    Remove-Item $out
    $env:APIX_BENCH = $mode
    $env:APIX_BENCH_OUT = $out
    try {
        $process = Start-Process -FilePath $app -PassThru
        if (-not $process.WaitForExit(120000)) {
            $process.Kill()
            throw "Apix did not finish '$mode' within 2 minutes"
        }
    } finally {
        Remove-Item Env:APIX_BENCH, Env:APIX_BENCH_OUT
    }
    $line = Get-Content $out | Select-Object -First 1
    Remove-Item $out
    $name, $value = $line -split '='
    if ($name -ne $expect) { throw "'$mode' reported '$line'" }
    [double]$value
}

$results = [ordered]@{}

$size = (Get-Item $app).Length / 1MB
$results['Release binary'] = Format-Number $size 1 'MB'

$startup = 1..$Runs | ForEach-Object { Invoke-Bench 'startup' 'startup_first_frame_ms' }
$results['Start to first frame'] = Format-Number (Get-Median $startup) 0 'ms'

# Idle memory: the example collection open, nothing sent.
$process = Start-Process -FilePath $app -PassThru
Start-Sleep -Seconds 5
$process.Refresh()
$results['Memory when idle (working set)'] = Format-Number ($process.WorkingSet64 / 1MB) 0 'MB'
$process.Kill()
$process.WaitForExit()

$serve = Start-Process -FilePath $server -ArgumentList "$JsonMb" -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 3   # the server builds the body before listening
    $json = 1..$Runs | ForEach-Object {
        Invoke-Bench 'json:http://127.0.0.1:8765/' 'json_on_screen_ms'
    }
    $results["$JsonMb MB JSON response on screen"] = Format-Number (Get-Median $json) 0 'ms'
} finally {
    $serve.Kill()
}

$cpu = if ($windows) {
    (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim()
} else {
    (uname -m)
}
$os = [System.Environment]::OSVersion.VersionString
$version = (Select-String -Path 'Cargo.toml' -Pattern '^version = "(.*)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value

"| Measure | Apix $version |"
"|---|---|"
foreach ($entry in $results.GetEnumerator()) { "| $($entry.Key) | $($entry.Value) |" }
""
"Measured with scripts/measure.ps1 ($Runs runs, median) on $cpu, $os, $(Get-Date -Format 'yyyy-MM-dd')."
