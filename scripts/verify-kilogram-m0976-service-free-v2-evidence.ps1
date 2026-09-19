[CmdletBinding()]
param(
    [string] $EvidenceDirectory,
    [switch] $SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-M0976Evidence {
    param([Parameter(Mandatory)] [string] $Directory, [Parameter(Mandatory)] [string] $Name)
    $path = Join-Path $Directory $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required M0.9.76 evidence is missing: $Name"
    }
    $item = Get-Item -LiteralPath $path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $item.Length -gt 4MB) {
        throw "M0.9.76 evidence must be a bounded regular file: $Name"
    }
    return (Get-Content -LiteralPath $path -Raw).Replace("`r`n", "`n")
}

function Assert-M0976ExactLine {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Line,
        [Parameter(Mandatory)] [string] $Description
    )
    $count = [regex]::Matches(
        $Text,
        "(?m)^$([regex]::Escape($Line))$",
        [Text.RegularExpressions.RegexOptions]::Multiline
    ).Count
    if ($count -ne 1) {
        throw "M0.9.76 evidence must contain exactly one $Description line, found $count"
    }
}

function Assert-M0976Absent {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Description
    )
    if ([regex]::IsMatch($Text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline)) {
        throw "M0.9.76 evidence contains forbidden $Description"
    }
}

function Test-M0976ServiceFreeEvidence {
    param(
        [Parameter(Mandatory)] [string] $Directory,
        [switch] $SkipInheritedExactLocator
    )

    if (-not $SkipInheritedExactLocator) {
        & (Join-Path $PSScriptRoot 'verify-kilogram-m0969-exact-locator-evidence.ps1') `
            -EvidenceDirectory $Directory -LabelPrefix 'm0976' -SuppressReport
    }

    $manifest = (Read-M0976Evidence $Directory 'manifest.json') | ConvertFrom-Json
    foreach ($name in @(
        'evidence_milestone', 'https_fixture_present_at_start',
        'mailbox_capability_format', 'central_service_descriptor_present'
    )) {
        if (-not ($manifest.PSObject.Properties.Name -contains $name)) {
            throw "M0.9.76 manifest value is missing: $name"
        }
    }
    if ([string]$manifest.evidence_milestone -cne 'M0.9.76' -or
        [bool]$manifest.https_fixture_present_at_start -ne $false -or
        [string]$manifest.mailbox_capability_format -cne 'v2-exact-volunteer' -or
        [bool]$manifest.central_service_descriptor_present -ne $false) {
        throw 'M0.9.76 manifest does not declare the canonical service-free v2 boundary'
    }
    if ($manifest.PSObject.Properties.Name -contains 'compatibility_endpoint') {
        throw 'M0.9.76 manifest unexpectedly retains a compatibility endpoint'
    }

    $initial = Read-M0976Evidence $Directory '00-service-free-v2.boundary'
    Assert-M0976ExactLine $initial 'field_phase=before-identity-and-mailbox-activation' 'initial phase'
    Assert-M0976ExactLine $initial 'mailbox_capability_format=v2-exact-volunteer' 'v2 capability format'
    Assert-M0976ExactLine $initial 'mailbox_service_descriptor_input=absent' 'absent service input'
    Assert-M0976ExactLine $initial 'https_fixture_binary_present=false' 'absent HTTPS fixture'

    foreach ($entry in @(
        @('03-bob-mailbox-capability.log', 'owner activation'),
        @('03-alice-mailbox-offer-import.log', 'recipient import')
    )) {
        $text = Read-M0976Evidence $Directory $entry[0]
        Assert-M0976ExactLine $text 'mailbox_capability_format=v2-exact-volunteer' "$($entry[1]) v2 format"
        Assert-M0976ExactLine $text 'mailbox_service_descriptor=absent' "$($entry[1]) absent descriptor"
        Assert-M0976Absent $text '^(?:mailbox_service_url|mailbox_store_key)=' "$($entry[1]) central tuple"
    }

    $send = Read-M0976Evidence $Directory '04-send-alice.log'
    Assert-M0976ExactLine `
        $send 'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer' `
        'absent v2 HTTPS compatibility copy'
    Assert-M0976ExactLine $send 'runtime_mailbox_http_put=not-attempted' 'unattempted HTTP PUT'
    Assert-M0976ExactLine `
        $send 'runtime_mailbox_delivery_durability=exact-volunteer-replication' `
        'exact volunteer durability'
    foreach ($forbidden in @(
        '^runtime_mailbox_http_put=attempted$',
        '^runtime_mailbox_delivery_durability=https-compatibility$',
        '^runtime_mailbox_https_compatibility_copy=(?:retained-|suppressed-)',
        '^runtime_mailbox_exact_completion_status=failed(?: |$)'
    )) {
        Assert-M0976Absent $send $forbidden $forbidden
    }

    $offline = Read-M0976Evidence $Directory '05-alice-offline.boundary'
    foreach ($line in @(
        'alice_runtime_ipc_reachable=false',
        'mailbox_capability_format=v2-exact-volunteer',
        'mailbox_service_descriptor=absent',
        'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
        'runtime_mailbox_http_put=not-attempted'
    )) {
        Assert-M0976ExactLine $offline $line $line
    }

    $boundaries = Read-M0976Evidence $Directory '08-boundaries.log'
    foreach ($line in @(
        'mailbox_https_retirement_boundary=verified',
        'runtime_cooperative_scheduling=verified',
        'runtime_mailbox_replication_recovery=verified',
        'service_free_mailbox_capability_v2=verified',
        'm0974_no_https_kit_boundary=verified',
        'm0976_service_free_v2_kit_boundary=verified'
    )) {
        Assert-M0976ExactLine $boundaries $line $line
    }

    [PSCustomObject]@{
        run_id = [string]$manifest.run_id
        build_commit = [string]$manifest.build_commit
        mailbox_capability_format = 'v2-exact-volunteer'
        central_service_descriptor_present = $false
        compatibility_endpoint_present = $false
        http_put = 'not-attempted'
        delivery_durability = 'exact-volunteer-replication'
        volunteer_receipts = '2-of-2'
        result = 'verified'
    }
}

function New-M0976SpecificSelfTestEvidence {
    param([Parameter(Mandatory)] [string] $Directory)
    New-Item -ItemType Directory -Path $Directory | Out-Null
    [IO.File]::WriteAllText(
        (Join-Path $Directory 'manifest.json'),
        (([ordered]@{
            run_id = '20260919-220000'
            build_commit = ('ab' * 20)
            evidence_milestone = 'M0.9.76'
            https_fixture_present_at_start = $false
            mailbox_capability_format = 'v2-exact-volunteer'
            central_service_descriptor_present = $false
        } | ConvertTo-Json) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
    [IO.File]::WriteAllLines((Join-Path $Directory '00-service-free-v2.boundary'), @(
        'field_phase=before-identity-and-mailbox-activation',
        'mailbox_capability_format=v2-exact-volunteer',
        'mailbox_service_descriptor_input=absent',
        'https_fixture_binary_present=false'
    ), [Text.UTF8Encoding]::new($false))
    foreach ($name in @('03-bob-mailbox-capability.log', '03-alice-mailbox-offer-import.log')) {
        [IO.File]::WriteAllLines((Join-Path $Directory $name), @(
            'mailbox_capability_format=v2-exact-volunteer',
            'mailbox_service_descriptor=absent'
        ), [Text.UTF8Encoding]::new($false))
    }
    [IO.File]::WriteAllLines((Join-Path $Directory '04-send-alice.log'), @(
        'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
        'runtime_mailbox_http_put=not-attempted',
        'runtime_mailbox_delivery_durability=exact-volunteer-replication'
    ), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllLines((Join-Path $Directory '05-alice-offline.boundary'), @(
        'alice_runtime_ipc_reachable=false',
        'mailbox_capability_format=v2-exact-volunteer',
        'mailbox_service_descriptor=absent',
        'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
        'runtime_mailbox_http_put=not-attempted'
    ), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllLines((Join-Path $Directory '08-boundaries.log'), @(
        'mailbox_https_retirement_boundary=verified',
        'runtime_cooperative_scheduling=verified',
        'runtime_mailbox_replication_recovery=verified',
        'service_free_mailbox_capability_v2=verified',
        'm0974_no_https_kit_boundary=verified',
        'm0976_service_free_v2_kit_boundary=verified'
    ), [Text.UTF8Encoding]::new($false))
}

if ($SelfTest) {
    $root = Join-Path ([IO.Path]::GetTempPath()) ("kilogram-m0976-verifier-" + [Guid]::NewGuid().ToString('N'))
    try {
        New-M0976SpecificSelfTestEvidence $root
        if ((Test-M0976ServiceFreeEvidence $root -SkipInheritedExactLocator).result -cne 'verified') {
            throw 'positive M0.9.76 verifier self-test failed'
        }

        $activationPath = Join-Path $root '03-bob-mailbox-capability.log'
        Add-Content -LiteralPath $activationPath -Value 'mailbox_service_url=https://central.invalid/'
        $rejected = $false
        try { $null = Test-M0976ServiceFreeEvidence $root -SkipInheritedExactLocator } catch { $rejected = $true }
        if (-not $rejected) { throw 'M0.9.76 verifier accepted a central mailbox URL' }

        Remove-Item -LiteralPath $root -Recurse -Force
        New-M0976SpecificSelfTestEvidence $root
        $sendPath = Join-Path $root '04-send-alice.log'
        $send = Get-Content -LiteralPath $sendPath -Raw
        [IO.File]::WriteAllText(
            $sendPath,
            $send.Replace(
                'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
                'runtime_mailbox_https_compatibility_copy=suppressed-exact-volunteer-durability'
            ),
            [Text.UTF8Encoding]::new($false)
        )
        $rejected = $false
        try { $null = Test-M0976ServiceFreeEvidence $root -SkipInheritedExactLocator } catch { $rejected = $true }
        if (-not $rejected) { throw 'M0.9.76 verifier accepted a suppressed legacy compatibility copy' }

        Write-Output 'm0976_service_free_v2_evidence_self_test=verified'
        Write-Output 'central_service_tuple_rejected=true'
        Write-Output 'suppressed_legacy_copy_rejected=true'
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
$report = Test-M0976ServiceFreeEvidence $resolved
$report | Format-List
