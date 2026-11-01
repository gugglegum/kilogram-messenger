# Exact reviewed runtime dependency identity. Read-only; no PATH lookup or probe.
Set-StrictMode -Version Latest
function Assert-KilogramS53ShellIdentity($Actual,$Expected) {
    if($Expected.role -cne 'metadata-shell' -or $Expected.path -isnot [string] -or -not [IO.Path]::IsPathRooted($Expected.path) -or -not [string]::Equals($Actual.path,[IO.Path]::GetFullPath($Expected.path),[StringComparison]::OrdinalIgnoreCase) -or $Expected.bytes -isnot [ValueType] -or $Expected.bytes -is [bool] -or $Expected.bytes -ne $Actual.bytes -or $Expected.sha256 -cnotmatch '^[0-9a-f]{64}$' -or $Expected.sha256 -cne $Actual.sha256){throw 'Reviewed shell dependency identity changed.'}
}
function Get-KilogramS53ShellDependency {
    param([Parameter(Mandatory)][string]$Executable,$Expected,[switch]$RequireCurrentHost)
    if(-not [IO.Path]::IsPathRooted($Executable)){throw 'Explicit absolute reviewed shell required.'}
    $item=Get-Item -LiteralPath $Executable -ErrorAction Stop
    if($item.PSIsContainer -or $item.Name -ine 'pwsh.exe' -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Reviewed shell must be an existing regular pwsh.exe.'}
    $actual=[pscustomobject][ordered]@{role='metadata-shell';path=$item.FullName;bytes=$item.Length;sha256=(Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    if($null -ne $Expected){Assert-KilogramS53ShellIdentity $actual $Expected}
    if($RequireCurrentHost){
        $hostPath=[IO.Path]::GetFullPath([Diagnostics.Process]::GetCurrentProcess().MainModule.FileName)
        if(-not [string]::Equals($hostPath,$actual.path,[StringComparison]::OrdinalIgnoreCase)){throw 'Current shell host is not the exact reviewed executable.'}
    }
    return $actual
}
