$ErrorActionPreference='Stop'
. 'E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1'
. 'E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s5-l1-deadline.ps1'
$j=[KilogramS5Deadline]::new(40,45,[long][Environment]::TickCount64)
$root='E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block4b-r1-01/guard-probe-01'
$null=New-Item -ItemType Directory -Path $root -ErrorAction Stop
$paths=Get-S54MutablePaths $root
$source=Join-Path $root 'source.bin';[IO.File]::WriteAllText($source,'public source')
$null=[IO.Directory]::CreateDirectory((Split-Path -Parent $paths[0]));[IO.File]::WriteAllText($paths[0],'public previous')
$g=$null
try{$g=New-S54ImageGuard $root (Join-Path $root 'saved');foreach($p in $paths){$null=Copy-S54PreparedImage $g $source $p};Complete-S54ImageStage $g;Write-Output ('guard_staged='+$g.StageCompleted)}
finally{Close-S54ImageGuard $g $false;$j.CleanupChildren(2000);$j.Finish()}
