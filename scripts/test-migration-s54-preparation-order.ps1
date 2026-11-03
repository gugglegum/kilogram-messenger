[CmdletBinding()]
param([Parameter(Mandatory)][string]$EvidenceDirectory,[Parameter(Mandatory)][string]$FixtureIndex,[Parameter(Mandatory)][string]$PFixtureIndex,[switch]$PurposeOnly)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1') -Purpose FeasibilityF
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Get-S54Path $EvidenceDirectory
if(-not @('.tmp/s54-block4d-preparation-order-repair-01/','.tmp/s54-block4h-head-repair-01/','.tmp/s54-block4k-admission-diagnostics-01/','.tmp/s54-block4o-f05-purpose-01/','.tmp/s54-block4s-f06-purpose-01/','.tmp/s54-block4w-f07-purpose-01/','.tmp/s54-block4aa-f08-purpose-01/','.tmp/s54-block4ag-f09-purpose-01/'|Where-Object {$evidence.StartsWith((Join-Path $workspace $_),[StringComparison]::OrdinalIgnoreCase)}).Count){throw 'Fresh owned preparation-order checks only.'}
$checks=[Collections.Generic.List[object]]::new()
function Require([bool]$Value,[string]$Name){if(-not $Value){throw $Name};$checks.Add(@{name=$Name;passed=$true})}
function Reject([string]$Name,[scriptblock]$Action,[string]$Pattern){
    $reason=$null;try{$null=& $Action}catch{$reason=$_.Exception.Message}
    Require ($null -ne $reason -and $reason -match $Pattern) "$Name : $reason"
}
function New-StageRoot([string]$Name){$root=Join-Path $evidence $Name;$null=New-Item -ItemType Directory -Path $root -ErrorAction Stop;return $root}
function Test-Rows($Root){return @($script:S54Images[3..8]|ForEach-Object {[pscustomobject]@{Package=$_.package;Kind=$_.kind;Target=$_.target;Path=(Join-Path $Root $_.path)}})}

if(-not $PurposeOnly){
# The old normal-first order loses compiler provenance when the test build
# replaces a shared compiler path. No executable is launched in this regression.
$root=New-StageRoot 'old-order';$source=Join-Path $root 'compiler-normal.bin'
[IO.File]::WriteAllText($source,'normal linked from normal-build graph')
$guard=New-S54ImageGuard $root (Join-Path $root 'preimages')
$stagePassed=$false
try {
    $pin=Copy-S54PreparedImage $guard $source $guard.Entries[0].Path
    [IO.File]::WriteAllText($source,'normal relinked during test-build graph')
    Reject 'old order refuses actual compiler drift' {Assert-S54CompilerImageBytes $source $pin} 'actual compiler executable bytes'
    Reject 'stable lane refuses normal already staged' {Assert-S54TestStageState $guard Before} 'test stage must precede'
}finally{Close-S54ImageGuard $guard $false}
$old=Read-S54Json (Get-S54Pin (Join-Path $root 'preimages/preparation-outcome.json')) 65536
Require (-not $old.preparation_succeeded -and -not $old.stage_completed -and -not $old.automatic_rollback) 'old-order failure retained without rollback'

$root=New-StageRoot 'tests-then-normal';$paths=Get-S54MutablePaths $root
foreach($path in $paths){$null=[IO.Directory]::CreateDirectory((Split-Path -Parent $path));[IO.File]::WriteAllText($path,'original public image')}
$source=Join-Path $root 'compiler-normal.bin';[IO.File]::WriteAllText($source,'test build incidental normal')
$guard=New-S54ImageGuard $root (Join-Path $root 'preimages')
try {
    Assert-S54TestStageState $guard Before
    Reject 'missing images cannot complete' {Complete-S54ImageStage $guard} 'all seven'
    Reject 'foreign destination refused' {Copy-S54PreparedImage $guard $source (Join-Path $root 'foreign.exe')} 'exact mutable destination'
    Reject 'concurrent preparation refused' {New-S54ImageGuard $root (Join-Path $root 'second-preimages')} 'concurrent owned preparation'
    $origins=@()
    for($n=1;$n -le 6;$n++){
        $origin=Join-Path $root "test-origin-$n.bin";[IO.File]::WriteAllText($origin,"actual public test $n")
        $pin=Copy-S54PreparedImage $guard $origin $paths[$n]
        $origins+=@{origin=$origin;pin=$pin}
        if($n -eq 5){Reject 'five test roles cannot supply six-role rows' {Get-S54GuardedTestRows $guard (Test-Rows $root)} 'test stage must precede'}
    }
    Assert-S54TestStageState $guard After
    $rows=Get-S54GuardedTestRows $guard (Test-Rows $root)
    Require ($rows.Count -eq 6 -and -not $guard.StageCompleted -and @($guard.Entries|Where-Object {$null -ne $_.Stream}).Count -eq 7) 'six staged roles retain all seven leases and normal remains unstaged'
    Reject 'six test roles cannot complete without normal' {Complete-S54ImageStage $guard} 'all seven'
    Reject 'duplicate stage refused' {Copy-S54PreparedImage $guard $origins[0].origin $paths[1]} 'unstaged exact mutable'
    $duplicate=Test-Rows $root;$duplicate[5]=$duplicate[0]
    Reject 'duplicate selected role refused' {Get-S54GuardedTestRows $guard $duplicate} 'exact six guarded test roles'
    $foreign=Test-Rows $root;$foreign[5].Package='foreign'
    Reject 'foreign selected role refused' {Get-S54GuardedTestRows $guard $foreign} 'exact six guarded test roles'
    Reject 'test lease denies another writer before normal' {$s=[IO.File]::Open($paths[1],[IO.FileMode]::Open,[IO.FileAccess]::Write,[IO.FileShare]::ReadWrite);$s.Dispose()} 'being used|another process|используется'
    [IO.File]::WriteAllText($source,'final normal linked from normal-build graph')
    $normal=Copy-S54PreparedImage $guard $source $paths[0]
    Complete-S54ImageStage $guard
    foreach($o in $origins){Assert-S54CompilerImageBytes $o.origin $o.pin}
    [IO.File]::WriteAllText($origins[0].origin,'test compiler origin drift')
    Reject 'test origin drift after final build still refused' {Assert-S54CompilerImageBytes $origins[0].origin $origins[0].pin} 'actual compiler executable bytes'
    Assert-S54CompilerImageBytes $source $normal
    Require ($guard.StageCompleted -and @($guard.Entries|Where-Object {$null -ne $_.Stream}).Count -eq 0) 'orchestrator releases seven leases only after final normal'
    Reject 'duplicate completion refused' {Complete-S54ImageStage $guard} 'all seven'
    Reject 'post-completion staging refused' {Copy-S54PreparedImage $guard $source $paths[0]} 'already completed'
    [IO.File]::WriteAllText($source,'drift after final capture')
    Reject 'final normal origin drift still refused' {Assert-S54CompilerImageBytes $source $normal} 'actual compiler executable bytes'
    $stagePassed=$true
}finally{Close-S54ImageGuard $guard $stagePassed}
$outcome=Read-S54Json (Get-S54Pin (Join-Path $root 'preimages/preparation-outcome.json')) 65536
Require ($outcome.stage_completed -and $outcome.staged.Count -eq 7 -and -not $outcome.automatic_rollback) 'actual seven-stage completion outcome'
$pre=Read-S54Json $outcome.preimages 65536
foreach($row in $pre.images){$null=Read-S54Pin $row.saved 4096;Require ($row.original.sha256 -ceq $row.saved.sha256) 'original preimage retained after final stage'}

$root=New-StageRoot 'interrupted-test-stage';$paths=Get-S54MutablePaths $root
$source=Join-Path $root 'test-origin.bin';[IO.File]::WriteAllText($source,'public test')
$guard=New-S54ImageGuard $root (Join-Path $root 'preimages')
try {
    $null=Copy-S54PreparedImage $guard $source $paths[1]
    Reject 'missing source retains interrupted test stage' {Copy-S54PreparedImage $guard (Join-Path $root 'absent.bin') $paths[2]} 'missing public path'
}finally{Close-S54ImageGuard $guard $false}
$outcome=Read-S54Json (Get-S54Pin (Join-Path $root 'preimages/preparation-outcome.json')) 65536
Require (-not $outcome.stage_completed -and -not $outcome.preparation_succeeded -and $outcome.staged.Count -eq 1 -and -not $outcome.automatic_rollback) 'interrupted test stage keeps outcome and preimages'
$lock=[IO.File]::Open((Join-Path $root '.tmp/s54-preparation.lock'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::None)
$lock.Dispose();Require $true 'failure cleanup releases workspace lock'
}

# Full actual PS admission on a fresh exported Rust F fixture. No production
# inventory or messenger capability is constructed by this test.
$index=Read-S54Json (Get-S54Pin $FixtureIndex 16384) 16384
Require ($index.classification -ceq 'public_schema_fixture_only') 'public fixture only'
$policy=New-S54Policy -FixtureRoot $index.root
$i=Read-S54Json $index.inputs.inventory 2097152
Assert-S54Inventory $i $policy
Require ($i.identity.attempt -ceq 'F-s54-mixed-hooks-feasibility-01-attempt-09') 'actual PS inventory admits F09'
$refused=0
foreach($suffix in @('01','02','03','04','05','06','07','08')){
    $i.identity.attempt=$i.identity.case+'-attempt-'+$suffix
    Reject "actual PS inventory refuses F$suffix" {Assert-S54Inventory $i $policy} 'required identity'
    $refused++
}
Require ($refused -eq 8) 'all eight consumed attempts executed by actual PS inventory'
$i.identity.attempt=(Get-S54Purpose FeasibilityF).Attempt
$i.identity.selector=(Get-S54Purpose WholeP).Selector
Reject 'actual PS inventory refuses foreign selector' {Assert-S54Inventory $i $policy} 'required identity'
Set-S54Purpose WholeP
$pindex=Read-S54Json (Get-S54Pin $PFixtureIndex 16384) 16384
$pi=Read-S54Json $pindex.inputs.inventory 2097152
Assert-S54Inventory $pi (New-S54Policy -FixtureRoot $pindex.root)
Require ($pi.identity.attempt -ceq 'P-s54-whole-resumed-l1-01-attempt-01') 'actual PS inventory retains P01'
Write-KilogramFrozenJson (Join-Path $evidence 'result.json') @{classification='public shared preparation-order and F09 admission regression; no compiler/runtime/network';purpose_only=[bool]$PurposeOnly;checks=@($checks);count=$checks.Count;whole_attempt_verdict='not_established'}
Write-Output ('preparation_order_checks='+$checks.Count)
