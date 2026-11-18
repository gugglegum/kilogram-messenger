[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Verify','Stage','Restore')][string]$Mode,
    [Parameter(Mandatory)][ValidateSet('A','B')][string]$Side,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9][a-z0-9-]{0,30}$')][string]$Attempt
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if ($isAdmin) { throw 'Stable test images must be managed by the ordinary test user' }
if ($Side -eq 'A' -and $env:COMPUTERNAME -ieq 'DESKTOP-PAUL-RY') {
    $base = 'E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f'
} elseif ($Side -eq 'B' -and $env:COMPUTERNAME -ieq 'PAUL_HP_OMEN') {
    $base = 'C:\Users\test\KilogramLab\terminal-t11f'
} else { throw 'Unexpected test host' }

$names = @('kilogram-cli.exe','kilogram-bootstrap.exe')
$old = @{
    'kilogram-cli.exe'='8c77c2e5cbfe2145b0cf3f8cc433204f90f99f60f6550eb79204c5395a3651f7'
    'kilogram-bootstrap.exe'='8ed2b27bf8a0e293e9622f58e5bc7999a9125b6ad8d573436f5455360fa112c8'
}
$new = @{
    'kilogram-cli.exe'='736827d50c3cfb4f34282b6651dccc82f25131c4c86a8594105771a38cda1ac2'
    'kilogram-bootstrap.exe'='dac789937cdb1743db49face9a5df23ddad89c33e649658ae33ad95ce45a9e7a'
}
$deploy = Join-Path $base "deploy/$Attempt"
$bin = Join-Path $base 'bin'
$archive = Join-Path $deploy 'archive'
$incoming = Join-Path $deploy 'incoming'

function Plain-Path([string]$Path) {
    $entry = Get-Item -LiteralPath $Path
    while ($entry) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Reparse point in $Path" }
        $entry = if ($entry -is [IO.FileInfo]) { $entry.Directory } else { $entry.Parent }
    }
}

function Hash([string]$Path) {
    Plain-Path $Path
    if ((Get-Item -LiteralPath $Path -Force) -isnot [IO.FileInfo]) { throw "Expected a plain file: $Path" }
    (Get-FileHash -LiteralPath $Path).Hash.ToLowerInvariant()
}

function Restore-Image([string]$Archive, [string]$Current, [string]$OldHash, [string]$NewHash) {
    $name = [IO.Path]::GetFileName($Current)
    $observed = Hash $Current
    if ($observed -eq $OldHash) { return }
    $backup = Join-Path $Archive "$name.original"
    if ($observed -ne $NewHash -or (Hash $backup) -ne $OldHash) {
        throw "Unexpected stable or archived image for $name"
    }
    # Enumerating the plain archive sees directory entries even for dangling links.
    $entries = @(Get-ChildItem -LiteralPath $Archive -Force)
    if ($entries.Name -contains "$name.candidate") { throw "Recovery candidate archive already exists for $name" }
    $temp = Join-Path $Archive "$name.restore"
    if ($entries.Name -notcontains "$name.restore") {
        [IO.File]::Copy($backup,$temp,$false)
    }
    if ((Hash $temp) -ne $OldHash) { throw "Unexpected recovery copy for $name" }
    [IO.File]::Replace($temp,$Current,(Join-Path $Archive "$name.candidate"))
}

if ($Mode -ne 'Verify') {
    if (@(Get-Process -Name 'kilogram-*' -ErrorAction SilentlyContinue).Count) { throw 'Close Kilogram test processes first' }
    Plain-Path $deploy
}

if ($Mode -eq 'Stage') {
    foreach ($n in $names) {
        if ((Hash (Join-Path $bin $n)) -ne $old[$n] -or (Hash (Join-Path $incoming $n)) -ne $new[$n]) {
            throw "Original or candidate hash mismatch for $n"
        }
    }
    if (Test-Path -LiteralPath $archive) { throw 'Image archive already exists' }
    [void][IO.Directory]::CreateDirectory($archive)
    foreach ($n in $names) {
        $temp = Join-Path $archive "$n.stage"
        [IO.File]::Copy((Join-Path $incoming $n),$temp,$false)
        if ((Hash $temp) -ne $new[$n]) { throw "Staged image mismatch for $n" }
        [IO.File]::Replace($temp,(Join-Path $bin $n),(Join-Path $archive "$n.original"))
        if ((Hash (Join-Path $archive "$n.original")) -ne $old[$n]) { throw "Archive mismatch for $n" }
    }
}
if ($Mode -eq 'Restore') {
    foreach ($n in $names) {
        Restore-Image $archive (Join-Path $bin $n) $old[$n] $new[$n]
    }
}
$expected = if ($Mode -eq 'Stage') { $new } elseif ($Mode -eq 'Restore') { $old } else { $null }
$images = @(foreach ($n in $names) {
    $digest = Hash (Join-Path $bin $n)
    if ($expected -and $expected[$n] -ne $digest) { throw "Final $Mode hash mismatch for $n" }
    @{name=$n; sha256=$digest}
})
@{mode=$Mode; side=$Side; attempt=$Attempt; images=$images} | ConvertTo-Json -Depth 5 -Compress
