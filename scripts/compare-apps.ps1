# Measures Apix next to other API clients on this machine, the same way for
# each: how long until the app's window is on screen, how much memory the
# whole app uses (every process it starts, web views included) after it has
# been idle for a while, and how much disk the installed app takes.
#
#   powershell -ExecutionPolicy Bypass -File scripts\compare-apps.ps1 [-Runs 5]
#
# The apps are found where their Windows installers put them; a missing one is
# skipped. Every app is closed before each run. The first run of each app is a
# warm-up and is not counted; the median of the rest is reported.
param([int]$Runs = 5, [int]$IdleSeconds = 15, [string[]]$Only = @())
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

$apps = [ordered]@{
    "Apix"       = @{ Exe = (Join-Path $repo "target\release\apix-desktop.exe"); Dir = (Join-Path $repo "target\release\apix-desktop.exe") }
    # The app itself, not the launcher in front of it (Squirrel installs); the
    # biggest one, as an update being downloaded leaves a partial file.
    "Postman"    = @{ Exe = (Get-ChildItem "$env:LOCALAPPDATA\Postman\app-*\Postman.exe" -ErrorAction SilentlyContinue | Sort-Object Length | Select-Object -Last 1).FullName; Dir = "$env:LOCALAPPDATA\Postman" }
    # Insomnia.exe there is a stub; the app is insomnia-<version>.exe.
    "Insomnia"   = @{ Exe = (Get-ChildItem "$env:LOCALAPPDATA\insomnia\app-*\insomnia-*.exe" -ErrorAction SilentlyContinue | Sort-Object Length | Select-Object -Last 1).FullName; Dir = "$env:LOCALAPPDATA\insomnia" }
    "Bruno"      = @{ Exe = "$env:ProgramFiles\Bruno\Bruno.exe"; Dir = "$env:ProgramFiles\Bruno" }
    "Hoppscotch" = @{ Exe = "$env:ProgramFiles\Hoppscotch\hoppscotch-desktop.exe"; Dir = "$env:ProgramFiles\Hoppscotch" }
}

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class AppWindows {
    delegate bool EnumProc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr p);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    // A visible window of a real size (not a tooltip) belongs to one of these processes.
    public static bool Shown(HashSet<uint> pids) {
        bool found = false;
        EnumWindows((h, p) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (pids.Contains(pid) && IsWindowVisible(h)) {
                RECT r; GetWindowRect(h, out r);
                if (r.R - r.L >= 400 && r.B - r.T >= 300) { found = true; return false; }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@

# The process and everything it started, by parent id.
function Get-Tree([int]$root) {
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name
    $tree = @($root)
    $added = $true
    while ($added) {
        $added = $false
        foreach ($p in $all) {
            if ($tree -contains [int]$p.ParentProcessId -and -not ($tree -contains [int]$p.ProcessId)) {
                $tree += [int]$p.ProcessId; $added = $true
            }
        }
    }
    $tree
}

function Stop-App($exe) {
    $name = [System.IO.Path]::GetFileNameWithoutExtension($exe)
    Get-Process -Name $name -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 1500
}

function Median($xs) { $s = @($xs | Sort-Object); $s[[int][Math]::Floor($s.Count / 2)] }

$results = @()
foreach ($name in $apps.Keys) {
    if ($Only.Count -and -not ($Only -contains $name)) { continue }
    $app = $apps[$name]
    if (-not $app.Exe -or -not (Test-Path $app.Exe)) { Write-Host "$name not installed, skipped"; continue }
    $starts = @(); $loads = @(); $memory = @()
    for ($run = 0; $run -le $Runs; $run++) {
        Stop-App $app.Exe
        $clock = [System.Diagnostics.Stopwatch]::StartNew()
        $proc = Start-Process -FilePath $app.Exe -PassThru
        $shown = $null
        while ($clock.Elapsed.TotalSeconds -lt 60) {
            # The window belongs to the app's own executable (its helpers share the name).
            $pids = New-Object 'System.Collections.Generic.HashSet[uint32]'
            [void]$pids.Add([uint32]$proc.Id)
            $exeName = [System.IO.Path]::GetFileNameWithoutExtension($app.Exe)
            Get-Process -Name $exeName -ErrorAction SilentlyContinue | ForEach-Object { [void]$pids.Add([uint32]$_.Id) }
            if ([AppWindows]::Shown($pids)) { $shown = $clock.Elapsed.TotalMilliseconds; break }
            Start-Sleep -Milliseconds 10
        }
        # Loaded: the whole app (helpers and web views too) has used less than
        # 3% of one core for a full second. A splash window counts as shown,
        # not as loaded.
        $exeName = [System.IO.Path]::GetFileNameWithoutExtension($app.Exe)
        $ready = $null
        $samples = New-Object System.Collections.Generic.List[object]
        $ids = @()
        $lastTree = -1
        while ($shown -and $clock.Elapsed.TotalSeconds -lt 45) {
            if ($clock.Elapsed.TotalMilliseconds - $lastTree -gt 500) {
                $ids = @()
                foreach ($r in @(Get-Process -Name $exeName -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })) { $ids += Get-Tree $r }
                $ids = @($ids | Sort-Object -Unique)
                $lastTree = $clock.Elapsed.TotalMilliseconds
            }
            $cpu = 0
            foreach ($id in $ids) { $p = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($p) { $cpu += $p.TotalProcessorTime.TotalMilliseconds } }
            $now = $clock.Elapsed.TotalMilliseconds
            $samples.Add(@($now, $cpu))
            $old = $samples | Where-Object { $now - $_[0] -ge 1000 } | Select-Object -Last 1
            if ($old -and ($cpu - $old[1]) -lt 30) { $ready = $old[0]; break }
            Start-Sleep -Milliseconds 100
        }
        Start-Sleep -Seconds $IdleSeconds
        $roots = @(Get-Process -Name $exeName -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
        $ids = @()
        foreach ($r in $roots) { $ids += Get-Tree $r }
        $ids = $ids | Sort-Object -Unique
        $bytes = 0
        foreach ($id in $ids) { $p = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($p) { $bytes += $p.WorkingSet64 } }
        if ($run -gt 0 -and $shown) { $starts += $shown; $loads += $ready; $memory += $bytes / 1MB }
        Write-Host ("{0} run {1}: window after {2:N0} ms, loaded after {3:N0} ms, {4:N0} MB in {5} processes" -f $name, $run, $shown, $ready, ($bytes / 1MB), $ids.Count)
    }
    Stop-App $app.Exe
    $disk = if ((Get-Item $app.Dir).PSIsContainer) { (Get-ChildItem $app.Dir -Recurse -File | Measure-Object Length -Sum).Sum } else { (Get-Item $app.Dir).Length }
    $results += [pscustomobject]@{ App = $name; "Window (ms)" = [int](Median $starts); "Loaded (ms)" = [int](Median $loads); "Idle memory (MB)" = [int](Median $memory); "On disk (MB)" = [int]($disk / 1MB) }
}
$results | Format-Table -AutoSize
$results | ConvertTo-Json | Set-Content (Join-Path $env:TEMP "apix-compare-apps.json")
