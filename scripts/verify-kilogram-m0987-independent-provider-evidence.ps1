[CmdletBinding()]
param(
    [string] $EvidenceDirectory,
    [switch] $SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-M0987Evidence {
    param([Parameter(Mandatory)] [string] $Directory, [Parameter(Mandatory)] [string] $Name)
    $path = Join-Path $Directory $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required M0.9.87 evidence is missing: $Name"
    }
    $item = Get-Item -LiteralPath $path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $item.Length -gt 4MB) {
        throw "M0.9.87 evidence must be a bounded regular file: $Name"
    }
    return (Get-Content -LiteralPath $path -Raw).Replace("`r`n", "`n")
}

function Assert-M0987ExactLine {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Line,
        [Parameter(Mandatory)] [string] $Description
    )
    $count = [regex]::Matches($Text, "(?m)^$([regex]::Escape($Line))$").Count
    if ($count -ne 1) {
        throw "M0.9.87 evidence must contain exactly one $Description line, found $count"
    }
}

function Assert-M0987Absent {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Description
    )
    if ([regex]::IsMatch($Text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline)) {
        throw "M0.9.87 evidence contains forbidden $Description"
    }
}

function Assert-M0987ExactProperties {
    param(
        [Parameter(Mandatory)] [object] $Value,
        [Parameter(Mandatory)] [string[]] $Expected,
        [Parameter(Mandatory)] [string] $Description
    )
    $actual = @($Value.PSObject.Properties.Name | Sort-Object)
    $expectedSorted = @($Expected | Sort-Object)
    if ($actual.Count -ne $expectedSorted.Count -or @(Compare-Object $expectedSorted $actual).Count -ne 0) {
        throw "M0.9.87 $Description contains missing or unapproved fields"
    }
}

function Test-M0987IndependentProviderEvidence {
    param(
        [Parameter(Mandatory)] [string] $Directory,
        [switch] $SkipInheritedExactLocator
    )

    if (-not $SkipInheritedExactLocator) {
        & (Join-Path $PSScriptRoot 'verify-kilogram-m0969-exact-locator-evidence.ps1') `
            -EvidenceDirectory $Directory -LabelPrefix 'm0987' -SuppressReport
    }

    $manifest = (Read-M0987Evidence $Directory 'manifest.json') | ConvertFrom-Json
    foreach ($name in @(
        'run_id', 'build_commit', 'evidence_milestone', 'https_fixture_present_at_start',
        'mailbox_capability_format', 'central_service_descriptor_present'
    )) {
        if (-not ($manifest.PSObject.Properties.Name -contains $name)) {
            throw "M0.9.87 manifest value is missing: $name"
        }
    }
    if ([string]$manifest.run_id -cnotmatch '^[0-9]{8}-[0-9]{6}$' -or
        [string]$manifest.build_commit -cnotmatch '^[0-9a-f]{40}$' -or
        [string]$manifest.evidence_milestone -cne 'M0.9.87' -or
        [bool]$manifest.https_fixture_present_at_start -ne $false -or
        [string]$manifest.mailbox_capability_format -cne 'v2-exact-volunteer' -or
        [bool]$manifest.central_service_descriptor_present -ne $false) {
        throw 'M0.9.87 manifest does not declare the canonical independent-provider service-free boundary'
    }
    if ($manifest.PSObject.Properties.Name -contains 'compatibility_endpoint') {
        throw 'M0.9.87 manifest unexpectedly retains a compatibility endpoint'
    }

    $initial = Read-M0987Evidence $Directory '00-service-free-v2.boundary'
    foreach ($line in @(
        'field_phase=before-identity-and-mailbox-activation',
        'mailbox_capability_format=v2-exact-volunteer',
        'mailbox_service_descriptor_input=absent',
        'https_fixture_binary_present=false'
    )) { Assert-M0987ExactLine $initial $line $line }

    foreach ($name in @('03-bob-mailbox-capability.log', '03-alice-mailbox-offer-import.log')) {
        $text = Read-M0987Evidence $Directory $name
        Assert-M0987ExactLine $text 'mailbox_capability_format=v2-exact-volunteer' "$name v2 format"
        Assert-M0987ExactLine $text 'mailbox_service_descriptor=absent' "$name absent descriptor"
        Assert-M0987Absent $text '^(?:mailbox_service_url|mailbox_store_key)=' "$name central tuple"
    }
    $send = Read-M0987Evidence $Directory '04-send-alice.log'
    foreach ($line in @(
        'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
        'runtime_mailbox_http_put=not-attempted',
        'runtime_mailbox_delivery_durability=exact-volunteer-replication'
    )) { Assert-M0987ExactLine $send $line $line }
    foreach ($pattern in @(
        '^runtime_mailbox_http_put=attempted$',
        '^runtime_mailbox_delivery_durability=https-compatibility$',
        '^runtime_mailbox_https_compatibility_copy=(?:retained-|suppressed-)',
        '^runtime_mailbox_exact_completion_status=failed(?: |$)'
    )) { Assert-M0987Absent $send $pattern $pattern }
    $offline = Read-M0987Evidence $Directory '05-alice-offline.boundary'
    foreach ($line in @(
        'alice_runtime_ipc_reachable=false',
        'mailbox_capability_format=v2-exact-volunteer',
        'mailbox_service_descriptor=absent',
        'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
        'runtime_mailbox_http_put=not-attempted'
    )) { Assert-M0987ExactLine $offline $line $line }

    $attestationFields = @(
        'schema', 'evidence_milestone', 'provider_role', 'run_id', 'build_commit',
        'machine_pseudonym', 'operator_claim_digest', 'network_claim_digest',
        'digest_scope', 'claim_boundary', 'private_state_shared'
    )
    $publicationFields = @(
        'schema', 'evidence_milestone', 'provider_role', 'run_id', 'build_commit',
        'offer_sha256', 'offer_expires_at_unix_seconds', 'attestation_sha256',
        'published_at_unix_seconds'
    )
    $claims = @()
    $publications = @{}
    foreach ($provider in @('provider1', 'provider2')) {
        $attestationName = "01-$provider-attestation.json"
        $publicationName = "01-$provider-publication.json"
        $attestation = (Read-M0987Evidence $Directory $attestationName) | ConvertFrom-Json
        $publication = (Read-M0987Evidence $Directory $publicationName) | ConvertFrom-Json
        Assert-M0987ExactProperties $attestation $attestationFields "$provider attestation"
        Assert-M0987ExactProperties $publication $publicationFields "$provider publication"
        if ([int]$attestation.schema -ne 1 -or [int]$publication.schema -ne 1 -or
            [string]$attestation.evidence_milestone -cne 'M0.9.87' -or
            [string]$publication.evidence_milestone -cne 'M0.9.87' -or
            [string]$attestation.provider_role -cne $provider -or
            [string]$publication.provider_role -cne $provider -or
            [string]$attestation.run_id -cne [string]$manifest.run_id -or
            [string]$publication.run_id -cne [string]$manifest.run_id -or
            [string]$attestation.build_commit -cne [string]$manifest.build_commit -or
            [string]$publication.build_commit -cne [string]$manifest.build_commit -or
            [string]$attestation.digest_scope -cne 'run-scoped-sha256' -or
            [string]$attestation.claim_boundary -cne 'controlled-self-attestation-not-protocol-proof' -or
            [bool]$attestation.private_state_shared -ne $false) {
            throw "$provider attestation/publication is not bound to the accepted controlled run"
        }
        foreach ($field in @('machine_pseudonym', 'operator_claim_digest', 'network_claim_digest')) {
            if ([string]$attestation.$field -cnotmatch '^[0-9a-f]{64}$') {
                throw "$provider has an invalid $field"
            }
        }
        $offerPath = Join-Path $Directory "01-$provider.offer"
        $offer = (Read-M0987Evidence $Directory "01-$provider.offer").Trim()
        if ($offer -cnotmatch '^[A-Za-z0-9_-]{64,8192}$') { throw "$provider offer is malformed" }
        $offerHash = (Get-FileHash -LiteralPath $offerPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $attestationHash = (Get-FileHash -LiteralPath (Join-Path $Directory $attestationName) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ([string]$publication.offer_sha256 -cne $offerHash -or
            [string]$publication.attestation_sha256 -cne $attestationHash -or
            [UInt64]$publication.offer_expires_at_unix_seconds -le 1) {
            throw "$provider publication is not hash-bound to its offer and attestation"
        }
        $claims += $attestation
        $publications[$provider] = $publication
    }
    foreach ($field in @('machine_pseudonym', 'operator_claim_digest', 'network_claim_digest')) {
        if (@($claims | ForEach-Object { [string]$_.$field } | Sort-Object -Unique).Count -ne 2) {
            throw "M0.9.87 requires two distinct self-attested $field values"
        }
    }

    $aggregate = (Read-M0987Evidence $Directory '01-provider-offers-publication.json') | ConvertFrom-Json
    if ([int]$aggregate.schema -ne 1 -or [string]$aggregate.evidence_milestone -cne 'M0.9.87' -or
        [string]$aggregate.claim_boundary -cne 'controlled-self-attestation-not-protocol-proof' -or
        @($aggregate.providers).Count -ne 2) {
        throw 'M0.9.87 aggregate provider publication has an invalid claim boundary or shape'
    }
    foreach ($provider in @('provider1', 'provider2')) {
        $entries = @($aggregate.providers | Where-Object { [string]$_.name -ceq $provider })
        if ($entries.Count -ne 1 -or
            [string]$entries[0].sha256 -cne [string]$publications[$provider].offer_sha256 -or
            [UInt64]$entries[0].expires_at_unix_seconds -ne [UInt64]$publications[$provider].offer_expires_at_unix_seconds) {
            throw "aggregate provider publication does not bind the exact $provider offer"
        }
    }

    $boundaries = Read-M0987Evidence $Directory '08-boundaries.log'
    foreach ($line in @(
        'mailbox_https_retirement_boundary=verified',
        'runtime_cooperative_scheduling=verified',
        'runtime_mailbox_replication_recovery=verified',
        'service_free_mailbox_capability_v2=verified',
        'm0976_service_free_v2_kit_boundary=verified',
        'm0987_independent_provider_kit_boundary=verified'
    )) { Assert-M0987ExactLine $boundaries $line $line }

    [PSCustomObject]@{
        run_id = [string]$manifest.run_id
        build_commit = [string]$manifest.build_commit
        mailbox_capability_format = 'v2-exact-volunteer'
        central_service_descriptor_present = $false
        provider_machine_claims = 'distinct-2-of-2'
        provider_operator_claims = 'distinct-2-of-2'
        provider_network_claims = 'distinct-2-of-2'
        claim_strength = 'controlled-self-attestation-not-protocol-proof'
        sender_offline_before_receive = $true
        volunteer_receipts = '2-of-2'
        result = 'verified'
    }
}

function Write-M0987SelfTestJson {
    param([Parameter(Mandatory)] [string] $Path, [Parameter(Mandatory)] [object] $Value)
    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 6) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
}

function New-M0987SpecificSelfTestEvidence {
    param([Parameter(Mandatory)] [string] $Directory)
    New-Item -ItemType Directory -Path $Directory | Out-Null
    $runId = '20260920-120000'
    $commit = 'ab' * 20
    Write-M0987SelfTestJson (Join-Path $Directory 'manifest.json') ([ordered]@{
        schema = 1; run_id = $runId; build_commit = $commit; evidence_milestone = 'M0.9.87'
        https_fixture_present_at_start = $false; mailbox_capability_format = 'v2-exact-volunteer'
        central_service_descriptor_present = $false
    })
    [IO.File]::WriteAllLines((Join-Path $Directory '00-service-free-v2.boundary'), @(
        'field_phase=before-identity-and-mailbox-activation',
        'mailbox_capability_format=v2-exact-volunteer',
        'mailbox_service_descriptor_input=absent',
        'https_fixture_binary_present=false'
    ), [Text.UTF8Encoding]::new($false))
    foreach ($name in @('03-bob-mailbox-capability.log', '03-alice-mailbox-offer-import.log')) {
        [IO.File]::WriteAllLines((Join-Path $Directory $name), @(
            'mailbox_capability_format=v2-exact-volunteer', 'mailbox_service_descriptor=absent'
        ), [Text.UTF8Encoding]::new($false))
    }
    [IO.File]::WriteAllLines((Join-Path $Directory '04-send-alice.log'), @(
        'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
        'runtime_mailbox_http_put=not-attempted',
        'runtime_mailbox_delivery_durability=exact-volunteer-replication'
    ), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllLines((Join-Path $Directory '05-alice-offline.boundary'), @(
        'alice_runtime_ipc_reachable=false', 'mailbox_capability_format=v2-exact-volunteer',
        'mailbox_service_descriptor=absent',
        'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
        'runtime_mailbox_http_put=not-attempted'
    ), [Text.UTF8Encoding]::new($false))
    $entries = @()
    foreach ($item in @(@('provider1', '11', '21', '31'), @('provider2', '12', '22', '32'))) {
        $provider = $item[0]
        $offerPath = Join-Path $Directory "01-$provider.offer"
        [IO.File]::WriteAllText($offerPath, (('A' * 95) + $provider.Substring(8) + "`n"), [Text.UTF8Encoding]::new($false))
        $attestationPath = Join-Path $Directory "01-$provider-attestation.json"
        Write-M0987SelfTestJson $attestationPath ([ordered]@{
            schema = 1; evidence_milestone = 'M0.9.87'; provider_role = $provider
            run_id = $runId; build_commit = $commit; machine_pseudonym = $item[1] * 32
            operator_claim_digest = $item[2] * 32; network_claim_digest = $item[3] * 32
            digest_scope = 'run-scoped-sha256'
            claim_boundary = 'controlled-self-attestation-not-protocol-proof'
            private_state_shared = $false
        })
        $publication = [ordered]@{
            schema = 1; evidence_milestone = 'M0.9.87'; provider_role = $provider
            run_id = $runId; build_commit = $commit
            offer_sha256 = (Get-FileHash $offerPath -Algorithm SHA256).Hash.ToLowerInvariant()
            offer_expires_at_unix_seconds = [UInt64]9999999999
            attestation_sha256 = (Get-FileHash $attestationPath -Algorithm SHA256).Hash.ToLowerInvariant()
            published_at_unix_seconds = [UInt64]1
        }
        Write-M0987SelfTestJson (Join-Path $Directory "01-$provider-publication.json") $publication
        $entries += [ordered]@{
            name = $provider; sha256 = $publication.offer_sha256
            expires_at_unix_seconds = $publication.offer_expires_at_unix_seconds
        }
    }
    Write-M0987SelfTestJson (Join-Path $Directory '01-provider-offers-publication.json') ([ordered]@{
        schema = 1; evidence_milestone = 'M0.9.87'
        claim_boundary = 'controlled-self-attestation-not-protocol-proof'
        published_at_unix_seconds = [UInt64]1; providers = $entries
    })
    [IO.File]::WriteAllLines((Join-Path $Directory '08-boundaries.log'), @(
        'mailbox_https_retirement_boundary=verified',
        'runtime_cooperative_scheduling=verified',
        'runtime_mailbox_replication_recovery=verified',
        'service_free_mailbox_capability_v2=verified',
        'm0976_service_free_v2_kit_boundary=verified',
        'm0987_independent_provider_kit_boundary=verified'
    ), [Text.UTF8Encoding]::new($false))
}

function Set-M0987SelfTestClaim {
    param(
        [Parameter(Mandatory)] [string] $Directory,
        [Parameter(Mandatory)] [string] $Field,
        [Parameter(Mandatory)] [string] $Value
    )
    $attestationPath = Join-Path $Directory '01-provider2-attestation.json'
    $attestation = Get-Content $attestationPath -Raw | ConvertFrom-Json
    $attestation.$Field = $Value
    Write-M0987SelfTestJson $attestationPath $attestation
    $publicationPath = Join-Path $Directory '01-provider2-publication.json'
    $publication = Get-Content $publicationPath -Raw | ConvertFrom-Json
    $publication.attestation_sha256 = (Get-FileHash $attestationPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-M0987SelfTestJson $publicationPath $publication
}

if ($SelfTest) {
    $root = Join-Path ([IO.Path]::GetTempPath()) ("kilogram-m0987-verifier-" + [Guid]::NewGuid().ToString('N'))
    try {
        & (Join-Path $PSScriptRoot 'verify-kilogram-m0969-exact-locator-evidence.ps1') `
            -SelfTest -LabelPrefix 'm0987' | Out-Null
        New-M0987SpecificSelfTestEvidence $root
        if ((Test-M0987IndependentProviderEvidence $root -SkipInheritedExactLocator).result -cne 'verified') {
            throw 'positive M0.9.87 verifier self-test failed'
        }
        $negativeCases = @(
            @('machine_pseudonym', ('11' * 32), 'same machine'),
            @('operator_claim_digest', ('21' * 32), 'same operator'),
            @('network_claim_digest', ('31' * 32), 'same network')
        )
        foreach ($case in $negativeCases) {
            Remove-Item -LiteralPath $root -Recurse -Force
            New-M0987SpecificSelfTestEvidence $root
            Set-M0987SelfTestClaim $root $case[0] $case[1]
            $rejected = $false
            try { $null = Test-M0987IndependentProviderEvidence $root -SkipInheritedExactLocator } catch { $rejected = $true }
            if (-not $rejected) { throw "M0.9.87 verifier accepted $($case[2]) claims" }
        }
        Remove-Item -LiteralPath $root -Recurse -Force
        New-M0987SpecificSelfTestEvidence $root
        $attestationPath = Join-Path $root '01-provider2-attestation.json'
        $attestation = Get-Content $attestationPath -Raw | ConvertFrom-Json
        $attestation | Add-Member -NotePropertyName raw_operator_label -NotePropertyValue 'forbidden'
        Write-M0987SelfTestJson $attestationPath $attestation
        $publicationPath = Join-Path $root '01-provider2-publication.json'
        $publication = Get-Content $publicationPath -Raw | ConvertFrom-Json
        $publication.attestation_sha256 = (Get-FileHash $attestationPath -Algorithm SHA256).Hash.ToLowerInvariant()
        Write-M0987SelfTestJson $publicationPath $publication
        $rejected = $false
        try { $null = Test-M0987IndependentProviderEvidence $root -SkipInheritedExactLocator } catch { $rejected = $true }
        if (-not $rejected) { throw 'M0.9.87 verifier accepted an unapproved raw claim field' }

        Write-Output 'm0987_independent_provider_evidence_self_test=verified'
        Write-Output 'same_machine_claim_rejected=true'
        Write-Output 'same_operator_claim_rejected=true'
        Write-Output 'same_network_claim_rejected=true'
        Write-Output 'raw_claim_field_rejected=true'
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
$report = Test-M0987IndependentProviderEvidence $resolved
$report | Format-List
