[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^S-[a-z0-9-]+$')][string]$Case,
    [Parameter(Mandatory)][ValidateSet('Check','Build','Compile','Fmt','FmtCheck','Clippy')][string]$Action,
    [string[]]$Package=@('kilogram-state','kilogram-cli')
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-evidence'
$record=Join-Path $evidence "$Case.json"
if(Test-Path -LiteralPath $record){throw 'Static attempt IDs are immutable.'}
$timer=[Diagnostics.Stopwatch]::StartNew()
$paths=@(& git -C $workspace diff --name-only)+@(& git -C $workspace ls-files --others --exclude-standard)
$paths=@($paths|Where-Object {$_ -match '\.(rs|ps1)$'})+@('Cargo.lock','Cargo.toml','scripts/run-cargo-tests-stable.ps1','scripts/cargo-resource-policy.ps1')
function SourceInventory {
    @($paths|Sort-Object -Unique|ForEach-Object {
        $full=Join-Path $workspace $_
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
}
$before=SourceInventory
$priorOffline=$env:CARGO_NET_OFFLINE
$code=0;$failure=$null;$argv=@()
try {
    $env:CARGO_NET_OFFLINE='true'
    . (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
    $jobs=Set-KilogramCargoResourcePolicy -RequestedJobs 12
    if($Action -in @('Fmt','FmtCheck')) {
        $argv=@('fmt','--all')
        if($Action -eq 'FmtCheck'){$argv+=@('--','--check')}
    } elseif($Action -eq 'Compile') {
        $live=@(Get-Process -ErrorAction SilentlyContinue|Where-Object {$_.ProcessName -like 'kilogram-*-tests'})
        if($live.Count){throw 'Live harness prevents stable wrapper recopy.'}
        $argv=@('scripts/run-cargo-tests-stable.ps1','-CargoJobs','12','-Profile','verification','-Package')+$Package
        & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package $Package -CargoJobs 12 -Profile verification *> (Join-Path $evidence "$Case.log")
        $code=$LASTEXITCODE
    } else {
        $argv=@($Action.ToLowerInvariant(),'--locked','--offline','--jobs',[string]$jobs,'--profile','verification')
        foreach($name in $Package){$argv+=@('--package',$name)}
        if($Action -eq 'Build'){$argv+=@('--bin','kilogram-cli')}
        if($Action -eq 'Clippy'){$argv+=@('--all-targets','--','-D','warnings')}
    }
    if($Action -ne 'Compile') {
        $oldErrors=$ErrorActionPreference
        try {$ErrorActionPreference='Continue'; & cargo @argv *> (Join-Path $evidence "$Case.log"); $code=$LASTEXITCODE}
        finally {$ErrorActionPreference=$oldErrors}
    }
} catch {$code=1;$failure=$_.Exception.Message;$_|Out-String|Add-Content -LiteralPath (Join-Path $evidence "$Case.log")}
finally {
    $env:CARGO_NET_OFFLINE=$priorOffline
    $after=SourceInventory
    if($Action -ne 'Fmt' -and (ConvertTo-Json -InputObject $before -Compress) -cne (ConvertTo-Json -InputObject $after -Compress)){$code=1;$failure='Source changed during static command.'}
    $timer.Stop()
    [ordered]@{case=$Case;classification='static-only';baseline=(& git -C $workspace rev-parse HEAD);action=$Action;argv=$argv;cargo_features=@();elapsed_seconds=$timer.Elapsed.TotalSeconds;exit_code=$code;error=$failure;before=$before;after=$after;rule='Separate static work outside C/F; no fixture/runtime execution.'}|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $record -Encoding utf8
}
Get-Content -LiteralPath (Join-Path $evidence "$Case.log") -Tail 70
exit $code
