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
$previousErrorActionPreference = $ErrorActionPreference
try {
    # Windows PowerShell 5.1 surfaces native stderr as non-terminating
    # NativeCommandError records. Cargo writes warnings and progress to stderr,
    # so Stop would turn a successful invocation into a false failure. The
    # native exit code remains the sole success authority.
    $ErrorActionPreference = 'Continue'
    & cargo @arguments
    $cargoExitCode = $LASTEXITCODE
}
finally {
    $ErrorActionPreference = $previousErrorActionPreference
}

exit $cargoExitCode
