[CmdletBinding()]
param(
    [string] $EvidenceDirectory,
    [switch] $SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-FieldText {
    param([Parameter(Mandatory)] [string] $Directory, [Parameter(Mandatory)] [string] $Name)
    $path = Join-Path $Directory $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required volunteer field evidence is missing: $Name"
    }
    $item = Get-Item -LiteralPath $path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $item.Length -gt 4MB) {
        throw "volunteer field evidence must be a bounded regular file: $Name"
    }
    return (Get-Content -LiteralPath $path -Raw).Replace("`r`n", "`n")
}

function Assert-FieldMatch {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Description
    )
    if (-not [regex]::IsMatch($Text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline)) {
        throw "volunteer field evidence does not prove $Description"
    }
}

function Get-UniqueMatches {
    param([Parameter(Mandatory)] [string] $Text, [Parameter(Mandatory)] [string] $Pattern)
    return @([regex]::Matches($Text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline) |
        ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
}

function Test-VolunteerFieldEvidence {
    param([Parameter(Mandatory)] [string] $Directory)

    $manifestText = Read-FieldText $Directory 'manifest.json'
    $manifest = $manifestText | ConvertFrom-Json
    if ($manifest.schema -ne 1) { throw 'volunteer field manifest schema must be exactly 1' }
    foreach ($name in @('run_id', 'build_commit', 'conversation_label', 'alice_account_id', 'bob_account_id', 'message_marker')) {
        if (-not ($manifest.PSObject.Properties.Name -contains $name) -or
            [string]::IsNullOrWhiteSpace([string]$manifest.$name)) {
            throw "volunteer field manifest value is missing: $name"
        }
    }
    if ([string]$manifest.run_id -notmatch '^[0-9]{8}-[0-9]{6}$' -or
        [string]$manifest.build_commit -cnotmatch '^[0-9a-f]{40}$') {
        throw 'volunteer field manifest has an invalid run ID or build commit'
    }
    foreach ($name in @('alice_account_id', 'bob_account_id')) {
        if ([string]$manifest.$name -cnotmatch '^[0-9a-f]{64}$') {
            throw "volunteer field manifest has an invalid account ID: $name"
        }
    }
    if ([string]$manifest.conversation_label -notmatch '^[A-Za-z0-9._-]{1,80}$' -or
        [string]$manifest.message_marker -notmatch '^kilogram-m0967-[A-Za-z0-9._-]{1,96}$') {
        throw 'conversation label or unique message marker is invalid'
    }

    $providerKeys = [Collections.Generic.List[string]]::new()
    foreach ($provider in @('provider1', 'provider2')) {
        $log = Read-FieldText $Directory "01-$provider.log"
        Assert-FieldMatch $log '^runtime_volunteer_storage=serving$' "$provider serving state"
        Assert-FieldMatch $log '^runtime_volunteer_storage_iroh_alpn=kilogram/m0/blind-mailbox/1$' "$provider dedicated Iroh ALPN"
        Assert-FieldMatch $log '^status=runtime-listening$' "$provider listening state"
        $keys = @(Get-UniqueMatches $log '^runtime_volunteer_storage_store_key=([0-9a-f]{64})$')
        if ($keys.Count -ne 1) { throw "$provider must expose exactly one store key" }
        $providerKeys.Add($keys[0])
        $offer = (Read-FieldText $Directory "01-$provider.offer").Trim()
        if ($offer -notmatch '^[A-Za-z0-9_-]{64,8192}$') {
            throw "$provider offer file is not one bounded base64url value"
        }
    }
    if (@($providerKeys | Sort-Object -Unique).Count -ne 2) {
        throw 'field providers do not have distinct store keys'
    }

    $providerTransports = [Collections.Generic.List[string]]::new()
    foreach ($role in @('alice', 'bob')) {
        $imports = Read-FieldText $Directory "02-$role-providers.log"
        foreach ($key in $providerKeys) {
            Assert-FieldMatch $imports "store_key=$([regex]::Escape($key)) " "$role import of provider $key"
        }
        $transportIds = @(Get-UniqueMatches $imports 'transport_identity=([0-9a-f]{64}) store_key=')
        if ($transportIds.Count -lt 2) { throw "$role did not retain two transport-distinct providers" }
        if ($role -eq 'alice') { foreach ($id in $transportIds) { $providerTransports.Add($id) } }
        Assert-FieldMatch $imports '^status=runtime-volunteer-providers-selected$' "$role provider selection"
    }
    if (@($providerTransports | Sort-Object -Unique).Count -lt 2) {
        throw 'Alice provider evidence is not transport-distinct'
    }

    $queue = Read-FieldText $Directory '03-alice-queue.log'
    Assert-FieldMatch $queue '^status=runtime-message-queued$' 'Alice durable queue insertion'
    $send = Read-FieldText $Directory '03-send-alice.log'
    Assert-FieldMatch $send '^runtime_outbound_status=mailbox-stored$' 'compatibility mailbox acceptance before asynchronous replication'
    Assert-FieldMatch $send '^runtime_mailbox_replication_receipts=2/2$' 'two-of-three volunteer receipt threshold'
    Assert-FieldMatch $send '^runtime_mailbox_replication_status=satisfied$' 'completed volunteer replication plan'
    $senderReceiptKeys = @(Get-UniqueMatches $send '^runtime_mailbox_replication_receipt_store_key=([0-9a-f]{64})$')
    foreach ($key in $providerKeys) {
        if ($key -notin $senderReceiptKeys) { throw "Alice has no signed PUT receipt from provider $key" }
    }

    $offline = Read-FieldText $Directory '03-alice-offline.boundary'
    Assert-FieldMatch $offline '^alice_replication_status=satisfied$' 'replication before sender shutdown'
    Assert-FieldMatch $offline '^alice_runtime_ipc_reachable=false$' 'Alice runtime stopped before Bob receive'

    $receive = Read-FieldText $Directory '04-receive-bob.log'
    $sourceKeys = @(Get-UniqueMatches $receive '^runtime_mailbox_replica_source_store_key=([0-9a-f]{64})$')
    foreach ($key in $providerKeys) {
        if ($key -notin $sourceKeys) { throw "Bob did not retrieve the replica from provider $key" }
    }
    $volunteerCommits = [regex]::Matches($receive, '(?m)^runtime_mailbox_inbound_source=volunteer-iroh$').Count
    $volunteerDeletes = [regex]::Matches($receive, '(?m)^runtime_mailbox_replica_delete_status=deleted-after-commit$').Count
    if ($volunteerCommits -lt 2 -or $volunteerDeletes -lt 2) {
        throw 'Bob did not commit and delete both volunteer replicas'
    }

    $restart = Read-FieldText $Directory '05-restart-bob.log'
    Assert-FieldMatch $restart '^status=runtime-listening$' 'Bob restart from retained state'
    if ([regex]::IsMatch($restart, '(?m)^runtime_mailbox_inbound_source=volunteer-iroh$')) {
        throw 'a deleted volunteer replica was delivered again after Bob restart'
    }
    $history = Read-FieldText $Directory '05-bob-history.log'
    $markerCount = [regex]::Matches($history, [regex]::Escape([string]$manifest.message_marker)).Count
    if ($markerCount -ne 1) {
        throw "Bob history must contain the exact test message once, found $markerCount"
    }

    $boundaries = Read-FieldText $Directory '06-boundaries.log'
    foreach ($line in @(
        'volunteer_storage_boundary=verified',
        'volunteer_iroh_boundary=verified',
        'volunteer_provider_selection_boundary=verified',
        'volunteer_provider_gossip_boundary=verified',
        'volunteer_replication_boundary=verified',
        'volunteer_retrieval_boundary=verified',
        'runtime_mailbox_flow=verified'
    )) {
        Assert-FieldMatch $boundaries "^$([regex]::Escape($line))$" $line
    }

    [PSCustomObject]@{
        run_id = [string]$manifest.run_id
        build_commit = [string]$manifest.build_commit
        provider_store_keys = ($providerKeys -join ',')
        independent_transport_identities = 2
        sender_replication = 'two-of-three-satisfied'
        sender_offline_before_receive = $true
        recipient_retrieval = 'two-volunteer-replicas-committed-and-deleted'
        restart_redelivery = 'absent'
        history_message_occurrences = 1
        operator_independence = 'not-proven-when-providers-share-a-host'
        result = 'verified'
    }
}

function New-VolunteerFieldSelfTest {
    param([Parameter(Mandatory)] [string] $Directory)
    New-Item -ItemType Directory -Path $Directory | Out-Null
    $p1 = '11' * 32
    $p2 = '22' * 32
    $t1 = '31' * 32
    $t2 = '32' * 32
    $message = 'kilogram-m0967-self-test-message'
    @{
        schema = 1
        run_id = '20260908-120000'
        build_commit = 'ab' * 20
        conversation_label = 'm0967-field'
        alice_account_id = 'aa' * 32
        bob_account_id = 'bb' * 32
        message_marker = $message
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'manifest.json') -Encoding utf8
    foreach ($entry in @(@('provider1', $p1), @('provider2', $p2))) {
        Set-Content -LiteralPath (Join-Path $Directory "01-$($entry[0]).log") -Encoding utf8 -Value @(
            'runtime_volunteer_storage=serving',
            "runtime_volunteer_storage_store_key=$($entry[1])",
            'runtime_volunteer_storage_iroh_alpn=kilogram/m0/blind-mailbox/1',
            'status=runtime-listening'
        )
        Set-Content -LiteralPath (Join-Path $Directory "01-$($entry[0]).offer") -Encoding utf8 -Value ('A' * 96)
    }
    $imports = @(
        "provider_offer_id=$('41' * 32) transport_identity=$t1 store_key=$p1 policy_class=bounded",
        "provider_offer_id=$('42' * 32) transport_identity=$t2 store_key=$p2 policy_class=bounded",
        'status=runtime-volunteer-providers-selected'
    )
    Set-Content -LiteralPath (Join-Path $Directory '02-alice-providers.log') -Encoding utf8 -Value $imports
    Set-Content -LiteralPath (Join-Path $Directory '02-bob-providers.log') -Encoding utf8 -Value $imports
    Set-Content -LiteralPath (Join-Path $Directory '03-alice-queue.log') -Encoding utf8 -Value 'status=runtime-message-queued'
    Set-Content -LiteralPath (Join-Path $Directory '03-send-alice.log') -Encoding utf8 -Value @(
        'runtime_outbound_status=mailbox-stored',
        "runtime_mailbox_replication_receipt_store_key=$p1",
        "runtime_mailbox_replication_receipt_store_key=$p2",
        'runtime_mailbox_replication_receipts=2/2',
        'runtime_mailbox_replication_status=satisfied'
    )
    Set-Content -LiteralPath (Join-Path $Directory '03-alice-offline.boundary') -Encoding utf8 -Value @(
        'alice_replication_status=satisfied', 'alice_runtime_ipc_reachable=false'
    )
    Set-Content -LiteralPath (Join-Path $Directory '04-receive-bob.log') -Encoding utf8 -Value @(
        "runtime_mailbox_replica_source_store_key=$p1",
        'runtime_mailbox_replica_delete_status=deleted-after-commit',
        'runtime_mailbox_inbound_source=volunteer-iroh',
        "runtime_mailbox_replica_source_store_key=$p2",
        'runtime_mailbox_replica_delete_status=deleted-after-commit',
        'runtime_mailbox_inbound_source=volunteer-iroh'
    )
    Set-Content -LiteralPath (Join-Path $Directory '05-restart-bob.log') -Encoding utf8 -Value 'status=runtime-listening'
    Set-Content -LiteralPath (Join-Path $Directory '05-bob-history.log') -Encoding utf8 -Value "payload=text body=`"$message`""
    Set-Content -LiteralPath (Join-Path $Directory '06-boundaries.log') -Encoding utf8 -Value @(
        'volunteer_storage_boundary=verified', 'volunteer_iroh_boundary=verified',
        'volunteer_provider_selection_boundary=verified', 'volunteer_provider_gossip_boundary=verified',
        'volunteer_replication_boundary=verified', 'volunteer_retrieval_boundary=verified',
        'runtime_mailbox_flow=verified'
    )
}

if ($SelfTest) {
    $temporary = Join-Path ([IO.Path]::GetTempPath()) ("kilogram-volunteer-field-self-test-" + [Guid]::NewGuid().ToString('N'))
    try {
        New-VolunteerFieldSelfTest $temporary
        $result = Test-VolunteerFieldEvidence $temporary
        if ($result.result -ne 'verified') { throw 'volunteer field evidence self-test did not verify' }
        Write-Output 'volunteer_field_evidence_self_test=verified'
    }
    finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Recurse -Force }
    }
    exit 0
}

if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    throw 'EvidenceDirectory is required unless SelfTest is used'
}
Test-VolunteerFieldEvidence ([IO.Path]::GetFullPath($EvidenceDirectory)) | Format-List
