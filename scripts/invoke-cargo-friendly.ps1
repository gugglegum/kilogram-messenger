[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('build', 'check', 'clippy', 'test')]
    [string]$CargoCommand,
    [int]$CargoJobs = 0,
    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]]$CargoArguments = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs
$arguments = @($CargoCommand, '--jobs', $cargoJobsResolved) + $CargoArguments
& cargo @arguments
exit $LASTEXITCODE
