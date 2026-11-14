# Measures the long conversation on Windows from a release build and writes the result to a file.
#
#   powershell -ExecutionPolicy Bypass -File scripts\measure-conversation.ps1 [-Exe .\fleet-native.exe] [-Out conversation-results.txt]
#
# The exe is the `fleet-native-windows-x64` artifact of the CI `package` job on main: no Rust needed.
# Run it beside a copy of `scripts\bench\` (a checkout has it).
# Part 1: `--bench-conversation` times every frame's commit and layout with no window and no GPU.
# Part 2: Rune's headless renderer, the window's GPU path drawn offscreen, driven by scripts\bench\*.json
# on a fixed 60 fps clock; Rune prints each frame's CPU time (encode and submit, not the display wait).
# Pass -SkipGpu to leave Part 2 out.
param([string]$Exe = ".\fleet-native.exe", [string]$Out = "conversation-results.txt", [int]$Runs = 3, [switch]$SkipGpu)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Exe)) { throw "no such program: $Exe" }

function Measure-Run([string[]]$ProgramArgs, [string]$Pattern) {
  $err = [System.IO.Path]::GetTempFileName(); $outf = [System.IO.Path]::GetTempFileName()
  $p = Start-Process -FilePath $Exe -PassThru -NoNewWindow -RedirectStandardError $err -RedirectStandardOutput $outf -ArgumentList $ProgramArgs
  $peak = 0; $cpu = 0
  while (-not $p.HasExited) {
    try { $p.Refresh(); $peak = [Math]::Max($peak, $p.PeakWorkingSet64); $cpu = $p.TotalProcessorTime.TotalSeconds } catch {}
    Start-Sleep -Milliseconds 100
  }
  $p.WaitForExit()
  Get-Content $outf, $err | Where-Object { $_ -match $Pattern }
  ("peak working set: {0:N0} MiB; cpu: {1:N1} s" -f ($peak / 1MB), $cpu)
  Remove-Item $err, $outf
}

@(
  "# Long conversation, native app",
  "date: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mmZ'))",
  "system: $([System.Environment]::OSVersion.VersionString); cpu: $((Get-CimInstance Win32_Processor | Select-Object -First 1).Name)",
  "gpu: $((Get-CimInstance Win32_VideoController | ForEach-Object Name) -join '; ')",
  "build: $Exe",
  "",
  "## Part 1: commit and layout per frame (no GPU)"
) | Set-Content $Out
for ($run = 1; $run -le $Runs; $run++) {
  Add-Content $Out ""; Add-Content $Out "run $run"
  Measure-Run @("--bench-conversation") '^\|' | Add-Content $Out
}
if (-not $SkipGpu) {
  Add-Content $Out ""; Add-Content $Out "## Part 2: the GPU path, drawn offscreen"
  foreach ($c in @(@("open", "dark", "open"), @("stream", "long-live", "stream"), @("scroll-end", "long-live", "scroll-end"), @("scroll-top", "long-conversation", "scroll-top"))) {
    for ($run = 1; $run -le $Runs; $run++) {
      Add-Content $Out ""; Add-Content $Out "$($c[0]), run $run"
      Measure-Run @("--scene", $c[1], "--script", "scripts\bench\$($c[2]).json", "--trace", "NUL") '^rune: .*(frames|first frame)' | Add-Content $Out
    }
  }
}
Write-Host "wrote $Out"
