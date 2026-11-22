# Row 25.43: the Windows package runs with no Visual C++ redistributable installed. A runner (and most machines) has it, so
# starting the app proves nothing; instead every .exe and .dll in the package is read and none may import the redistributable's
# DLLs (vcruntime140*.dll, msvcp140*.dll, concrt140.dll, vccorlib140.dll), directly or delay-loaded. The Universal C runtime
# (ucrtbase.dll, api-ms-win-crt-*) is part of Windows 10 and later, so it is allowed.
#
#   pwsh scripts/check-windows-runtime.ps1 <package folder>
param([Parameter(Mandatory)] [string] $Folder)
$ErrorActionPreference = 'Stop'
$redist = '^(vcruntime140.*|msvcp140.*|concrt140|vccorlib140|vcomp140)(d)?\.dll$'

# The names of the DLLs a PE file imports (the import and delay-import tables), or $null for a file that is not PE.
function Get-Imports([string] $path) {
    $b = [IO.File]::ReadAllBytes($path)
    if ($b.Length -lt 0x40 -or $b[0] -ne 0x4D -or $b[1] -ne 0x5A) { return $null }
    $pe = [BitConverter]::ToInt32($b, 0x3C)
    if ($pe -lt 0 -or $pe + 24 -gt $b.Length -or [BitConverter]::ToUInt32($b, $pe) -ne 0x4550) { return $null }
    $sections = [BitConverter]::ToUInt16($b, $pe + 6)
    $optSize = [BitConverter]::ToUInt16($b, $pe + 20)
    $opt = $pe + 24
    $dirs = $opt + $(if ([BitConverter]::ToUInt16($b, $opt) -eq 0x20B) { 112 } else { 96 })
    $table = $opt + $optSize
    $toOffset = {
        param([uint32] $rva)
        for ($i = 0; $i -lt $sections; $i++) {
            $s = $table + 40 * $i
            $va = [BitConverter]::ToUInt32($b, $s + 12); $size = [Math]::Max([BitConverter]::ToUInt32($b, $s + 8), [BitConverter]::ToUInt32($b, $s + 16))
            if ($rva -ge $va -and $rva -lt $va + $size) { return [int64] ($rva - $va + [BitConverter]::ToUInt32($b, $s + 20)) }
        }
        -1
    }
    $readName = { param([int64] $at) $e = $at; while ($e -lt $b.Length -and $b[$e] -ne 0) { $e++ }; [Text.Encoding]::ASCII.GetString($b, $at, $e - $at) }
    $names = @()
    # (directory index, descriptor size, where the name's RVA is in a descriptor)
    foreach ($d in @(@(1, 20, 12), @(13, 32, 4))) {
        $rva = [BitConverter]::ToUInt32($b, $dirs + 8 * $d[0])
        if ($rva -eq 0) { continue }
        $at = & $toOffset $rva
        while ($at -ge 0 -and $at + $d[1] -le $b.Length) {
            $nameRva = [BitConverter]::ToUInt32($b, $at + $d[2])
            if ($nameRva -eq 0) { break }
            $n = & $toOffset $nameRva
            if ($n -ge 0) { $names += & $readName $n }
            $at += $d[1]
        }
    }
    $names
}

$bad = @(); $count = 0
Get-ChildItem $Folder -Recurse -File -Include *.exe, *.dll | ForEach-Object {
    $imports = Get-Imports $_.FullName
    if ($null -eq $imports) { return }
    $count++
    $hits = @($imports | Where-Object { $_ -match $redist })
    if ($hits.Count -gt 0) { $bad += "$($_.FullName.Substring((Resolve-Path $Folder).Path.Length + 1)): $($hits -join ', ')" }
    if ($_.Name -in 'fleet-native.exe', 'WeaveFleet.Api.exe') { "$($_.Name) imports: $(($imports | Sort-Object -Unique) -join ', ')" }
}
"read the imports of $count programs and libraries"
if ($bad.Count -gt 0) { $bad; throw 'these need the Visual C++ redistributable' }
'none needs the Visual C++ redistributable'
