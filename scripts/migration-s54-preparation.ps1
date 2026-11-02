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
function Assert-S54TestStageState($Guard,[ValidateSet('Before','After')][string]$State) {
    Assert-S54ImageGuard $Guard $Guard.Workspace
    $expected=if($State -ceq 'Before'){0}else{6}
    if($Guard.StageCompleted -or $Guard.Entries[0].Staged -or @($Guard.Entries|Where-Object Staged).Count -ne $expected){throw 'S54 test stage must precede final normal image.'}
    if($State -ceq 'After' -and @($Guard.Entries[1..6]|Where-Object {-not $_.Staged -or $null -eq $_.Stream -or $_.Stream.SafeFileHandle.IsClosed}).Count){throw 'S54 six retained test image leases required.'}
}
function Get-S54GuardedTestRows($Guard,$Artifacts) {
    Assert-S54TestStageState $Guard After
    if(@($Artifacts).Count -ne 6){throw 'S54 exact six guarded test roles.'}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    return @($Artifacts|Sort-Object Package,Kind,Target,Path|ForEach-Object {
        $a=$_;$role=@($script:S54Images[3..8]|Where-Object {$_.package -ceq $a.Package -and $_.kind -ceq $a.Kind -and $_.target -ceq $a.Target})
        if($role.Count -ne 1 -or -not $seen.Add($role[0].role)){throw 'S54 exact six guarded test roles.'}
        $path=Get-S54Path (Join-Path $Guard.Workspace $role[0].path)
        Assert-S54SamePath $a.Path $path
        $entry=@($Guard.Entries|Where-Object Path -CEQ $path)[0]
        $pin=Get-S54LeasedPin $entry
        Assert-S54PinEqual $pin $entry.StagedPin
        [ordered]@{package=$a.Package;kind=$a.Kind;target=$a.Target;path=$pin.path;bytes=$pin.bytes;sha256=$pin.sha256;profile='verification-scale'}
    })
}
function Assert-S54CompilerImageBytes([string]$Executable,$Pin) {
    $bytes=Read-S54Public $Executable 134217728
    if($bytes.Length -ne $Pin.bytes -or (Get-S54Sha $bytes) -cne $Pin.sha256){throw 'S54 actual compiler executable bytes.'}
}
function Get-S54SelectedOriginPins($Capture,[bool]$Normal) {
    $roles=@(if($Normal){$script:S54Images[1]}else{$script:S54Images[3..8]})
    return ,@($roles|ForEach-Object {
        $role=$_;$u=@($Capture.units|Where-Object {$_.package -ceq $role.package -and $_.target -ceq $role.target -and (@($_.kind)-join ',') -ceq $role.kind -and $_.profile.test -ne $Normal -and $_.class -ceq 'runtime-target' -and $_.executable})
        $a=@($Capture.stable_artifacts|Where-Object {$_.package -ceq $role.package -and $_.target -ceq $role.target -and $_.kind -ceq $role.kind})
        if($u.Count -ne 1 -or $a.Count -ne 1){throw 'S54 exact selected compiler origin role.'}
        Assert-S54CompilerImageBytes $u[0].executable $a[0]
        Get-S54Pin $u[0].executable 134217728
    })
}
function Invoke-S54CompileStage($Guard,[string]$Public) {
    # Shared by production preparation and bounded source verification. No
    # inventory, activation, list or runtime capability is created here.
    Assert-S54TestStageState $Guard Before
    $root=$Guard.Workspace
    $embeddedBefore=Get-S54EmbeddedSnapshot $root
    $null=Write-S54New (Join-Path $Public 'embedded-before.json') @{classification='exact public compiler inputs before both preparation builds';inputs=$embeddedBefore} 65536
    $before=Get-KilogramFrozenBuildInputs -Workspace $root -Profile verification-scale -Target x86_64-pc-windows-msvc -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()
    & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package @('kilogram-cli','kilogram-transport-iroh') -Profile verification-scale -CargoJobs 2 -SkipCacheStatus -CargoTarget x86_64-pc-windows-msvc -StableArtifactsDirectory (Join-Path $root 'target/verification-scale/deps') -BuildEvidencePath (Join-Path $Public 'tests.json') -S54PreparationGuard $Guard | Out-Host
    if($LASTEXITCODE -ne 0){throw 'S54 preparation stable compilation failed.'}
    Assert-S54TestStageState $Guard After
    $testsPin=Get-S54Pin (Join-Path $Public 'tests.json') 8388608;$tests=Read-S54Json $testsPin 8388608
    Assert-KilogramFrozenInputs $before $tests.build_inputs_before;Assert-KilogramFrozenInputs $before $tests.build_inputs_after
    $testOrigins=Get-S54SelectedOriginPins $tests $false
    $testDepInfo=Get-S54DepInfoPins $tests $false
    $normalArgs=@('build','--package','kilogram-cli','--bin','kilogram-cli','--profile','verification-scale','--target','x86_64-pc-windows-msvc','--jobs','2','--offline','--locked','--message-format=json-render-diagnostics')
    $raw=@(& cargo @normalArgs);if($LASTEXITCODE -ne 0){throw 'S54 preparation normal compiler failed.'}
    $after=Get-KilogramFrozenBuildInputs -Workspace $root -Profile verification-scale -Target x86_64-pc-windows-msvc -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()
    Assert-KilogramFrozenInputs $before $after
    $units=@(Get-KilogramCompilerUnits -Lines $raw -Workspace $root -Target x86_64-pc-windows-msvc -Profile verification-scale -PackageNames $before.package_names)
    $normal=@($units|Where-Object {$_.package -ceq 'kilogram-cli' -and $_.target -ceq 'kilogram-cli' -and (@($_.kind)-join ',') -ceq 'bin' -and -not $_.profile.test -and $_.executable})
    if($normal.Count -ne 1){throw 'S54 one actual non-test normal compiler image.'}
    $pin=Copy-S54PreparedImage $Guard $normal[0].executable (Join-Path $root $script:S54Images[1].path)
    $normalCapture=@{classification='nonexecuting-normal-cli-compiler-provenance';profile='verification-scale';target='x86_64-pc-windows-msvc';expected_manifest_sha256=$null;build_inputs_before=$before;build_inputs_after=$after;units=$units;raw_compiler_artifact_json=@($raw|Where-Object {$_ -match '^\s*\{"reason":"compiler-artifact"'});stable_artifacts=@(@{package='kilogram-cli';kind='bin';target='kilogram-cli';path=$pin.path;bytes=$pin.bytes;sha256=$pin.sha256;profile='verification-scale'});frozen_guard_verified=$false}
    $normalPin=Write-S54New (Join-Path $Public 'normal.json') $normalCapture 8388608
    # Only the orchestrator owns all-seven completion. No Cargo follows it.
    Complete-S54ImageStage $Guard
    foreach($p in @($testOrigins)+@($testDepInfo)){$null=Read-S54Pin $p 134217728}
    $finalTests=Get-S54SelectedOriginPins $tests $false
    $finalDepInfo=Get-S54DepInfoPins $tests $false
    Assert-S54Equal $testOrigins $finalTests;Assert-S54Equal $testDepInfo $finalDepInfo
    $normalOrigins=Get-S54SelectedOriginPins $normalCapture $true
    $normalDepInfo=Get-S54DepInfoPins $normalCapture $true
    $embeddedAfter=Get-S54EmbeddedSnapshot $root
    Assert-S54Equal $embeddedBefore $embeddedAfter
    return @{before=$before;normal=$normalCapture;normal_pin=$normalPin;normal_argv=$normalArgs;tests=$tests;tests_pin=$testsPin;test_origins_before=$testOrigins;test_origins_after=$finalTests;test_dep_info_before=$testDepInfo;test_dep_info_after=$finalDepInfo;normal_origins=$normalOrigins;normal_dep_info=$normalDepInfo;embedded_before=$embeddedBefore;embedded_after=$embeddedAfter}
}
