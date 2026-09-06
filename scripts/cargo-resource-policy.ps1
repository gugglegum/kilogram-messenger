function Set-KilogramCargoResourcePolicy {
    param([int]$RequestedJobs = 0)

    $logicalProcessors = [Environment]::ProcessorCount
    $jobs = $RequestedJobs
    if ($jobs -le 0 -and -not [string]::IsNullOrWhiteSpace($env:KILOGRAM_CARGO_JOBS)) {
        if (-not [int]::TryParse($env:KILOGRAM_CARGO_JOBS, [ref]$jobs) -or $jobs -le 0) {
            throw 'KILOGRAM_CARGO_JOBS must be a positive integer.'
        }
    }
    if ($jobs -le 0) {
        # Half of the logical processors is a deliberate interactive default:
        # it keeps Windows and local VMs responsive during cold Rust builds.
        $jobs = [math]::Max(1, [math]::Floor($logicalProcessors / 2))
    }
    $jobs = [math]::Min($jobs, $logicalProcessors)

    try {
        [System.Diagnostics.Process]::GetCurrentProcess().PriorityClass =
            [System.Diagnostics.ProcessPriorityClass]::BelowNormal
        Write-Host 'cargo_process_priority=below-normal'
    }
    catch {
        Write-Warning "Could not lower Cargo parent-process priority: $($_.Exception.Message)"
    }
    Write-Host "cargo_logical_processors=$logicalProcessors"
    Write-Host "cargo_jobs=$jobs"
    [int]$jobs
}
