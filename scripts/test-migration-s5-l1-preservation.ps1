[CmdletBinding()]
param([Parameter(Mandatory)][string]$Case)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$inputRoot=Join-Path $workspace ".tmp/migration-s5-storage/preservation-probe-$Case"
$outputRoot=Join-Path $workspace ".tmp/migration-s5-preserved/preservation-probe-$Case"
if((Test-Path -LiteralPath $inputRoot) -or (Test-Path -LiteralPath $outputRoot)){throw 'Probe IDs are immutable.'}
[void][IO.Directory]::CreateDirectory($inputRoot)
$logRoot="$outputRoot-logs"
[void][IO.Directory]::CreateDirectory($logRoot)
$lockOne=Join-Path $inputRoot 'one/state/.kilogram-state.lock'
$lockTwo=Join-Path $inputRoot 'two/state/.kilogram-state.lock'
foreach($path in @($lockOne,$lockTwo)){[void][IO.Directory]::CreateDirectory((Split-Path -Parent $path));[IO.File]::WriteAllBytes($path,[byte[]]@())}
[IO.File]::WriteAllBytes((Join-Path $inputRoot 'opaque-probe.bin'),[byte[]](0..255))
$shell=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
$stream=[IO.File]::Open($lockOne,[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::ReadWrite)
$second=[IO.File]::Open($lockTwo,[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::ReadWrite)
function Invoke-Probe([string]$Suffix){
 $ownerTwo=if($Suffix -eq 'ownership'){'missing-ownership'}else{$lockTwo}
 $p=Start-Process -FilePath $shell -ArgumentList @('-NoProfile','-NonInteractive','-File',('"{0}"' -f (Join-Path $PSScriptRoot 'preserve-migration-s5-l1-input.ps1')),'-InputDirectory',('"{0}"' -f $inputRoot),'-OutputDirectory',('"{0}"' -f "$outputRoot-$Suffix"),'-OwnedLockOne',('"{0}"' -f $lockOne),'-OwnedLockTwo',('"{0}"' -f $ownerTwo)) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $logRoot "$Suffix.stdout") -RedirectStandardError (Join-Path $logRoot "$Suffix.stderr")
 try {if(-not $p.WaitForExit(10000)){throw 'Preservation child deadline.'}}
 finally {if(-not $p.HasExited){$p.Kill($true);$p.WaitForExit()}}
 $p.Refresh();$p.ExitCode
}
try {
 $stream.Lock(0,[long]::MaxValue)
 $second.Lock(0,[long]::MaxValue)
 # Redirected output must be outside the inventoried source; otherwise writing
 # the child's own stdout would correctly fail the source-stability assertion.
 $ok=Invoke-Probe 'empty'
 if($ok -ne 0){throw (Get-Content -Raw -LiteralPath (Join-Path $logRoot 'empty.stderr'))}
 if($stream.Length -ne 0 -or $second.Length -ne 0){throw 'Held source handles changed.'}
 $manifest=Get-Content -Raw -LiteralPath "$outputRoot-empty/preservation-inventory.json"|ConvertFrom-Json
 if(@($manifest.source_files|Where-Object {$_.digest_provenance -eq 'held-lock length checked; empty-byte digest derived'}).Count -ne 2 -or @($manifest.destination_files|Where-Object {$_.digest_provenance -ne 'actual file byte read'}).Count){throw 'Source/destination digest provenance mismatch.'}
 $stream.Unlock(0,[long]::MaxValue)
 $stream.WriteByte(1);$stream.Flush($true);$stream.Lock(0,[long]::MaxValue)
 $bad=Invoke-Probe 'nonempty'
 if($bad -eq 0 -or (Test-Path -LiteralPath "$outputRoot-nonempty")){throw 'Nonempty held OS-lock file was inferred/copied.'}
 $stream.Unlock(0,[long]::MaxValue);$stream.SetLength(0);$stream.Flush($true);$stream.Lock(0,[long]::MaxValue)
 $unknown=Join-Path $inputRoot 'unlisted/.kilogram-state.lock';[void][IO.Directory]::CreateDirectory((Split-Path -Parent $unknown));[IO.File]::WriteAllBytes($unknown,[byte[]]@())
 $unlisted=Invoke-Probe 'unlisted'
 if($unlisted -eq 0 -or (Test-Path -LiteralPath "$outputRoot-unlisted")){throw 'Unlisted same-basename synchronization artifact admitted.'}
 $ownership=Invoke-Probe 'ownership'
 if($ownership -eq 0 -or (Test-Path -LiteralPath "$outputRoot-ownership")){throw 'Missing exact ownership admitted.'}
} finally {$stream.Dispose();$second.Dispose()}
[ordered]@{status='PASS';scope='two exact real held empty locks; actual destination byte digests; opaque non-lock byte equality; nonempty/unlisted/missing ownership refusals';empty_exit=$ok;nonempty_exit=$bad;unlisted_exit=$unlisted;missing_ownership_exit=$ownership}|ConvertTo-Json
