# Seven fixed mutable image roles only. Production fixes Workspace internally;
# public tests use the same layout below their fresh owned test workspace.
function Get-S54MutablePaths([string]$Workspace) {
    return @(@($script:S54Images | Where-Object {$_.role -cne 'legacy-cli' -and $_.role -cne 'relay'}) | ForEach-Object {Get-S54Path (Join-Path $Workspace $_.path) -AllowMissing})
}
function Assert-S54NoLiveImages([string[]]$Paths) {
    # Local OS process inventory, never process-name selection or termination.
    $rows=@(Get-CimInstance -ClassName Win32_Process -Property ProcessId,ExecutablePath -OperationTimeoutSec 3 -ErrorAction Stop)
    foreach($row in $rows){
        if(-not $row.ExecutablePath){continue}
        $reported=[IO.Path]::GetFullPath($row.ExecutablePath)
        if($reported -notin $Paths){continue}
        $process=$null
        try {
            $process=[Diagnostics.Process]::GetProcessById([int]$row.ProcessId)
            if($process.HasExited){continue}
            $actual=[IO.Path]::GetFullPath($process.MainModule.FileName)
            if($actual -in $Paths){throw "S54 live exact image path refused: $actual (pid $($process.Id))."}
        }catch [ArgumentException]{continue}
        finally{if($null -ne $process){$process.Dispose()}}
    }
}
function Get-S54LeasedPin($Entry) {
    $stream=$Entry.Stream
    if($null -eq $stream -or $stream.Length -gt 134217728){throw 'S54 active bounded image lease required.'}
    $stream.Position=0
    return @{path=$Entry.Path;bytes=$stream.Length;sha256=([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream))).ToLowerInvariant()}
}
function Assert-S54ImageGuard($Guard,[string]$Workspace) {
    if($null -eq $Guard -or $Guard.OwnerPid -ne $PID -or $Guard.Closed -or $Guard.Lock.SafeFileHandle.IsClosed){throw 'S54 active owned preparation guard required.'}
    Assert-S54SamePath $Guard.Workspace $Workspace
    Assert-S54Equal @($Guard.Entries|ForEach-Object {$_.Path}) (Get-S54MutablePaths $Workspace)
    $null=Read-S54Pin $Guard.PreimageManifest 65536
}
function New-S54ImageGuard([string]$Workspace,[string]$Evidence) {
    $root=Get-S54Path $Workspace;$paths=Get-S54MutablePaths $root
    if($paths.Count -ne 7 -or @($paths|Sort-Object -Unique).Count -ne 7){throw 'S54 exact seven mutable image paths.'}
    $evidencePath=Get-S54Path $Evidence -AllowMissing
    if(-not $evidencePath.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'S54 image preimages must be owned by workspace.'}
    $lockPath=Join-Path $root '.tmp/s54-preparation.lock';$null=[IO.Directory]::CreateDirectory((Split-Path -Parent $lockPath));$null=Get-S54Path $lockPath -AllowMissing
    $lock=$null;$guard=$null
    try {
        try{$lock=[IO.File]::Open($lockPath,[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)}catch [IO.IOException]{throw 'S54 concurrent owned preparation refused.'}
        $guard=@{OwnerPid=$PID;Workspace=$root;Evidence=$evidencePath;Lock=$lock;Entries=@();Closed=$false;StageCompleted=$false;PreimageManifest=$null}
        # Preflight all roles before opening any destination for replacement.
        Assert-S54NoLiveImages $paths
        $null=New-Item -ItemType Directory -Path $evidencePath -ErrorAction Stop
        $original=@()
        for($n=0;$n -lt $paths.Count;$n++){
            $path=$paths[$n];$present=Test-Path -LiteralPath $path
            $entry=@{Path=$path;Present=$present;Stream=$null;Staged=$false;StagedPin=$null;Index=$n}
            $guard.Entries+=,$entry
            $before=$null;$saved=$null
            if($present){
                # Write lease denies other writers/deletion and executable image
                # replacement; reads use this retained handle until stage ends.
                $entry.Stream=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::Read)
                $before=Get-S54LeasedPin $entry
                $savedPath=Join-Path $evidencePath ('preimage-{0:d2}.bin' -f $n)
                $copy=[IO.File]::Open($savedPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
                try{$entry.Stream.Position=0;$entry.Stream.CopyTo($copy);$copy.Flush($true)}finally{$copy.Dispose()}
                $saved=Get-S54Pin $savedPath 134217728
                if($saved.bytes -ne $before.bytes -or $saved.sha256 -cne $before.sha256){throw 'S54 saved preimage differs from leased original.'}
            }
            $original+=@{path=$path;state=if($present){'present'}else{'absent'};original=$before;saved=$saved}
        }
        $guard.PreimageManifest=Write-S54New (Join-Path $evidencePath 'preimages.json') @{classification='seven_image_preimages_before_any_replacement';owner_pid=$PID;workspace=$root;images=$original} 65536
        return $guard
    }catch{
        if($null -ne $guard){foreach($entry in $guard.Entries){if($null -ne $entry.Stream){$entry.Stream.Dispose()}}}
        if($null -ne $lock){$lock.Dispose()}
        throw
    }
}
function Copy-S54PreparedImage($Guard,[string]$Source,[string]$Destination) {
    Assert-S54ImageGuard $Guard $Guard.Workspace
    if($Guard.StageCompleted){throw 'S54 image staging already completed.'}
    $dest=Get-S54Path $Destination -AllowMissing
    $entries=@($Guard.Entries|Where-Object {$_.Path -ceq $dest})
    if($entries.Count -ne 1 -or $entries[0].Staged){throw 'S54 unstaged exact mutable destination required.'}
    $entry=$entries[0];$sourcePin=Get-S54Pin $Source 134217728
    if([string]::Equals($sourcePin.path,$dest,[StringComparison]::OrdinalIgnoreCase)){throw 'S54 separate compiler source required.'}
    Assert-S54NoLiveImages @($dest)
    if($null -eq $entry.Stream){
        $null=[IO.Directory]::CreateDirectory((Split-Path -Parent $dest));$null=Get-S54Path $dest -AllowMissing
        $entry.Stream=[IO.File]::Open($dest,[IO.FileMode]::CreateNew,[IO.FileAccess]::ReadWrite,[IO.FileShare]::Read)
    }
    $inputFile=[IO.File]::Open($sourcePin.path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    try{$entry.Stream.Position=0;$entry.Stream.SetLength(0);$inputFile.CopyTo($entry.Stream);$entry.Stream.Flush($true)}finally{$inputFile.Dispose()}
    $actual=Get-S54LeasedPin $entry
    $null=Read-S54Pin $sourcePin 134217728
    if($actual.bytes -ne $sourcePin.bytes -or $actual.sha256 -cne $sourcePin.sha256){throw 'S54 actual staged bytes differ from compiler source.'}
    $null=Write-S54New (Join-Path $Guard.Evidence ('staged-{0:d2}.json' -f $entry.Index)) @{classification='one_guarded_image_stage';source=$sourcePin;destination=$actual} 16384
    $entry.Staged=$true;$entry.StagedPin=$actual
    return $actual
}
function Complete-S54ImageStage($Guard) {
    Assert-S54ImageGuard $Guard $Guard.Workspace
    if($Guard.StageCompleted -or @($Guard.Entries|Where-Object {-not $_.Staged}).Count){throw 'S54 all seven images must be staged exactly once.'}
    foreach($entry in $Guard.Entries){$entry.Stream.Dispose();$entry.Stream=$null}
    foreach($entry in $Guard.Entries){$null=Read-S54Pin $entry.StagedPin 134217728}
    $Guard.StageCompleted=$true
}
function Close-S54ImageGuard($Guard,[bool]$Succeeded) {
    if($null -eq $Guard -or $Guard.Closed){return}
    try {
        foreach($entry in $Guard.Entries){if($null -ne $entry.Stream){$entry.Stream.Dispose();$entry.Stream=$null}}
        if($null -ne $Guard.PreimageManifest){$null=Write-S54New (Join-Path $Guard.Evidence 'preparation-outcome.json') @{classification='seven_image_guard_release';owner_pid=$PID;stage_completed=$Guard.StageCompleted;preparation_succeeded=$Succeeded;staged=@($Guard.Entries|Where-Object Staged|ForEach-Object {$_.StagedPin});preimages=$Guard.PreimageManifest;automatic_rollback=$false} 65536}
    }finally{$Guard.Closed=$true;$Guard.Lock.Dispose()}
}
