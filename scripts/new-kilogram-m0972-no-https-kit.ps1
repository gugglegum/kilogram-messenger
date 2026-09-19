[CmdletBinding()]
param(
    [string] $OutputDirectory,
    [ValidateRange(1, 64)] [int] $CargoJobs = 2
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'new-kilogram-m0969-exact-locator-kit.ps1') `
    -OutputDirectory $OutputDirectory -CargoJobs $CargoJobs -Milestone 'M0.9.72'
