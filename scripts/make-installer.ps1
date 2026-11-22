# Builds the Windows installer (component 25, piece C): fleet-native.exe, the README and Fleet's server (`fleet\`) in one
# per-user NSIS setup.exe (packaging/windows/installer.nsi). Unsigned: signing is piece E's.
#
#   scripts/make-installer.ps1 -Exe <fleet-native.exe> -Build <build number> -Server <staged server folder> -Out <setup.exe>
#
# makensis comes from $env:MAKENSIS, else PATH, else NSIS's usual install folder (ci.yml installs NSIS from Chocolatey when the
# runner's image has none). Also writes <Out>.sha256.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [int] $Build,
    [Parameter(Mandatory)] [string] $Server,
    [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$makensis = $env:MAKENSIS
if (-not $makensis) { $makensis = (Get-Command makensis -ErrorAction SilentlyContinue).Source }
if (-not $makensis) {
    $makensis = @("${env:ProgramFiles(x86)}\NSIS\makensis.exe", "$env:ProgramFiles\NSIS\makensis.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $makensis) { throw 'makensis not found: install NSIS or set MAKENSIS' }
if (-not (Test-Path (Join-Path $Server 'WeaveFleet.Api.exe'))) { throw "no WeaveFleet.Api.exe in $Server" }

# What the installer puts in the install folder, laid out as it will be there.
$stage = Join-Path ([IO.Path]::GetTempPath()) ("fleet-native-installer-" + [guid]::NewGuid())
New-Item -ItemType Directory $stage | Out-Null
try {
    Copy-Item $Exe (Join-Path $stage 'fleet-native.exe')
    Copy-Item (Join-Path $root 'README.md') $stage
    Copy-Item $Server (Join-Path $stage 'fleet') -Recurse

    $outFull = [IO.Path]::GetFullPath($Out)
    New-Item -ItemType Directory -Force (Split-Path -Parent $outFull) | Out-Null
    $icon = Join-Path $root 'assets\icon\fleet-native.ico'
    & $makensis /V2 /INPUTCHARSET UTF8 "/DSTAGE=$stage" "/DBUILD=$Build" "/DOUTFILE=$outFull" "/DICON=$icon" (Join-Path $root 'packaging\windows\installer.nsi')
    if ($LASTEXITCODE -ne 0) { throw "makensis failed ($LASTEXITCODE)" }
} finally {
    Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
}

$hash = (Get-FileHash $outFull -Algorithm SHA256).Hash.ToLower()
# `sha256sum -c` form, like the Linux files' .sha256.
Set-Content -NoNewline -Encoding ascii "$outFull.sha256" "$hash  $(Split-Path -Leaf $outFull)`n"
'{0}: {1:N1} MB, sha256 {2}' -f (Split-Path -Leaf $outFull), ((Get-Item $outFull).Length / 1MB), $hash
