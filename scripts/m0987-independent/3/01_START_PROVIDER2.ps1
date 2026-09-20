[CmdletBinding()]
param(
    [string] $OperatorLabel,
    [string] $NetworkLabel
)

. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
Start-M0987IndependentProvider 'provider2' $OperatorLabel $NetworkLabel
