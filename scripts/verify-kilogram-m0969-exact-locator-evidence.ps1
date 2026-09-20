[CmdletBinding()]
param(
    [string] $EvidenceDirectory,
    [switch] $SelfTest,
    [ValidateSet('m0969', 'm0972', 'm0973', 'm0974', 'm0976', 'm0987')] [string] $LabelPrefix = 'm0969',
    [switch] $SuppressReport
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-M0969Evidence {
    param([Parameter(Mandatory)] [string] $Directory, [Parameter(Mandatory)] [string] $Name)
    $path = Join-Path $Directory $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required M0.9.69 evidence is missing: $Name"
    }
    $item = Get-Item -LiteralPath $path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $item.Length -gt 4MB) {
        throw "M0.9.69 evidence must be a bounded regular file: $Name"
    }
    return (Get-Content -LiteralPath $path -Raw).Replace("`r`n", "`n")
}

function Assert-M0969Match {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Description
    )
    if (-not [regex]::IsMatch($Text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline)) {
        throw "M0.9.69 evidence does not prove $Description"
    }
}

function Get-M0969Matches {
    param([Parameter(Mandatory)] [string] $Text, [Parameter(Mandatory)] [string] $Pattern)
    return @([regex]::Matches(
        $Text,
        $Pattern,
        [Text.RegularExpressions.RegexOptions]::Multiline
    ) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
}

function Assert-M0969SameSet {
    param(
        [Parameter(Mandatory)] [string[]] $Expected,
        [Parameter(Mandatory)] [string[]] $Actual,
        [Parameter(Mandatory)] [string] $Description
    )
    if ($Expected.Count -ne $Actual.Count -or @(Compare-Object $Expected $Actual).Count -ne 0) {
        throw "M0.9.69 evidence has a different $Description"
    }
}

function Test-M0969ExactLocatorEvidence {
    param(
        [Parameter(Mandatory)] [string] $Directory,
        [ValidateSet('m0969', 'm0972', 'm0973', 'm0974', 'm0976', 'm0987')] [string] $ExpectedLabelPrefix = 'm0969'
    )

    $expectedRoutePolicy = 'auto'
    $expectedRelayUrl = 'https://aps1-1.relay.n0.iroh.link./'
    $expectedRoutePattern = '^route_policy=' + [regex]::Escape($expectedRoutePolicy) + '$'
    $expectedRelayPattern = '^relay_home_url=' + [regex]::Escape($expectedRelayUrl) + '$'
    $manifestText = Read-M0969Evidence $Directory 'manifest.json'
    $manifest = $manifestText | ConvertFrom-Json
    if ([int]$manifest.schema -ne 1) { throw 'M0.9.69 manifest schema must be exactly 1' }
    foreach ($name in @(
        'run_id', 'build_commit', 'conversation_label', 'alice_account_id',
        'bob_account_id', 'message_marker', 'route_policy', 'relay_url'
    )) {
        if (-not ($manifest.PSObject.Properties.Name -contains $name) -or
            [string]::IsNullOrWhiteSpace([string]$manifest.$name)) {
            throw "M0.9.69 manifest value is missing: $name"
        }
    }
    if ([string]$manifest.run_id -notmatch '^[0-9]{8}-[0-9]{6}$' -or
        [string]$manifest.build_commit -cnotmatch '^[0-9a-f]{40}$' -or
        [string]$manifest.alice_account_id -cnotmatch '^[0-9a-f]{64}$' -or
        [string]$manifest.bob_account_id -cnotmatch '^[0-9a-f]{64}$' -or
        [string]$manifest.conversation_label -notmatch ("^$([regex]::Escape($ExpectedLabelPrefix))-[0-9]{8}-[0-9]{6}`$") -or
        [string]$manifest.message_marker -notmatch ("^kilogram-$([regex]::Escape($ExpectedLabelPrefix))-[0-9]{8}-[0-9]{6}`$") -or
        [string]$manifest.route_policy -cne $expectedRoutePolicy -or
        [string]$manifest.relay_url -cne $expectedRelayUrl) {
        throw 'M0.9.69 manifest has an invalid canonical identity or label'
    }

    $providerKeys = [Collections.Generic.List[string]]::new()
    $transportIds = [Collections.Generic.List[string]]::new()
    foreach ($provider in @('provider1', 'provider2')) {
        $log = Read-M0969Evidence $Directory "01-$provider.log"
        Assert-M0969Match $log '^runtime_volunteer_storage=serving$' "$provider serving state"
        Assert-M0969Match `
            $log '^runtime_volunteer_storage_iroh_alpn=kilogram/m0/blind-mailbox/1$' `
            "$provider Iroh ALPN"
        Assert-M0969Match $log $expectedRoutePattern "$provider auto route policy"
        Assert-M0969Match $log $expectedRelayPattern "$provider pinned field relay"
        Assert-M0969Match $log '^status=runtime-listening$' "$provider listening state"
        $keys = @(Get-M0969Matches $log '^runtime_volunteer_storage_store_key=([0-9a-f]{64})$')
        $transports = @(Get-M0969Matches $log '^transport_endpoint_id=([0-9a-f]{64})$')
        if ($keys.Count -ne 1 -or $transports.Count -ne 1) {
            throw "$provider must expose one stable store key and transport identity"
        }
        $providerKeys.Add($keys[0])
        $transportIds.Add($transports[0])
        $offerPath = Join-Path $Directory "01-$provider.offer"
        $offer = (Read-M0969Evidence $Directory "01-$provider.offer").Trim()
        if ($offer -notmatch '^[A-Za-z0-9_-]{64,8192}$') {
            throw "$provider offer is not one bounded base64url value"
        }
        if (-not (Test-Path -LiteralPath $offerPath -PathType Leaf)) { throw "$provider offer is absent" }
    }
    $providerSet = @($providerKeys | Sort-Object -Unique)
    if ($providerSet.Count -ne 2 -or @($transportIds | Sort-Object -Unique).Count -ne 2) {
        throw 'M0.9.69 providers are not store-key and transport distinct'
    }

    $publication = (Read-M0969Evidence $Directory '01-provider-offers-publication.json') |
        ConvertFrom-Json
    if ([int]$publication.schema -ne 1 -or @($publication.providers).Count -ne 2) {
        throw 'provider publication manifest has an unexpected shape'
    }
    $publicationNames = @($publication.providers | ForEach-Object { [string]$_.name } | Sort-Object -Unique)
    Assert-M0969SameSet @('provider1', 'provider2') $publicationNames 'provider publication names'
    foreach ($entry in @($publication.providers)) {
        $path = Join-Path $Directory "01-$([string]$entry.name).offer"
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne
            ([string]$entry.sha256).ToUpperInvariant()) {
            throw 'provider publication hash does not match the retained offer'
        }
    }

    foreach ($entry in @(
        @('02-bob-providers-before-activation.log', 'bob-before-activation'),
        @('04-alice-providers-before-send.log', 'alice-before-send'),
        @('06-bob-providers-before-receive.log', 'bob-before-receive')
    )) {
        $imports = Read-M0969Evidence $Directory $entry[0]
        Assert-M0969Match $imports "^field_role=$([regex]::Escape($entry[1]))$" "$($entry[1]) role"
        Assert-M0969Match `
            $imports '^field_provider_snapshot=manifest-hash-consistent$' `
            "$($entry[1]) consistent offer snapshot"
        Assert-M0969Match $imports '^status=runtime-volunteer-providers-selected$' "$($entry[1]) selection"
        $imported = @(Get-M0969Matches $imports 'store_key=([0-9a-f]{64}) policy_class=bounded')
        Assert-M0969SameSet $providerSet $imported "$($entry[1]) imported provider set"
    }

    $activation = Read-M0969Evidence $Directory '03-bob-mailbox-capability.log'
    Assert-M0969Match $activation '^mailbox_replica_set_store_count=2$' 'two-store activation commitment'
    Assert-M0969Match `
        $activation '^mailbox_replica_set_discovery=exact-authenticated$' `
        'exact authenticated activation'
    Assert-M0969Match $activation '^status=runtime-mailbox-offer-created$' 'mailbox offer creation'
    $commitments = @(Get-M0969Matches $activation '^mailbox_replica_set_commitment_id=([0-9a-f]{64})$')
    $updateIds = @(Get-M0969Matches $activation '^mailbox_capability_update_id=([0-9a-f]{64})$')
    if ($commitments.Count -ne 1 -or $updateIds.Count -ne 1) {
        throw 'activation must expose one replica-set commitment and one capability update'
    }
    $commitmentId = $commitments[0]
    $updateId = $updateIds[0]

    foreach ($entry in @(
        @('03-alice-live-contact-refresh.log', [string]$manifest.bob_account_id, 'Alice'),
        @('03-bob-live-contact-refresh.log', [string]$manifest.alice_account_id, 'Bob')
    )) {
        $refresh = Read-M0969Evidence $Directory $entry[0]
        $bootstrapHashes = @(Get-M0969Matches $refresh '^bootstrap_ticket_sha256=([0-9a-f]{64})$')
        $liveHashes = @(Get-M0969Matches $refresh '^live_ticket_sha256=([0-9a-f]{64})$')
        if ($bootstrapHashes.Count -ne 1 -or $liveHashes.Count -ne 1 -or
            $bootstrapHashes[0] -ceq $liveHashes[0]) {
            throw "$($entry[2]) did not prove a fresh runtime ticket replaced the bootstrap ticket"
        }
        Assert-M0969Match `
            $refresh "^peer_account_id=$([regex]::Escape($entry[1]))$" `
            "$($entry[2]) live peer identity"
        Assert-M0969Match `
            $refresh '^runtime_contact_update=live-ipc$' `
            "$($entry[2]) live runtime contact validation"
        Assert-M0969Match `
            $refresh '^endpoint_candidate_added=false$' `
            "$($entry[2]) refresh of the already enrolled peer Device"
        Assert-M0969Match `
            $refresh '^runtime_contact_store=AlreadyPresent$' `
            "$($entry[2]) stable contact identity during ticket refresh"
        Assert-M0969Match $refresh '^status=runtime-contact-ready$' "$($entry[2]) fresh ticket acceptance"
    }

    $aliceConvergence = Read-M0969Evidence $Directory '03-alice-capability-convergence.log'
    $bobConvergence = Read-M0969Evidence $Directory '03-bob-capability-convergence.log'
    Assert-M0969Match $aliceConvergence $expectedRoutePattern 'Alice convergence auto route policy'
    Assert-M0969Match $aliceConvergence $expectedRelayPattern 'Alice convergence pinned field relay'
    Assert-M0969Match $bobConvergence $expectedRoutePattern 'Bob convergence auto route policy'
    Assert-M0969Match $bobConvergence $expectedRelayPattern 'Bob convergence pinned field relay'
    Assert-M0969Match `
        $aliceConvergence "^mailbox_capability_update_id=$([regex]::Escape($updateId))$" `
        'Alice application of the exact capability update'
    Assert-M0969Match `
        $aliceConvergence '^status=runtime-mailbox-capability-updated$' `
        'Alice capability update completion'
    Assert-M0969Match `
        $bobConvergence "^mailbox_capability_update_id=$([regex]::Escape($updateId))$" `
        'Bob acknowledgement of the exact capability update'
    Assert-M0969Match `
        $bobConvergence '^runtime_mailbox_capability_update_status=acknowledged$' `
        'Bob capability acknowledgement completion'

    $queue = Read-M0969Evidence $Directory '04-alice-queue.log'
    Assert-M0969Match $queue '^status=runtime-message-queued$' 'Alice durable queue insertion'
    $send = Read-M0969Evidence $Directory '04-send-alice.log'
    Assert-M0969Match $send $expectedRoutePattern 'Alice send auto route policy'
    Assert-M0969Match $send $expectedRelayPattern 'Alice send pinned field relay'
    Assert-M0969Match $send '^runtime_outbound_status=mailbox-stored$' 'compatibility acceptance'
    Assert-M0969Match `
        $send '^runtime_mailbox_replica_set_discovery=exact-authenticated$' `
        'Alice exact provider discovery'
    Assert-M0969Match $send '^runtime_mailbox_replica_set_resolved=2/2$' 'Alice exact 2/2 resolution'
    Assert-M0969Match $send '^runtime_mailbox_replication_receipts=2/2$' 'Alice two receipts'
    Assert-M0969Match $send '^runtime_mailbox_replication_status=satisfied$' 'Alice replication completion'
    $sendCommitments = @(Get-M0969Matches $send '^runtime_mailbox_replica_set_commitment_id=([0-9a-f]{64})$')
    if ($sendCommitments.Count -ne 1 -or $sendCommitments[0] -cne $commitmentId) {
        throw 'Alice sender commitment differs from Bob activation'
    }
    $attemptKeys = @(Get-M0969Matches `
        $send '^runtime_mailbox_replication_provider_attempt_store_key=([0-9a-f]{64})$')
    $receiptKeys = @(Get-M0969Matches `
        $send '^runtime_mailbox_replication_receipt_store_key=([0-9a-f]{64})$')
    Assert-M0969SameSet $providerSet $attemptKeys 'Alice exact attempt set'
    Assert-M0969SameSet $providerSet $receiptKeys 'Alice signed receipt set'

    $offline = Read-M0969Evidence $Directory '05-alice-offline.boundary'
    Assert-M0969Match $offline '^alice_replication_status=satisfied$' 'replication before Alice shutdown'
    Assert-M0969Match `
        $offline "^alice_replica_set_commitment_id=$([regex]::Escape($commitmentId))$" `
        'offline boundary commitment'
    Assert-M0969Match $offline '^alice_replica_discovery=exact-authenticated$' 'offline exact discovery'
    Assert-M0969Match $offline '^alice_runtime_ipc_reachable=false$' 'Alice offline before Bob receive'

    $receive = Read-M0969Evidence $Directory '06-receive-bob.log'
    Assert-M0969Match $receive $expectedRoutePattern 'Bob receive auto route policy'
    Assert-M0969Match $receive $expectedRelayPattern 'Bob receive pinned field relay'
    Assert-M0969Match `
        $receive '^runtime_mailbox_replica_set_discovery=exact-authenticated$' `
        'Bob exact provider discovery'
    Assert-M0969Match $receive '^runtime_mailbox_replica_set_resolved=2/2$' 'Bob exact 2/2 resolution'
    $receiveCommitments = @(Get-M0969Matches `
        $receive '^runtime_mailbox_replica_set_commitment_id=([0-9a-f]{64})$')
    if ($receiveCommitments.Count -ne 1 -or $receiveCommitments[0] -cne $commitmentId) {
        throw 'Bob receiver commitment differs from activation and sender'
    }
    $pollKeys = @(Get-M0969Matches $receive '^runtime_mailbox_replica_poll_store_key=([0-9a-f]{64})$')
    $sourceKeys = @(Get-M0969Matches $receive '^runtime_mailbox_replica_source_store_key=([0-9a-f]{64})$')
    Assert-M0969SameSet $providerSet $pollKeys 'Bob exact poll set'
    Assert-M0969SameSet $providerSet $sourceKeys 'Bob committed source set'
    if ([regex]::Matches($receive, '(?m)^runtime_mailbox_inbound_source=volunteer-iroh$').Count -lt 2 -or
        [regex]::Matches(
            $receive,
            '(?m)^runtime_mailbox_replica_delete_status=deleted-after-commit$'
        ).Count -lt 2) {
        throw 'Bob did not commit and delete both exact volunteer replicas'
    }

    $restart = Read-M0969Evidence $Directory '07-restart-bob.log'
    Assert-M0969Match $restart $expectedRoutePattern 'Bob restart auto route policy'
    Assert-M0969Match $restart $expectedRelayPattern 'Bob restart pinned field relay'
    Assert-M0969Match $restart '^status=runtime-listening$' 'Bob restart from retained state'
    if ([regex]::IsMatch($restart, '(?m)^runtime_mailbox_inbound_source=volunteer-iroh$')) {
        throw 'a deleted volunteer replica was delivered again after restart'
    }
    $history = Read-M0969Evidence $Directory '07-bob-history.log'
    $messageCount = [regex]::Matches(
        $history,
        [regex]::Escape([string]$manifest.message_marker)
    ).Count
    if ($messageCount -ne 1) {
        throw "Bob history must contain the exact test message once, found $messageCount"
    }

    foreach ($text in @($activation, $aliceConvergence, $bobConvergence, $send, $receive, $restart)) {
        if ([regex]::IsMatch(
            $text,
            '(?m)^(?:runtime_)?mailbox_replica_set_discovery=legacy-random-fallback$'
        )) { throw 'legacy-random-fallback is forbidden in a clean exact-locator run' }
    }

    $boundaries = Read-M0969Evidence $Directory '08-boundaries.log'
    foreach ($line in @(
        'runtime_mailbox_flow=verified',
        'volunteer_storage_boundary=verified',
        'volunteer_iroh_boundary=verified',
        'volunteer_provider_selection_boundary=verified',
        'volunteer_provider_gossip_boundary=verified',
        'volunteer_replication_boundary=verified',
        'volunteer_retrieval_boundary=verified',
        'volunteer_replica_set_locator_boundary=verified',
        'm0969_exact_locator_kit_boundary=verified'
    )) {
        Assert-M0969Match $boundaries "^$([regex]::Escape($line))$" $line
    }

    [PSCustomObject]@{
        run_id = [string]$manifest.run_id
        build_commit = [string]$manifest.build_commit
        route_policy = $expectedRoutePolicy
        relay_url = $expectedRelayUrl
        replica_set_commitment_id = $commitmentId
        provider_store_keys = ($providerSet -join ',')
        activation = 'providers-before-capability'
        capability_convergence = 'recipient-applied-owner-acknowledged'
        sender_resolution = 'exact-2-of-2'
        sender_receipts = '2-of-2'
        sender_offline_before_receive = $true
        recipient_resolution = 'exact-2-of-2'
        recipient_commit_delete = '2-of-2'
        legacy_random_fallback = $false
        restart_redelivery = 'absent'
        history_message_occurrences = 1
        https_compatibility_copy = $ExpectedLabelPrefix -cnotin @('m0976', 'm0987')
        result = 'verified'
    }
}

function New-M0969SelfTestEvidence {
    param(
        [Parameter(Mandatory)] [string] $Directory,
        [ValidateSet('m0969', 'm0972', 'm0973', 'm0974', 'm0976', 'm0987')]
        [string] $ExpectedLabelPrefix = 'm0969'
    )
    New-Item -ItemType Directory -Path $Directory | Out-Null
    $p1 = '11' * 32
    $p2 = '22' * 32
    $t1 = '31' * 32
    $t2 = '32' * 32
    $commitment = '44' * 32
    $update = '55' * 32
    $message = "kilogram-$ExpectedLabelPrefix-20260919-120000"
    @{
        schema = 1
        run_id = '20260919-120000'
        build_commit = 'ab' * 20
        conversation_label = "$ExpectedLabelPrefix-20260919-120000"
        alice_account_id = 'aa' * 32
        bob_account_id = 'bb' * 32
        message_marker = $message
        route_policy = 'auto'
        relay_url = 'https://aps1-1.relay.n0.iroh.link./'
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'manifest.json') -Encoding utf8
    foreach ($entry in @(@('provider1', $p1, $t1), @('provider2', $p2, $t2))) {
        Set-Content -LiteralPath (Join-Path $Directory "01-$($entry[0]).log") -Encoding utf8 -Value @(
            "transport_endpoint_id=$($entry[2])",
            'route_policy=auto',
            'relay_home_url=https://aps1-1.relay.n0.iroh.link./',
            'runtime_volunteer_storage=serving',
            "runtime_volunteer_storage_store_key=$($entry[1])",
            'runtime_volunteer_storage_iroh_alpn=kilogram/m0/blind-mailbox/1',
            'status=runtime-listening'
        )
        Set-Content -LiteralPath (Join-Path $Directory "01-$($entry[0]).offer") `
            -Encoding utf8 -Value ('A' * 96)
    }
    $publication = @{
        schema = 1
        published_at_unix_seconds = 1
        providers = @(
            @{ name = 'provider1'; sha256 = (Get-FileHash (Join-Path $Directory '01-provider1.offer') -Algorithm SHA256).Hash; expires_at_unix_seconds = 9999999999 },
            @{ name = 'provider2'; sha256 = (Get-FileHash (Join-Path $Directory '01-provider2.offer') -Algorithm SHA256).Hash; expires_at_unix_seconds = 9999999999 }
        )
    }
    $publication | ConvertTo-Json -Depth 4 |
        Set-Content -LiteralPath (Join-Path $Directory '01-provider-offers-publication.json') -Encoding utf8
    $imports = @(
        'field_provider_snapshot=manifest-hash-consistent',
        "provider_offer_id=$('61' * 32) transport_identity=$t1 store_key=$p1 policy_class=bounded",
        "provider_offer_id=$('62' * 32) transport_identity=$t2 store_key=$p2 policy_class=bounded",
        'status=runtime-volunteer-providers-selected'
    )
    foreach ($entry in @(
        @('02-bob-providers-before-activation.log', 'bob-before-activation'),
        @('04-alice-providers-before-send.log', 'alice-before-send'),
        @('06-bob-providers-before-receive.log', 'bob-before-receive')
    )) {
        Set-Content -LiteralPath (Join-Path $Directory $entry[0]) -Encoding utf8 `
            -Value (@("field_role=$($entry[1])") + $imports)
    }
    Set-Content -LiteralPath (Join-Path $Directory '03-bob-mailbox-capability.log') -Encoding utf8 -Value @(
        "mailbox_capability_update_id=$update",
        "mailbox_replica_set_commitment_id=$commitment",
        'mailbox_replica_set_store_count=2',
        'mailbox_replica_set_discovery=exact-authenticated',
        'status=runtime-mailbox-offer-created'
    )
    Set-Content -LiteralPath (Join-Path $Directory '03-alice-mailbox-offer-import.log') `
        -Encoding utf8 -Value 'status=runtime-mailbox-offer-imported'
    foreach ($entry in @(
        @('03-alice-live-contact-refresh.log', ('bb' * 32)),
        @('03-bob-live-contact-refresh.log', ('aa' * 32))
    )) {
        Set-Content -LiteralPath (Join-Path $Directory $entry[0]) -Encoding utf8 -Value @(
            "bootstrap_ticket_sha256=$('71' * 32)",
            "live_ticket_sha256=$('72' * 32)",
            "peer_account_id=$($entry[1])",
            'endpoint_candidate_added=false',
            'runtime_contact_update=live-ipc',
            'runtime_contact_store=AlreadyPresent',
            'status=runtime-contact-ready'
        )
    }
    Set-Content -LiteralPath (Join-Path $Directory '03-alice-capability-convergence.log') -Encoding utf8 -Value @(
        'route_policy=auto',
        'relay_home_url=https://aps1-1.relay.n0.iroh.link./',
        "mailbox_capability_update_id=$update",
        'mailbox_capability_update_store=Inserted',
        'status=runtime-mailbox-capability-updated'
    )
    Set-Content -LiteralPath (Join-Path $Directory '03-bob-capability-convergence.log') -Encoding utf8 -Value @(
        'route_policy=auto',
        'relay_home_url=https://aps1-1.relay.n0.iroh.link./',
        "mailbox_capability_update_id=$update",
        'runtime_mailbox_capability_update_status=acknowledged'
    )
    Set-Content -LiteralPath (Join-Path $Directory '04-alice-queue.log') `
        -Encoding utf8 -Value 'status=runtime-message-queued'
    Set-Content -LiteralPath (Join-Path $Directory '04-send-alice.log') -Encoding utf8 -Value @(
        'route_policy=auto',
        'relay_home_url=https://aps1-1.relay.n0.iroh.link./',
        'runtime_outbound_status=mailbox-stored',
        "runtime_mailbox_replica_set_commitment_id=$commitment",
        'runtime_mailbox_replica_set_resolved=2/2',
        'runtime_mailbox_replica_set_discovery=exact-authenticated',
        "runtime_mailbox_replication_provider_attempt_store_key=$p1",
        "runtime_mailbox_replication_provider_attempt_store_key=$p2",
        "runtime_mailbox_replication_receipt_store_key=$p1",
        "runtime_mailbox_replication_receipt_store_key=$p2",
        'runtime_mailbox_replication_receipts=2/2',
        'runtime_mailbox_replication_status=satisfied'
    )
    Set-Content -LiteralPath (Join-Path $Directory '05-alice-offline.boundary') -Encoding utf8 -Value @(
        'alice_replication_status=satisfied',
        "alice_replica_set_commitment_id=$commitment",
        'alice_replica_discovery=exact-authenticated',
        'alice_runtime_ipc_reachable=false'
    )
    Set-Content -LiteralPath (Join-Path $Directory '06-receive-bob.log') -Encoding utf8 -Value @(
        'route_policy=auto',
        'relay_home_url=https://aps1-1.relay.n0.iroh.link./',
        "runtime_mailbox_replica_set_commitment_id=$commitment",
        'runtime_mailbox_replica_set_resolved=2/2',
        'runtime_mailbox_replica_set_discovery=exact-authenticated',
        "runtime_mailbox_replica_poll_store_key=$p1",
        "runtime_mailbox_replica_source_store_key=$p1",
        'runtime_mailbox_inbound_source=volunteer-iroh',
        'runtime_mailbox_replica_delete_status=deleted-after-commit',
        "runtime_mailbox_replica_poll_store_key=$p2",
        "runtime_mailbox_replica_source_store_key=$p2",
        'runtime_mailbox_inbound_source=volunteer-iroh',
        'runtime_mailbox_replica_delete_status=deleted-after-commit'
    )
    Set-Content -LiteralPath (Join-Path $Directory '07-restart-bob.log') `
        -Encoding utf8 -Value @(
            'route_policy=auto',
            'relay_home_url=https://aps1-1.relay.n0.iroh.link./',
            'status=runtime-listening'
        )
    Set-Content -LiteralPath (Join-Path $Directory '07-bob-history.log') `
        -Encoding utf8 -Value "payload=text body=`"$message`""
    Set-Content -LiteralPath (Join-Path $Directory '08-boundaries.log') -Encoding utf8 -Value @(
        'runtime_mailbox_flow=verified',
        'volunteer_storage_boundary=verified',
        'volunteer_iroh_boundary=verified',
        'volunteer_provider_selection_boundary=verified',
        'volunteer_provider_gossip_boundary=verified',
        'volunteer_replication_boundary=verified',
        'volunteer_retrieval_boundary=verified',
        'volunteer_replica_set_locator_boundary=verified',
        'm0969_exact_locator_kit_boundary=verified'
    )
}

if ($SelfTest) {
    $root = Join-Path ([IO.Path]::GetTempPath()) ("kilogram-m0969-verifier-" + [Guid]::NewGuid().ToString('N'))
    try {
        New-M0969SelfTestEvidence $root $LabelPrefix
        $positive = Test-M0969ExactLocatorEvidence $root $LabelPrefix
        if ($positive.result -cne 'verified') { throw 'positive M0.9.69 verifier self-test failed' }

        $sendPath = Join-Path $root '04-send-alice.log'
        $sendOriginal = Get-Content -LiteralPath $sendPath -Raw
        Set-Content -LiteralPath $sendPath -Encoding utf8 -Value (
            $sendOriginal.Replace(
                'runtime_mailbox_replica_set_discovery=exact-authenticated',
                'runtime_mailbox_replica_set_discovery=legacy-random-fallback'
            )
        )
        $legacyRejected = $false
        try { $null = Test-M0969ExactLocatorEvidence $root $LabelPrefix }
        catch { $legacyRejected = $true }
        if (-not $legacyRejected) { throw 'verifier accepted legacy random fallback' }
        Set-Content -LiteralPath $sendPath -Encoding utf8 -Value $sendOriginal

        $receivePath = Join-Path $root '06-receive-bob.log'
        $receiveOriginal = Get-Content -LiteralPath $receivePath -Raw
        Set-Content -LiteralPath $receivePath -Encoding utf8 -Value (
            $receiveOriginal.Replace(
                ('runtime_mailbox_replica_poll_store_key=' + ('22' * 32)),
                ('runtime_mailbox_replica_poll_store_key=' + ('99' * 32))
            )
        )
        $substitutionRejected = $false
        try { $null = Test-M0969ExactLocatorEvidence $root $LabelPrefix }
        catch { $substitutionRejected = $true }
        if (-not $substitutionRejected) { throw 'verifier accepted a provider outside the committed set' }

        Set-Content -LiteralPath $receivePath -Encoding utf8 -Value $receiveOriginal
        $refreshPath = Join-Path $root '03-alice-live-contact-refresh.log'
        $refreshOriginal = Get-Content -LiteralPath $refreshPath -Raw
        Set-Content -LiteralPath $refreshPath -Encoding utf8 -Value (
            $refreshOriginal.Replace(
                ('live_ticket_sha256=' + ('72' * 32)),
                ('live_ticket_sha256=' + ('71' * 32))
            )
        )
        $staleTicketRejected = $false
        try { $null = Test-M0969ExactLocatorEvidence $root $LabelPrefix }
        catch { $staleTicketRejected = $true }
        if (-not $staleTicketRejected) { throw 'verifier accepted a stale bootstrap endpoint ticket' }
        Set-Content -LiteralPath $refreshPath -Encoding utf8 -Value $refreshOriginal

        $bobConvergencePath = Join-Path $root '03-bob-capability-convergence.log'
        $bobConvergenceOriginal = Get-Content -LiteralPath $bobConvergencePath -Raw
        Set-Content -LiteralPath $bobConvergencePath -Encoding utf8 -Value (
            $bobConvergenceOriginal.Replace(
                'relay_home_url=https://aps1-1.relay.n0.iroh.link./',
                'relay_home_url=https://euc1-1.relay.n0.iroh.link./'
            )
        )
        $relayMismatchRejected = $false
        try { $null = Test-M0969ExactLocatorEvidence $root $LabelPrefix }
        catch { $relayMismatchRejected = $true }
        if (-not $relayMismatchRejected) { throw 'verifier accepted a divergent field relay' }

        Write-Output 'm0969_exact_locator_evidence_self_test=verified'
        Write-Output 'legacy_random_fallback_rejected=true'
        Write-Output 'provider_substitution_rejected=true'
        Write-Output 'stale_endpoint_ticket_rejected=true'
        Write-Output 'relay_mismatch_rejected=true'
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
$report = Test-M0969ExactLocatorEvidence $resolved $LabelPrefix
if (-not $SuppressReport) { $report | Format-List }
