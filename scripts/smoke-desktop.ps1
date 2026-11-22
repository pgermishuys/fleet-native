# Smoke test for a built, installed or unpacked app on Windows (row 25.40; scripts/smoke-desktop.sh's twin, from
# weave-fleet's desktop/scripts/smoke.mjs): start the app on scratch data, wait for the Fleet it starts, check the API,
# list a repository (which runs git from Fleet: on Windows a git that inherited Fleet's standard input hung, weave-fleet
# #301), then kill the app the hard way (Stop-Process -Force, TerminateProcess) and check its Fleet stops too and gives up
# the database lock, by starting the app again on the same data.
#
#   pwsh scripts/smoke-desktop.ps1 -App <fleet-native.exe> [-Shot <picture.png>] [-- app arguments]
#
# -Shot saves the app's own window (PrintWindow, so nothing around it) once its Fleet has listed the repository.
# FLEET_NATIVE_SERVER_BIN (and FLEET_NATIVE_SERVER_CONTENT_ROOT) name the server when the app has none bundled beside it.
#
# The scratch folder holds the app's own data, a fake home and the Fleet's data; nothing outside it is touched, and the app
# is told not to start a second instance of itself (a set FLEET_NATIVE_DATA_DIR does that).
param(
    [Parameter(Mandatory)] [string] $App,
    [string] $Shot = '',
    [Parameter(ValueFromRemainingArguments)] [string[]] $AppArgs = @()
)
$ErrorActionPreference = 'Stop'
$began = Get-Date
$App = (Resolve-Path $App).Path

$root = Join-Path ([IO.Path]::GetTempPath()) ('fleet-smoke-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
$data = Join-Path $root 'data'
$repos = Join-Path $root 'repos'
$home_ = Join-Path $root 'home'
$appData = Join-Path $root 'app-data'
foreach ($d in $data, (Join-Path $repos 'smoke-repo'), $home_, $appData, (Join-Path $home_ 'AppData\Roaming'), (Join-Path $home_ 'AppData\Local')) {
    New-Item -ItemType Directory -Force $d | Out-Null
}
Push-Location (Join-Path $repos 'smoke-repo')
try {
    git init -q; Set-Content README.md 'smoke'; git add .; git -c user.name=smoke -c user.email=smoke@example.com commit -q -m smoke
    if ($LASTEXITCODE -ne 0) { throw 'git failed' }
} finally { Pop-Location }

# Never Fleet's default 6262, where somebody's own Fleet may live: the app tries the port its settings remember first.
$port = if ($env:SMOKE_PORT) { [int] $env:SMOKE_PORT } else {
    $l = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0); $l.Start(); $p = $l.LocalEndpoint.Port; $l.Stop(); $p
}
Set-Content (Join-Path $appData 'settings.json') "{`"desktopPort`": $port}"
# The app, like Fleet's, uses a Fleet that already answers on 6262 instead of starting its own; that would test nothing.
try {
    Invoke-WebRequest -Uri 'http://127.0.0.1:6262/readyz' -TimeoutSec 2 | Out-Null
    Write-Host 'A Fleet already answers on 127.0.0.1:6262, and the app would use it rather than start its own: run this where none runs.'
    Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
    exit 2
} catch { }
$instance = Join-Path $data 'fleet.instance.json'
$script:apps = @()
$script:failed = $false

function Read-Instance {
    try { Get-Content -Raw $instance -ErrorAction Stop | ConvertFrom-Json } catch { $null }
}
function Alive([int] $id) { $null -ne (Get-Process -Id $id -ErrorAction SilentlyContinue) }
function Fail([string] $why) { $script:failed = $true; throw "Smoke test failed: $why" }
function Until([string] $what, [int] $secs, [scriptblock] $ok) {
    $end = (Get-Date).AddSeconds($secs)
    while ((Get-Date) -lt $end) {
        if (& $ok) { return }
        Start-Sleep -Milliseconds 500
    }
    Fail "timed out waiting for $what"
}
function Launch {
    # The home folder is the scratch one (USERPROFILE is Windows' HOME; APPDATA and LOCALAPPDATA live under it), so the
    # app's own folders, git's config and the server's caches are the scratch ones. FLEET_DESKTOP_DATA_DIR is where Fleet
    # keeps its database and instance file. Set here, inherited by the child, put back after.
    $set = @{
        USERPROFILE = $home_; HOME = $home_; APPDATA = (Join-Path $home_ 'AppData\Roaming'); LOCALAPPDATA = (Join-Path $home_ 'AppData\Local')
        FLEET_NATIVE_DATA_DIR = $appData; FLEET_DESKTOP_DATA_DIR = $data; FLEET_HARNESS = 'test'; FLEET_WORKSPACE_ROOTS = $repos
    }
    $old = @{}
    foreach ($k in $set.Keys) { $old[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, $set[$k]) }
    try {
        $n = $script:apps.Count
        $start = @{ FilePath = $App; PassThru = $true; RedirectStandardOutput = (Join-Path $root "app-$n.out.log"); RedirectStandardError = (Join-Path $root "app-$n.err.log") }
        if ($AppArgs.Count -gt 0) { $start.ArgumentList = $AppArgs }
        $p = Start-Process @start
    } finally {
        foreach ($k in $old.Keys) { [Environment]::SetEnvironmentVariable($k, $old[$k]) }
    }
    $script:apps += $p
    $p
}
function Fleet-Up([System.Diagnostics.Process] $p, [int] $previous) {
    if ($p.HasExited) { Fail "the app exited (code $($p.ExitCode)) before its Fleet started" }
    $i = Read-Instance
    $null -ne $i -and $i.desktop -eq $true -and $i.pid -gt 0 -and $i.pid -ne $previous -and (Alive $i.pid)
}
function Get-Json([string] $url) { Invoke-RestMethod -Uri $url -TimeoutSec 30 }

function Save-Window([System.Diagnostics.Process] $p, [string] $path) {
    Add-Type -AssemblyName System.Drawing
    if (-not ('SmokeWin' -as [type])) {
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SmokeWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
    }
    [SmokeWin]::SetProcessDPIAware() | Out-Null
    $h = [IntPtr]::Zero
    $end = (Get-Date).AddSeconds(30)
    while ($h -eq [IntPtr]::Zero -and (Get-Date) -lt $end) { $p.Refresh(); $h = $p.MainWindowHandle; if ($h -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 500 } }
    if ($h -eq [IntPtr]::Zero) { Write-Warning 'the app has no window to picture'; return }
    Start-Sleep -Seconds 4 # the sessions page asks its Fleet for the list; let it draw
    $r = New-Object SmokeWin+RECT; [SmokeWin]::GetWindowRect($h, [ref] $r) | Out-Null
    $bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $dc = $g.GetHdc(); [SmokeWin]::PrintWindow($h, $dc, 2) | Out-Null; $g.ReleaseHdc($dc); $g.Dispose()
    New-Item -ItemType Directory -Force (Split-Path -Parent ([IO.Path]::GetFullPath($path))) | Out-Null
    $bmp.Save([IO.Path]::GetFullPath($path), [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    "Saved the app's window to $path."
}

try {
    $proc = Launch
    Until "the app's Fleet to write its instance file" 120 { Fleet-Up $proc 0 }
    $i = Read-Instance
    $url = $i.url.TrimEnd('/'); $fleetPid = [int] $i.pid
    "Fleet $($i.version) is up at $url (pid $fleetPid)"

    try { Invoke-WebRequest -Uri "$url/readyz" -TimeoutSec 30 | Out-Null } catch { Fail "/readyz didn't answer: $_" }
    try { $ui = (Invoke-WebRequest -Uri "$url/" -TimeoutSec 30).Content } catch { Fail "Fleet didn't serve its UI: $_" }
    if ($ui -notmatch 'id="app"') { Fail "Fleet didn't serve its UI" }
    try { $status = Invoke-WebRequest -Uri "$url/api/desktop/status" -TimeoutSec 30 } catch { Fail "/api/desktop/status didn't answer: $_" }
    "UI served, desktop status $($status.Content)"

    # Listing repositories runs git for each one; a git that inherited Fleet's standard input (the app's pipe) hung forever.
    $t = [Diagnostics.Stopwatch]::StartNew()
    try { $list = (Invoke-WebRequest -Uri "$url/api/repositories" -TimeoutSec 30).Content } catch { Fail "/api/repositories didn't answer: $_" }
    if ($list -notmatch '"smoke-repo"') { Fail "/api/repositories didn't list the smoke repository: $list" }
    "Listed the smoke repository in $([int] $t.Elapsed.TotalSeconds) s."
    if ($Shot) { Save-Window $proc $Shot }

    # The app dying any way at all closes the pipe the Fleet reads, so it stops by itself.
    Stop-Process -Id $proc.Id -Force
    Until "the app's Fleet to stop after the app was killed" 30 { -not (Alive $fleetPid) }
    'Killing the app stopped its Fleet.'

    # Fleet deletes its instance file as it stops, but a file left behind is stale and the next start replaces it. What
    # matters is the lock: a new app has to start its own Fleet on the same data.
    $proc = Launch
    Until 'a new app to start its own Fleet on the same data' 120 { Fleet-Up $proc $fleetPid }
    $i = Read-Instance
    $second = [int] $i.pid
    try { Invoke-WebRequest -Uri "$($i.url.TrimEnd('/'))/readyz" -TimeoutSec 30 | Out-Null } catch { Fail "/readyz didn't answer after the restart: $_" }
    "The app started again on the same data (Fleet pid $second), so the lock was released."
    Stop-Process -Id $proc.Id -Force
    Until "the second app's Fleet to stop after it was killed" 30 { -not (Alive $second) }
    'Smoke test passed.'
} catch {
    $script:failed = $true
    Write-Host $_
} finally {
    foreach ($p in $script:apps) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } }
    $left = Read-Instance
    # Only a Fleet this run started: its instance file is in the scratch folder, and the pid is not a later process.
    if ($left -and $left.pid -gt 0) {
        $p = Get-Process -Id $left.pid -ErrorAction SilentlyContinue
        if ($p -and $p.StartTime -ge $began) { Stop-Process -Id $left.pid -Force -ErrorAction SilentlyContinue }
    }
    if ($script:failed) {
        Get-ChildItem $root -Recurse -File -Include 'fleet-server.log', 'app-*.log' -ErrorAction SilentlyContinue | ForEach-Object {
            ''; "-- $($_.Name)"; $c = Get-Content -Raw $_.FullName; if ($c) { $c.Substring([Math]::Max(0, $c.Length - 8000)) }
        }
    }
    Start-Sleep -Milliseconds 500 # the killed processes let go of their files
    Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
}
if ($script:failed) { exit 1 }
exit 0
