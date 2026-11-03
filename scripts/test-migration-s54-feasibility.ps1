[CmdletBinding()]
param([Parameter(Mandatory)][string]$EvidenceDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1')
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Get-S54Path $EvidenceDirectory
$owned=@('.tmp/s54-block4c-implementation-01/','.tmp/s54-block4d-preparation-order-repair-01/','.tmp/s54-block4h-head-repair-01/','.tmp/s54-block4k-admission-diagnostics-01/','.tmp/s54-block4o-f05-purpose-01/','.tmp/s54-block4s-f06-purpose-01/','.tmp/s54-block4w-f07-purpose-01/','.tmp/s54-block4aa-f08-purpose-01/','.tmp/s54-block4ag-f09-purpose-01/','.tmp/s54-block4aj-f10-purpose-01/')
if(-not @($owned|Where-Object {$evidence.StartsWith((Join-Path $workspace $_),[StringComparison]::OrdinalIgnoreCase)}).Count){throw 'Fresh owned public F checks only.'}
$checks=[Collections.Generic.List[object]]::new()
function Require([bool]$Value,[string]$Name){if(-not $Value){throw $Name};$checks.Add(@{name=$Name;passed=$true})}
function Reject([string]$Name,[scriptblock]$Action){$failed=$false;try{$null=& $Action}catch{$failed=$true};Require $failed $Name}
$beforePath=$env:PATH
try {
    $one=Join-Path $evidence 'application-first';$two=Join-Path $evidence 'application-second'
    $null=New-Item -ItemType Directory -Path $one,$two -ErrorAction Stop
    $name='kilogram-s54-public-resolver.exe'
    [IO.File]::WriteAllText((Join-Path $one $name),'first public candidate; never executed')
    [IO.File]::WriteAllText((Join-Path $two $name),'second public candidate; never executed')
    $env:PATH=$one+[IO.Path]::PathSeparator+$two+[IO.Path]::PathSeparator+$beforePath
    Require (@(Get-Command $name -CommandType Application).Count -eq 2) 'two real Application resolutions'
    $selected=Get-KilogramFrozenApplicationPath $name
    Require ($selected -is [string] -and $selected -ceq (Join-Path $one $name)) 'scalar first Application matches invocation'
    Require ((Get-KilogramFrozenHash $selected) -cne (Get-KilogramFrozenHash (Join-Path $two $name))) 'selected content identity is distinct'
    $env:PATH=$two+[IO.Path]::PathSeparator+$beforePath
    Require ((Get-KilogramFrozenApplicationPath $name) -ceq (Join-Path $two $name)) 'single candidate remains scalar'
}finally{$env:PATH=$beforePath}
$p=Get-S54Purpose WholeP;$f=Get-S54Purpose FeasibilityF
Require ($f.Attempt -ceq 'F-s54-mixed-hooks-feasibility-01-attempt-10' -and $p.Attempt -ceq 'P-s54-whole-resumed-l1-01-attempt-01') 'closed F10/P01 binding'
Require ($p.Deadline.controller -eq 2350 -and $p.Deadline.work -eq 2375 -and $p.Deadline.outer -eq 2400) 'P original deadline unchanged'
Require ($f.Deadline.controller -eq 450 -and $f.Deadline.work -eq 480 -and $f.Deadline.outer -eq 510) 'F fixed deadline'
Reject 'closed purpose rejects invented limits' {Get-S54Purpose arbitrary}
$i=@{identity=@{case=$f.Case;attempt=$f.Attempt;selector=$f.Selector;run_id=('a'*64)}}
$refused=0
foreach($suffix in @('01','02','03','04','05','06','07','08','09')){
    $old=@{identity=@{case=$f.Case;attempt=($f.Case+'-attempt-'+$suffix);selector=$f.Selector;run_id=('a'*64)}}
    Reject "consumed F$suffix rejected by actual output policy" {Get-S54OutputPurpose $old}
    $refused++
}
Require ($refused -eq 9) 'all nine consumed attempts executed by output policy'
$inputs=@{inventory=@{sha256=('b'*64)}}
$assertion=@{Pin=@{sha256=('c'*64);bytes=123};Value=@{provenance=@{blake3=('d'*64)}}}
$native=$f.Native+' '+(ConvertTo-Json -InputObject (New-S54Marker $i $inputs $assertion $f.FunctionalClass) -Compress)
$wrapper=$f.Wrapper+' '+(ConvertTo-Json -InputObject (New-S54Marker $i $inputs $assertion $f.WrapperClass) -Compress)
$stdout=(@('running 1 test',('test '+$f.Selector+' ... diagnostic'),$native,'ok','test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s',$wrapper) -join "`n")+"`n"
$layout=Assert-S54NativeOutput $stdout '' $i $inputs $assertion
Require ($layout.native_markers -eq 1 -and $layout.wrapper_markers -eq 1) 'actual F marker parser accepts exact fixed grammar'
Reject 'F rejects P marker' {Assert-S54NativeOutput ($stdout.Replace($f.Native,$p.Native)) '' $i $inputs $assertion}
Reject 'F rejects extra foreign P marker' {Assert-S54NativeOutput ($stdout+"S54_FUNCTIONAL private-data`n") '' $i $inputs $assertion}
Reject 'F rejects marker on stderr' {Assert-S54NativeOutput $stdout $native $i $inputs $assertion}
Reject 'F rejects zero invocation' {Assert-S54NativeOutput ($stdout.Replace('running 1 test','running 0 tests')) '' $i $inputs $assertion}
Reject 'F rejects incomplete wrapper' {Assert-S54NativeOutput ($stdout.Replace($wrapper,'partial')) '' $i $inputs $assertion}
$wrong=@{identity=@{case=$p.Case;attempt=$p.Attempt;selector=$p.Selector;run_id=('a'*64)}}
Reject 'P rejects F output' {Assert-S54NativeOutput $stdout '' $wrong $inputs $assertion}
$bad=@{identity=@{case=$f.Case;attempt=$f.Attempt;selector=$p.Selector;run_id=('a'*64)}}
Reject 'F output identity cannot use P selector' {Get-S54OutputPurpose $bad}
Set-S54Purpose FeasibilityF
$bound=@{FrozenExecutionOnly=$true;S54Purpose='FeasibilityF';S54InventorySha256=('a'*64);S54ActivationSha256=('b'*64);S54LaunchSha256=('c'*64);S54CaptureSha256=('d'*64)}
Assert-S54FrozenArguments $bound
Require $true 'fixed F leaf option set'
$bound.S54Purpose='WholeP';Reject 'leaf cross-purpose rejected before setup' {Assert-S54FrozenArguments $bound}
Set-S54Purpose WholeP
$bound.Remove('S54Purpose');Assert-S54FrozenArguments $bound
Require $true 'historical P leaf option set unchanged'
# Exercise the REAL leaf parameter/dot-source path. Valid closed options must
# reach the absent pinned-input refusal, never a configuration-variable collision
# or the default compiler/list/cache/copy lane. No F/P directory is allocated.
$script:forbiddenLeaf=0
function cargo {$script:forbiddenLeaf++;throw 'FORBIDDEN leaf compiler'}
function Copy-Item {$script:forbiddenLeaf++;throw 'FORBIDDEN leaf copy'}
try {
    foreach($modeName in @('WholeP','FeasibilityF')) {
        $forward=@{FrozenExecutionOnly=$true;S54Purpose=$modeName;S54InventorySha256=('a'*64);S54ActivationSha256=('b'*64);S54LaunchSha256=('c'*64);S54CaptureSha256=('d'*64)}
        $leafError=$null;try{& (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') @forward}catch{$leafError=$_.Exception.Message}
        Require ($null -ne $leafError -and $leafError -match 'S54 missing public path|S54 externally bound input bytes') ("actual frozen leaf reaches pinned-input refusal/$modeName : $leafError")
    }
    Require ($script:forbiddenLeaf -eq 0) 'actual F/P frozen leaf never reached compiler/copy'
}finally{Remove-Item Function:cargo;Remove-Item Function:Copy-Item}
foreach($path in @('cargo-frozen-artifacts.ps1','run-cargo-tests-stable.ps1','invoke-migration-s54-whole.ps1','migration-s54-outer.ps1','test-migration-s54-feasibility.ps1')){
    $tokens=$null;$errors=$null;$null=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $path),[ref]$tokens,[ref]$errors)
    Require ($errors.Count -eq 0) "parse/$path"
}
Write-KilogramFrozenJson (Join-Path $evidence 'result.json') @{classification='pure public F boundary checks; no messenger/compiler/profile/network';checks=@($checks);exit_code=0}
Write-Output "public_F_checks=$($checks.Count);exit_code=0"
