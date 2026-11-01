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
function Get-KilogramS53MemoryShellDependency([string]$Workspace) {
    if(-not [Environment]::Is64BitProcess){throw 'G4 requires the reviewed x64 parent/search context.'}
    foreach($directory in @($Workspace,(Join-Path $Workspace 'target/verification-scale/deps'))){
        if(Test-Path -LiteralPath (Join-Path $directory 'powershell.exe')){throw 'Workspace/harness powershell.exe shadow changes runtime resolution.'}
    }
    $resolved=Resolve-KilogramScalarExecutable 'powershell.exe'
    $system=Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::System)) 'WindowsPowerShell/v1.0/powershell.exe'
    if(-not [string]::Equals($resolved,[IO.Path]::GetFullPath($system),[StringComparison]::OrdinalIgnoreCase)){throw 'Runtime2 memory shell search differs from reviewed System32 resolution.'}
    $item=Get-Item -LiteralPath $resolved
    if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Memory shell must be a regular executable.'}
    [pscustomobject][ordered]@{role='runtime2-memory-shell';path=$item.FullName;bytes=$item.Length;sha256=(Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
}
function Assert-KilogramS53RuntimeDependencies([string]$Case,$Actual,$Expected) {
    $roles=@(Get-KilogramS53RuntimeRoles $Case)
    if(@($Actual).Count -ne $roles.Count -or @($Expected).Count -ne $roles.Count){throw 'Exact case runtime dependency count required.'}
    for($i=0;$i -lt $roles.Count;$i++){
        if($Actual[$i].role -cne $roles[$i] -or $Expected[$i].role -cne $roles[$i]){throw 'Ordered runtime dependency roles differ.'}
        foreach($key in @('role','path','bytes','sha256')){if($Actual[$i].$key -cne $Expected[$i].$key){throw ('Runtime dependency changed: '+$roles[$i]+'/'+$key)}}
    }
}
function Get-KilogramS53RuntimeDependencies([string]$Case,[string]$Workspace,$Shell,$Expected) {
    $rows=@($Shell)
    if($null -ne (Get-KilogramS53G4Policy $Case)){$rows+=Get-KilogramS53MemoryShellDependency $Workspace}
    Assert-KilogramS53RuntimeDependencies $Case $rows $Expected
    return ,$rows
}
