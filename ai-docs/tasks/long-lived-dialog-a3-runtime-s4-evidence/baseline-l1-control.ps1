[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$RunDirectory)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
$OutputEncoding=[Text.UTF8Encoding]::new($false)
$repository=(Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
& (Join-Path $repository 'scripts/test-ordered-dialog-process.ps1') -Executable 'target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe' -RunDirectory $RunDirectory -Scenario Duplex -MessagesPerLane 2 -Burst 1
exit $LASTEXITCODE
