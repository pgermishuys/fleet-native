# Proves the Windows installer (component 25, piece C) on a real Windows: installs it silently into a scratch folder, checks
# what it wrote, installs again over it, refuses while the app's exe is held, then uninstalls silently and checks what is
# gone and what stays. Writes to the current user's Start menu and HKCU while it runs, and removes both.
#
#   scripts/check-windows-installer.ps1 -Setup <setup.exe> -Exe <the fleet-native.exe it carries> -Build <n>
#       -Server <staged server folder> -Checker <check-bundled-server.exe>
param(
    [Parameter(Mandatory)] [string] $Setup,
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [int] $Build,
    [Parameter(Mandatory)] [string] $Server,
    [Parameter(Mandatory)] [string] $Checker
)
$ErrorActionPreference = 'Stop'
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\FleetNative'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Fleet Native.lnk'
$uninstaller = 'Uninstall Fleet Native.exe'
$Setup = (Resolve-Path $Setup).Path
if ((Test-Path $key) -or (Test-Path $shortcut)) { throw 'Fleet Native is already installed for this user: uninstall it first' }

$dir = Join-Path ([IO.Path]::GetTempPath()) ('fleet-native-install-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
function Install([int] $want) {
    # NSIS reads /D= only as the last argument, unquoted, spaces and all.
    $p = Start-Process -FilePath $Setup -ArgumentList @('/S', "/D=$dir") -Wait -PassThru
    if ($p.ExitCode -ne $want) { throw "setup exited $($p.ExitCode), expected $want" }
}
function Check-Installed {
    $top = (Get-ChildItem $dir | ForEach-Object Name | Sort-Object) -join ', '
    "installed: $top"
    if ($top -ne "fleet, fleet-native.exe, README.md, $uninstaller") { throw "unexpected files in the install folder: $top" }
    if ((Get-FileHash (Join-Path $dir 'fleet-native.exe')).Hash -ne (Get-FileHash $Exe).Hash) { throw 'the installed exe is not the one packed' }
    & $Checker $Server $dir
    if ($LASTEXITCODE -ne 0) { throw 'the installed server is not the pinned one' }
}

try {
    "installing into $dir"
    Install 0
    Check-Installed

    $lnk = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
    "Start menu: $shortcut -> $($lnk.TargetPath)"
    if ($lnk.TargetPath -ne (Join-Path $dir 'fleet-native.exe')) { throw 'the Start-menu entry does not open the installed exe' }

    $reg = Get-ItemProperty $key
    $reg | Format-List DisplayName, DisplayVersion, Publisher, InstallLocation, UninstallString, QuietUninstallString, EstimatedSize, DisplayIcon
    if ($reg.DisplayName -ne 'Fleet Native' -or $reg.DisplayVersion -ne "0.0.$Build" -or $reg.Publisher -ne 'Weave') { throw 'the Apps entry is wrong' }
    if ($reg.InstallLocation -ne $dir -or $reg.EstimatedSize -lt 50000) { throw 'the Apps entry has the wrong folder or size' }
    $v = (Get-Item $Setup).VersionInfo
    "setup: $($v.ProductName), $($v.FileDescription), $($v.ProductVersion), $($v.CompanyName)"
    if ($v.ProductVersion -ne "0.0.$Build.0") { throw 'the setup does not carry the version' }

    # Again over it: a file only an older server had goes, the rest is as before.
    Set-Content (Join-Path $dir 'fleet\left-by-an-older-server.txt') 'old'
    Install 0
    if (Test-Path (Join-Path $dir 'fleet\left-by-an-older-server.txt')) { throw 'an older server file survived the reinstall' }
    Check-Installed
    'installed again over itself: same files'

    # A running app holds its exe: the installer must leave everything as it is and say so (exit code 2).
    $held = [IO.File]::Open((Join-Path $dir 'fleet-native.exe'), 'Open', 'Read', 'Read')
    try { Install 2 } finally { $held.Close() }
    Check-Installed
    'refused while the exe was held, and changed nothing'

    # The uninstaller removes what the installer wrote and nothing else: a file of the user's keeps the folder.
    Set-Content (Join-Path $dir 'notes.txt') 'mine'
    Start-Process -FilePath (Join-Path $dir $uninstaller) -ArgumentList '/S' -Wait
    # It runs from a copy in %TEMP% and returns at once: wait for its work.
    $until = (Get-Date).AddSeconds(120)
    while ((Test-Path (Join-Path $dir 'fleet-native.exe')) -or (Test-Path $key) -or (Test-Path $shortcut)) {
        if ((Get-Date) -gt $until) { throw 'the uninstaller did not finish' }
        Start-Sleep -Milliseconds 200
    }
    $left = (Get-ChildItem $dir | ForEach-Object Name) -join ', '
    "after uninstall: $left"
    if ($left -ne 'notes.txt') { throw "the uninstaller left or took the wrong files: $left" }
    'uninstalled: the exe, the server, the README, the uninstaller, the Start-menu entry and the Apps entry are gone'
} finally {
    if (Test-Path (Join-Path $dir $uninstaller)) { Start-Process -FilePath (Join-Path $dir $uninstaller) -ArgumentList '/S' -Wait }
    Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
}
