# Authenticode signing for the Windows packages (component 25, piece E: rows 25.23, 25.24): the app's exe, the server's own
# exe and dlls, and the installer, each with an RFC 3161 timestamp. Runs only when the owner's certificate is set; without it
# it says so ("not signed: WINDOWS_CERTIFICATE is not set") and changes nothing, so a build with no secrets stays green.
#
#   pwsh scripts/sign-windows.ps1 <file or folder> [<file or folder> ...]
#
# A folder means every .exe and .dll under it that has no valid signature yet (the .NET runtime's own files come signed by
# Microsoft and are left alone).
#
# Secrets: WINDOWS_CERTIFICATE (the .pfx, base64) and WINDOWS_CERTIFICATE_PASSWORD. WINDOWS_TIMESTAMP_URL overrides the
# timestamp server. SIGN_DRY_RUN=1 runs every step but prints each signtool command instead of running it (the password
# masked), so the path can be proven with no certificate.
#
# To use Azure Trusted Signing instead of a .pfx: install its dlib (the Microsoft.Trusted.Signing.Client package), write its
# metadata.json from AZURE_* secrets (endpoint, account, certificate profile), and replace the `/f`, `/p` pair below with
# `/dlib <Azure.CodeSigning.Dlib.dll> /dmdf <metadata.json>`; signtool reads AZURE_TENANT_ID, AZURE_CLIENT_ID and
# AZURE_CLIENT_SECRET from the environment. The rest of this script stays.
param([Parameter(Mandatory, ValueFromRemainingArguments)] [string[]] $Paths)
$ErrorActionPreference = 'Stop'
$dry = $env:SIGN_DRY_RUN -eq '1'

if (-not $env:WINDOWS_CERTIFICATE) {
    'not signed: WINDOWS_CERTIFICATE is not set (SmartScreen warns before the first run: "More info", then "Run anyway")'
    exit 0
}
# Half a set of secrets is a mistake to fix, not a build to ship unsigned.
if (-not $env:WINDOWS_CERTIFICATE_PASSWORD) { Write-Error 'WINDOWS_CERTIFICATE is set but WINDOWS_CERTIFICATE_PASSWORD is not set'; exit 1 }
$timestamp = if ($env:WINDOWS_TIMESTAMP_URL) { $env:WINDOWS_TIMESTAMP_URL } else { 'http://timestamp.digicert.com' }

# signtool comes with the Windows SDK (on GitHub's Windows runners); the newest x64 one.
$signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
if (-not $signtool) {
    $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $signtool) { if ($dry) { $signtool = 'signtool.exe' } else { Write-Error 'signtool.exe not found: install the Windows SDK'; exit 1 } }

$files = foreach ($p in $Paths) {
    if (Test-Path $p -PathType Container) {
        Get-ChildItem $p -Recurse -File -Include *.exe, *.dll | Where-Object { (Get-AuthenticodeSignature $_.FullName).Status -ne 'Valid' } |
            ForEach-Object FullName
    } else { (Resolve-Path $p).Path }
}
$files = @($files)
"signing $($files.Count) files"

$work = Join-Path ([IO.Path]::GetTempPath()) ('fleet-native-sign-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory $work | Out-Null
$pfx = Join-Path $work 'cert.pfx'
try {
    # The certificate is written to a scratch file only for signtool, and removed when this ends.
    if (-not $dry) { [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE)) }
    # signtool takes many files at once; batches keep the command line short.
    for ($i = 0; $i -lt $files.Count; $i += 50) {
        $batch = $files[$i..([Math]::Min($i + 49, $files.Count - 1))]
        $args_ = @('sign', '/fd', 'SHA256', '/f', $pfx, '/p', $env:WINDOWS_CERTIFICATE_PASSWORD, '/tr', $timestamp, '/td', 'SHA256', '/d', 'Fleet Native') + $batch
        $shown = (@($signtool) + $args_ | ForEach-Object { if ($_ -match ' ') { "'$_'" } else { $_ } }) -join ' '
        '+ ' + $shown.Replace($env:WINDOWS_CERTIFICATE_PASSWORD, '***')
        if (-not $dry) {
            & $signtool @args_
            if ($LASTEXITCODE -ne 0) { throw "signtool sign failed ($LASTEXITCODE)" }
        }
    }
    if ($dry) { 'dry run: nothing was signed'; exit 0 }
    foreach ($f in $files) {
        $s = Get-AuthenticodeSignature $f
        if ($s.Status -ne 'Valid') { throw "$f is not validly signed after signing: $($s.Status)" }
    }
    "signed: $($files.Count) files, each checked with Get-AuthenticodeSignature"
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
