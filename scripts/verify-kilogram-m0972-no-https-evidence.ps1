[CmdletBinding()]
param(
    [string] $EvidenceDirectory,
    [switch] $SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-M0972Evidence {
    param([Parameter(Mandatory)] [string] $Directory, [Parameter(Mandatory)] [string] $Name)
    $path = Join-Path $Directory $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required M0.9.72 evidence is missing: $Name"
    }
    $item = Get-Item -LiteralPath $path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $item.Length -gt 4MB) {
        throw "M0.9.72 evidence must be a bounded regular file: $Name"
    }
    return (Get-Content -LiteralPath $path -Raw).Replace("`r`n", "`n")
}

function Assert-M0972ExactLine {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Line,
        [Parameter(Mandatory)] [string] $Description
    )
    $matches = [regex]::Matches(
        $Text,
        "(?m)^$([regex]::Escape($Line))$",
        [Text.RegularExpressions.RegexOptions]::Multiline
    )
    if ($matches.Count -ne 1) {
        throw "M0.9.72 evidence must contain exactly one $Description line, found $($matches.Count)"
    }
}

function Assert-M0972Absent {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Description
    )
    if ([regex]::IsMatch($Text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline)) {
        throw "M0.9.72 evidence contains forbidden $Description"
    }
}

function Test-M0972NoHttpsEvidence {
    param([Parameter(Mandatory)] [string] $Directory)

    $expectedEndpoint = 'http://127.0.0.1:18787'
    $manifest = (Read-M0972Evidence $Directory 'manifest.json') | ConvertFrom-Json
    foreach ($name in @(
        'evidence_milestone', 'https_fixture_present_at_start', 'compatibility_endpoint'
    )) {
        if (-not ($manifest.PSObject.Properties.Name -contains $name)) {
            throw "M0.9.72 manifest value is missing: $name"
        }
    }
    if ([string]$manifest.evidence_milestone -cne 'M0.9.72' -or
        [bool]$manifest.https_fixture_present_at_start -ne $false -or
        [string]$manifest.compatibility_endpoint -cne $expectedEndpoint) {
        throw 'M0.9.72 manifest does not declare the canonical absent HTTPS fixture'
    }

    foreach ($entry in @(
        @('00-https-fixture-absence.log', 'before-identity-and-mailbox-activation'),
        @('04-https-fixture-absence.log', 'immediately-before-send')
    )) {
        $absence = Read-M0972Evidence $Directory $entry[0]
        Assert-M0972ExactLine $absence "field_phase=$($entry[1])" "$($entry[1]) phase"
        Assert-M0972ExactLine $absence 'https_fixture_binary_present=false' 'absent fixture binary'
        Assert-M0972ExactLine $absence 'https_fixture_process_started=false' 'never-started fixture process'
        Assert-M0972ExactLine $absence "compatibility_endpoint=$expectedEndpoint" 'inert endpoint identity'
        Assert-M0972ExactLine $absence 'compatibility_endpoint_reachable=false' 'unreachable endpoint'
    }

    $send = Read-M0972Evidence $Directory '04-send-alice.log'
    Assert-M0972ExactLine `
        $send 'runtime_mailbox_https_compatibility_copy=suppressed-exact-volunteer-durability' `
        'suppressed HTTPS compatibility copy'
    Assert-M0972ExactLine $send 'runtime_mailbox_http_put=not-attempted' 'unattempted HTTP PUT'
    Assert-M0972ExactLine `
        $send 'runtime_mailbox_delivery_durability=exact-volunteer-replication' `
        'exact volunteer durability'
    foreach ($forbidden in @(
        '^runtime_mailbox_http_put=attempted$',
        '^runtime_mailbox_delivery_durability=https-compatibility$',
        '^runtime_mailbox_https_compatibility_copy=retained-',
        '^runtime_mailbox_exact_completion_status=failed(?: |$)'
    )) {
        Assert-M0972Absent $send $forbidden $forbidden
    }

    $offline = Read-M0972Evidence $Directory '05-alice-offline.boundary'
    Assert-M0972ExactLine $offline 'alice_runtime_ipc_reachable=false' 'offline Alice runtime'
    Assert-M0972ExactLine $offline 'https_fixture_binary_present=false' 'offline absent fixture binary'
    Assert-M0972ExactLine $offline 'compatibility_endpoint_reachable=false' 'offline unreachable endpoint'
    Assert-M0972ExactLine $offline 'runtime_mailbox_http_put=not-attempted' 'offline no-PUT boundary'

    $boundaries = Read-M0972Evidence $Directory '08-boundaries.log'
    foreach ($line in @(
        'mailbox_https_retirement_boundary=verified',
        'm0972_no_https_kit_boundary=verified'
    )) {
        Assert-M0972ExactLine $boundaries $line $line
    }

    [PSCustomObject]@{
        run_id = [string]$manifest.run_id
        build_commit = [string]$manifest.build_commit
        https_fixture_present_at_start = $false
        https_fixture_binary_present = $false
        compatibility_endpoint_reachable = $false
        http_put = 'not-attempted'
        delivery_durability = 'exact-volunteer-replication'
        volunteer_receipts = '2-of-2'
        result = 'verified'
    }
}

function New-M0972SelfTestEvidence {
    param([Parameter(Mandatory)] [string] $Directory)
    New-Item -ItemType Directory -Path $Directory | Out-Null
    [IO.File]::WriteAllText(
        (Join-Path $Directory 'manifest.json'),
        (([ordered]@{
            run_id = '20260919-160000'
            build_commit = ('ab' * 20)
            evidence_milestone = 'M0.9.72'
            https_fixture_present_at_start = $false
            compatibility_endpoint = 'http://127.0.0.1:18787'
        } | ConvertTo-Json) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
    foreach ($entry in @(
        @('00-https-fixture-absence.log', 'before-identity-and-mailbox-activation'),
        @('04-https-fixture-absence.log', 'immediately-before-send')
    )) {
        [IO.File]::WriteAllLines((Join-Path $Directory $entry[0]), @(
            "field_phase=$($entry[1])",
            'https_fixture_binary_present=false',
            'https_fixture_process_started=false',
            'compatibility_endpoint=http://127.0.0.1:18787',
            'compatibility_endpoint_reachable=false'
        ), [Text.UTF8Encoding]::new($false))
    }
    [IO.File]::WriteAllLines((Join-Path $Directory '04-send-alice.log'), @(
        'runtime_mailbox_https_compatibility_copy=suppressed-exact-volunteer-durability',
        'runtime_mailbox_http_put=not-attempted',
        'runtime_mailbox_delivery_durability=exact-volunteer-replication'
    ), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllLines((Join-Path $Directory '05-alice-offline.boundary'), @(
        'alice_runtime_ipc_reachable=false',
        'https_fixture_binary_present=false',
        'compatibility_endpoint_reachable=false',
        'runtime_mailbox_http_put=not-attempted'
    ), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllLines((Join-Path $Directory '08-boundaries.log'), @(
        'mailbox_https_retirement_boundary=verified',
        'm0972_no_https_kit_boundary=verified'
    ), [Text.UTF8Encoding]::new($false))
}

if ($SelfTest) {
    $root = Join-Path ([IO.Path]::GetTempPath()) ("kilogram-m0972-verifier-" + [Guid]::NewGuid().ToString('N'))
    try {
        New-M0972SelfTestEvidence $root
        if ((Test-M0972NoHttpsEvidence $root).result -cne 'verified') {
            throw 'positive M0.9.72 verifier self-test failed'
        }
        $sendPath = Join-Path $root '04-send-alice.log'
        $send = Get-Content -LiteralPath $sendPath -Raw
        [IO.File]::WriteAllText(
            $sendPath,
            $send.Replace('runtime_mailbox_http_put=not-attempted', 'runtime_mailbox_http_put=attempted'),
            [Text.UTF8Encoding]::new($false)
        )
        $rejected = $false
        try { $null = Test-M0972NoHttpsEvidence $root } catch { $rejected = $true }
        if (-not $rejected) { throw 'M0.9.72 verifier accepted an attempted HTTP PUT' }

        Remove-Item -LiteralPath $root -Recurse -Force
        New-M0972SelfTestEvidence $root
        $absencePath = Join-Path $root '00-https-fixture-absence.log'
        $absence = Get-Content -LiteralPath $absencePath -Raw
        [IO.File]::WriteAllText(
            $absencePath,
            $absence.Replace('compatibility_endpoint_reachable=false', 'compatibility_endpoint_reachable=true'),
            [Text.UTF8Encoding]::new($false)
        )
        $rejected = $false
        try { $null = Test-M0972NoHttpsEvidence $root } catch { $rejected = $true }
        if (-not $rejected) { throw 'M0.9.72 verifier accepted a reachable compatibility endpoint' }

        Write-Output 'm0972_no_https_evidence_self_test=verified'
        Write-Output 'attempted_http_put_rejected=true'
        Write-Output 'reachable_compatibility_endpoint_rejected=true'
        return
    }
    finally {
        if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
    }
}

if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    throw 'EvidenceDirectory is required unless SelfTest is used.'
}
$resolved = [IO.Path]::GetFullPath($EvidenceDirectory)
& (Join-Path $PSScriptRoot 'verify-kilogram-m0969-exact-locator-evidence.ps1') `
    -EvidenceDirectory $resolved -LabelPrefix m0972 -SuppressReport
$report = Test-M0972NoHttpsEvidence $resolved
$report | Format-List
