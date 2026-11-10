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
        # User decision 2026-11-10: cold builds stalled the desktop even with
        # BelowNormal/Jobs2. Prefer one compiler worker unless explicitly set.
        $jobs = 1
    }
    $jobs = [math]::Min($jobs, $logicalProcessors)

    try {
        [System.Diagnostics.Process]::GetCurrentProcess().PriorityClass =
            [System.Diagnostics.ProcessPriorityClass]::Idle
        Write-Host 'cargo_process_priority=idle'
    }
    catch {
        throw "Cannot apply the requested Idle compilation priority: $($_.Exception.Message)"
    }
    Write-Host "cargo_logical_processors=$logicalProcessors"
    Write-Host "cargo_jobs=$jobs"
    [int]$jobs
}
