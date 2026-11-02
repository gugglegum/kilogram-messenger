param([Parameter(Mandatory)][string]$Step)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$root=(Get-Location).Path
. (Join-Path $root 'scripts/invoke-migration-s54-whole.ps1') -Purpose FeasibilityF
$base=Get-S54Path (Join-Path $root '.tmp/s54-block4h-head-repair-01')
$baseline=Read-S54Json (Get-S54Pin (Join-Path $base 'baseline.json') 1048576) 1048576
$changed=@('scripts/invoke-migration-s54-whole.ps1','scripts/migration-s54-outer.ps1','scripts/test-migration-s54-admission.ps1','scripts/test-migration-s54-feasibility.ps1','scripts/test-migration-s54-preparation-order.ps1','apps/kilogram-cli/tests/support/migration_s54_purpose.rs','apps/kilogram-cli/tests/support/migration_s54_feasibility_tests.rs','apps/kilogram-cli/tests/support/migration_s54_admission_tests.rs')
function Write-H([string]$Name,$Value){
    $p=Get-S54Path (Join-Path $base $Name) -AllowMissing
    if(-not $p.StartsWith($base+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $p)){throw 'Fresh canonical owned H output.'};Write-KilogramFrozenJson $p $Value
}
function Assert-HProtected {
    if((& git rev-parse HEAD) -cne $baseline.head -or @(& git diff --cached --name-only).Count){throw 'Baseline HEAD/index changed.'}
    foreach($p in @($baseline.navigation)+@($baseline.protected_pins)){$null=Read-S54Pin $p 134217728}
    foreach($p in $baseline.source_pins){if([IO.Path]::GetRelativePath($root,$p.path).Replace('\','/') -cnotin $changed){$null=Read-S54Pin $p 8388608}}
    foreach($p in @($baseline.F01_run,$baseline.F02_run)){if(@(Get-ChildItem -LiteralPath $p -Force).Count){throw 'Historical failed run changed.'}}
    foreach($p in @('.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-03','.tmp/migration-s54-feasibility-public-evidence/F-s54-mixed-hooks-feasibility-01-attempt-03','.tmp/migration-s54-integrated/P-s54-whole-resumed-l1-01-attempt-01','.tmp/migration-s54-public-evidence/P-s54-whole-resumed-l1-01-attempt-01')){if(Test-Path -LiteralPath $p){throw 'No F03/P allocation permitted.'}}
}
Assert-HProtected
if($Step -like 'format-*'){
    $files=@($changed|Where-Object {$_.EndsWith('.rs')}|ForEach-Object {Join-Path $root $_})
    & rustfmt --edition 2024 @files;if($LASTEXITCODE){throw 'Scoped formatting failed.'}
    & rustfmt --edition 2024 --check @files;if($LASTEXITCODE){throw 'Scoped formatting check failed.'}
    Write-H ($Step+'.result.json') @{scoped_rustfmt=$true;files=$files;protected_images_and_history_preserved=$true}
}elseif($Step -like 'compile-*' -or $Step -like 'rust-*'){
    if($env:CARGO_TARGET_DIR){throw 'No inherited cargo target override is admitted to policy unit tests.'}
    $global:KilogramBlock4HTarget=Get-S54Path (Join-Path $root 'target/s54-head-source-checks') -AllowMissing
    $ownership=Join-Path $base 'target-ownership.json'
    if(-not (Test-Path $ownership)){
        if(Test-Path $global:KilogramBlock4HTarget){throw 'Non-production source-check slot must be fresh.'}
        Write-H 'target-ownership.json' @{path=$global:KilogramBlock4HTarget;head=$baseline.head;purpose='exclusive new H source-check slot; not historical'}
    }else{$own=Read-S54Json (Get-S54Pin $ownership 4096) 4096;if($own.path -cne $global:KilogramBlock4HTarget){throw 'Stable slot ownership differs.'}}
    $global:KilogramBlock4HCargo=(Get-Command cargo.exe -CommandType Application|Select-Object -First 1).Source
    Assert-S54SamePath $global:KilogramBlock4HCargo 'C:/Users/Paul/.rustup/toolchains/1.98.0-x86_64-pc-windows-msvc/bin/cargo.exe'
    $global:KilogramBlock4HCargoCalls=[Collections.Generic.List[object]]::new();$global:KilogramBlock4HCopies=[Collections.Generic.List[object]]::new()
    function cargo {
        $forward=@($args)+@('--offline')
        if($forward[0] -ceq 'test'){$forward+=@('--target-dir',$global:KilogramBlock4HTarget)}
        $global:KilogramBlock4HCargoCalls.Add(@{executable=$global:KilogramBlock4HCargo;argv=$forward})
        & $global:KilogramBlock4HCargo @forward
    }
    function Copy-Item {
        param([string]$LiteralPath,[string]$Destination,[switch]$Force)
        $src=Get-S54Path $LiteralPath;$dst=Get-S54Path $Destination -AllowMissing
        foreach($p in @($src,$dst)){if(-not $p.StartsWith($global:KilogramBlock4HTarget+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Source-check runner copy escaped dedicated slot.'}}
        $global:KilogramBlock4HCopies.Add(@{source=$src;destination=$dst})
        Microsoft.PowerShell.Management\Copy-Item -LiteralPath $src -Destination $dst -Force:$Force
    }
    try{
        $a=@{Package=@('kilogram-cli');Profile='verification';CargoJobs=2;SkipCacheStatus=$true}
        if($Step -like 'rust-*'){
            $a.Run=$true;$a.NoCapture=$true;$a.IsolatedTestsFile=Join-Path $base 'isolated-tests.json';$a.IsolatedTestOrder='BeforeAggregate'
            $a.SkipTest=@([char[]](48..57)+[char[]](65..90)+[char[]](97..122)+[char]'_'|ForEach-Object {[string]$_})
        }
        & (Join-Path $root 'scripts/run-cargo-tests-stable.ps1') @a
        if($LASTEXITCODE){throw 'Actual stable runner failed.'}
    }finally{Write-H ($Step+'.cargo.json') @{cargo_calls=@($global:KilogramBlock4HCargoCalls);copies=@($global:KilogramBlock4HCopies);target=$global:KilogramBlock4HTarget;process_CARGO_TARGET_DIR_present=($null -ne [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR','Process'))}}
    $pins=@(Get-ChildItem -LiteralPath (Join-Path $global:KilogramBlock4HTarget 'verification/deps') -Filter 'kilogram-cli-*-tests.exe' -File|ForEach-Object {Get-S54Pin $_.FullName 134217728})
    Write-H ($Step+'.images.json') @{classification='separate stable source-check artifacts only';images=$pins;exact_selectors=$(if($Step -like 'rust-*'){@(Read-S54Json (Get-S54Pin (Join-Path $base 'isolated-tests.json') 16384) 16384).tests}else{@()})}
}elseif($Step -like 'ps-*'){
    . (Join-Path $base 'ps-checks.ps1')
}elseif($Step -like 'seal-*'){
    foreach($name in @($changed|Where-Object {$_.EndsWith('.ps1')})){
        $tokens=$null;$errors=$null;$null=[Management.Automation.Language.Parser]::ParseFile((Join-Path $root $name),[ref]$tokens,[ref]$errors)
        if($errors.Count){throw ('Final PS parse: '+$name)}
    }
    $files=@($changed|Where-Object {$_.EndsWith('.rs')}|ForEach-Object {Join-Path $root $_})
    & rustfmt --edition 2024 --check @files;if($LASTEXITCODE){throw 'Final scoped rustfmt check failed.'}
    & git diff --check -- @changed;if($LASTEXITCODE){throw 'Final source whitespace check failed.'}
    $paths=Get-S54SourcePaths $root
    Assert-S54Equal @($paths|Sort-Object) @($baseline.source_pins|ForEach-Object {$_.path}|Sort-Object)
    $fresh=@($paths|ForEach-Object {Get-S54Pin $_ 8388608})
    $current=Read-S54Json (Get-S54Pin (Join-Path $root '.tmp/s54-block4g-f-preparation-02/diagnose-01.json') 1048576) 1048576
    $rows=@(Get-CimInstance Win32_Process -Property ProcessId,ExecutablePath -OperationTimeoutSec 3 -ErrorAction Stop)
    $imagePaths=@($current.images.pin.path);$target=Get-S54Path (Join-Path $root 'target/s54-head-source-checks')
    $active=@($rows|Where-Object {$_.ExecutablePath -and ($_.ExecutablePath -in $imagePaths -or $_.ExecutablePath.StartsWith($target+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase))}|ForEach-Object {@{pid=$_.ProcessId;path=$_.ExecutablePath}})
    if($active.Count){throw 'Unexpected relevant image process; none stopped.'}
    $head=Get-S54SourceHead $root;if($head -cne (& git rev-parse HEAD)){throw 'Actual checkout HEAD differs.'}
    Write-H ($Step+'.result.json') @{classification='final source-check seal; not production admission/runtime';head=$head;sources=$fresh;changed_sources=$changed;images=$current.images;normal_origins=$current.normal_origins;test_origins=$current.test_origins;normal_dep_info=$current.normal_dep_info;test_dep_info=$current.test_dep_info;native_process_inventory_succeeded=$true;active_relevant_images=$active;historical_F01_F02_and_current_images_preserved=$true;F03_P_directories_absent=$true;scoped_ps_parse_rustfmt_diff_check_passed=$true;runtime='not_established'}
}else{throw 'Unknown H step.'}
Assert-HProtected
Write-H ($Step+'.preservation.json') @{head=(& git rev-parse HEAD);index_empty=$true;navigation_preserved=$true;protected_pins=$baseline.protected_pins;historical_F01_F02_and_images_unchanged=$true;production_F03_P_absent=$true}
Write-Output ($Step+'=passed;historical_images_preserved=true;production=false')
