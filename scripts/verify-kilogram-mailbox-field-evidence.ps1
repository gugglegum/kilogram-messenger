[CmdletBinding()]
param(
    [string] $EvidenceDirectory,
    [switch] $SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-EvidenceText {
    param(
        [Parameter(Mandatory)] [string] $Directory,
        [Parameter(Mandatory)] [string] $Name
    )

    $path = Join-Path $Directory $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required field evidence is missing: $Name"
    }
    return (Get-Content -LiteralPath $path -Raw).Replace("`r`n", "`n")
}

function Assert-TextMatch {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Description
    )

    if (-not [regex]::IsMatch($Text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline)) {
        throw "field evidence does not prove $Description"
    }
}

function Get-CapabilityHead {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $ConversationId,
        [Parameter(Mandatory)] [string] $PeerAccountId,
        [Parameter(Mandatory)] [string] $PeerDeviceId,
        [Parameter(Mandatory)] [ValidateSet('receive', 'write')] [string] $Direction,
        [Parameter(Mandatory)] [UInt64] $Generation,
        [Parameter(Mandatory)] [string] $Acknowledged,
        [Parameter(Mandatory)] [bool] $Revoked,
        [Parameter(Mandatory)] [string] $State
    )

    $heads = @()
    foreach ($line in ($Text -split "`r?`n")) {
        if (-not $line.StartsWith('mailbox_capability_contact_id=', [StringComparison]::Ordinal)) {
            continue
        }
        $fields = @{}
        foreach ($field in ($line -split ' ')) {
            $parts = $field -split '=', 2
            if ($parts.Count -eq 2) {
                $fields[$parts[0]] = $parts[1]
            }
        }
        if ($fields['conversation_id'] -eq $ConversationId -and
            $fields['peer_account_id'] -eq $PeerAccountId -and
            $fields['peer_device_id'] -eq $PeerDeviceId -and
            $fields['direction'] -eq $Direction -and
            $fields['generation'] -eq $Generation.ToString() -and
            $fields['acknowledged'] -eq $Acknowledged -and
            $fields['revoked'] -eq $Revoked.ToString().ToLowerInvariant() -and
            $fields['state'] -eq $State) {
            $heads += ,$fields
        }
    }
    if ($heads.Count -ne 1) {
        throw "expected exactly one $Direction generation $Generation capability head in state '$State', found $($heads.Count)"
    }
    foreach ($field in @('binding_id', 'update_id')) {
        if ($heads[0][$field] -notmatch '^[0-9a-f]{64}$') {
            throw "capability head has an invalid $field"
        }
    }
    return $heads[0]
}

function Get-UnsignedCounter {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Name
    )

    $match = [regex]::Match(
        $Text,
        "(?m)^$([regex]::Escape($Name))=([0-9]+)$"
    )
    if (-not $match.Success) {
        throw "field evidence counter is missing: $Name"
    }
    return [UInt64]::Parse($match.Groups[1].Value)
}

function Assert-RouteEvidence {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [ValidateSet('direct', 'relay')] [string] $Route,
        [Parameter(Mandatory)] [string] $Phase
    )

    Assert-TextMatch $Text "^transport_ready_path=$([regex]::Escape($Route))$" "$Phase ready route $Route"
    Assert-TextMatch $Text "^transport_path=$([regex]::Escape($Route))$" "$Phase completed route $Route"
}

function Test-FieldEvidence {
    param([Parameter(Mandatory)] [string] $Directory)

    $manifestPath = Join-Path $Directory 'manifest.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw 'required field evidence is missing: manifest.json'
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.schema -ne 1) {
        throw 'field evidence manifest schema must be exactly 1'
    }
    foreach ($name in @(
        'run_id',
        'build_commit',
        'conversation_id',
        'alice_account_id',
        'alice_device_id',
        'bob_account_id',
        'bob_device_id',
        'mailbox_service_url',
        'mailbox_store_key',
        'activation_route',
        'rotation_route',
        'revocation_route'
    )) {
        if (-not ($manifest.PSObject.Properties.Name -contains $name) -or
            [string]::IsNullOrWhiteSpace([string]$manifest.$name)) {
            throw "field evidence manifest value is missing: $name"
        }
    }
    if ([string]$manifest.run_id -notmatch '^[0-9]{8}-[0-9]{6}$') {
        throw 'field evidence run_id must use yyyyMMdd-HHmmss'
    }
    if ([string]$manifest.build_commit -notmatch '^[0-9a-f]{7,40}$') {
        throw 'field evidence build_commit must be a lowercase Git commit ID'
    }
    foreach ($name in @(
        'conversation_id',
        'alice_account_id',
        'alice_device_id',
        'bob_account_id',
        'bob_device_id',
        'mailbox_store_key'
    )) {
        if ([string]$manifest.$name -notmatch '^[0-9a-f]{64}$') {
            throw "field evidence manifest value must be a lowercase 32-byte hex ID: $name"
        }
    }
    $serviceUri = [Uri]$manifest.mailbox_service_url
    if (-not $serviceUri.IsAbsoluteUri -or $serviceUri.Scheme -ne 'https' -or
        -not [string]::IsNullOrEmpty($serviceUri.Query) -or
        -not [string]::IsNullOrEmpty($serviceUri.Fragment) -or
        $serviceUri.UserInfo.Length -ne 0) {
        throw 'mailbox_service_url must be an absolute public HTTPS URL without credentials, query or fragment'
    }
    $routes = @(
        [string]$manifest.activation_route,
        [string]$manifest.rotation_route,
        [string]$manifest.revocation_route
    )
    if ($routes | Where-Object { $_ -notin @('direct', 'relay') }) {
        throw 'field lifecycle routes must be direct or relay'
    }
    if ('direct' -notin $routes -or 'relay' -notin $routes) {
        throw 'field lifecycle evidence must include both a direct and a relay transition'
    }

    $lostAck = Read-EvidenceText $Directory '01-lost-ack-bob.log'
    foreach ($proof in @(
        @{ Pattern = '^runtime_test_fault_armed=mailbox-capability-ack-drop-after-durable-apply-once$'; Description = 'the explicit one-shot debug fault was armed' },
        @{ Pattern = '^mailbox_capability_generation=1$'; Description = 'generation-one activation reached Bob' },
        @{ Pattern = '^mailbox_capability_update_store=Inserted$'; Description = 'Bob durably inserted the activation before dropping ACK' },
        @{ Pattern = '^runtime_test_fault=mailbox-capability-ack-dropped-after-durable-apply$'; Description = 'ACK was deliberately dropped after durable apply' },
        @{ Pattern = '^runtime_test_fault_ack_written=false$'; Description = 'the first recipient ACK was not written' },
        @{ Pattern = '^runtime_test_fault_status=triggered-after-durable-apply-and-vault-mirror$'; Description = 'the applied update crossed the vault mirror boundary' },
        @{ Pattern = '^runtime_stop_reason=debug-mailbox-capability-ack-drop$'; Description = 'Bob stopped at the controlled restart boundary' }
    )) {
        Assert-TextMatch $lostAck $proof.Pattern $proof.Description
    }
    Assert-TextMatch $lostAck "^transport_ready_path=$([regex]::Escape([string]$manifest.activation_route))$" 'activation route before the lost ACK'

    $alicePending = Read-EvidenceText $Directory '01-alice-pending.status'
    $alicePendingHead = Get-CapabilityHead $alicePending $manifest.conversation_id $manifest.bob_account_id $manifest.bob_device_id 'receive' 1 'false' $false 'activation-pending'
    if ((Get-UnsignedCounter $alicePending 'mailbox_local_unacknowledged_update_count') -lt 1) {
        throw 'Alice did not retain the unacknowledged activation after Bob stopped'
    }

    $aliceRetry = Read-EvidenceText $Directory '02-retry-alice.log'
    Assert-RouteEvidence $aliceRetry $manifest.activation_route 'activation retry'
    Assert-TextMatch $aliceRetry '^mailbox_capability_generation=1$' 'generation-one retry'
    Assert-TextMatch $aliceRetry '^runtime_mailbox_capability_update_status=acknowledged$' 'recipient-signed activation ACK'
    $bobRetry = Read-EvidenceText $Directory '02-retry-bob.log'
    Assert-TextMatch $bobRetry '^mailbox_capability_update_store=AlreadyPresent$' 'idempotent activation replay after Bob restart'
    Assert-TextMatch $bobRetry '^status=runtime-mailbox-capability-updated$' 'Bob activation retry completion'

    $aliceActive = Read-EvidenceText $Directory '02-alice-active.status'
    $bobActive = Read-EvidenceText $Directory '02-bob-active.status'
    $aliceActiveHead = Get-CapabilityHead $aliceActive $manifest.conversation_id $manifest.bob_account_id $manifest.bob_device_id 'receive' 1 'true' $false 'active'
    $bobActiveHead = Get-CapabilityHead $bobActive $manifest.conversation_id $manifest.alice_account_id $manifest.alice_device_id 'write' 1 'not-applicable' $false 'active'
    if ($alicePendingHead.update_id -ne $aliceActiveHead.update_id -or
        $aliceActiveHead.update_id -ne $bobActiveHead.update_id -or
        $aliceActiveHead.binding_id -ne $bobActiveHead.binding_id) {
        throw 'activation did not converge on the exact same signed update and binding'
    }

    $aliceRotationPending = Read-EvidenceText $Directory '03-alice-rotation-pending.status'
    $rotationPendingHead = Get-CapabilityHead $aliceRotationPending $manifest.conversation_id $manifest.bob_account_id $manifest.bob_device_id 'receive' 2 'false' $false 'rotation-pending'
    if ($rotationPendingHead.binding_id -eq $aliceActiveHead.binding_id) {
        throw 'rotation pending state did not install a different binding'
    }
    $aliceRotation = Read-EvidenceText $Directory '04-rotation-alice.log'
    Assert-RouteEvidence $aliceRotation $manifest.rotation_route 'rotation'
    Assert-TextMatch $aliceRotation '^mailbox_capability_generation=2$' 'generation-two rotation transfer'
    Assert-TextMatch $aliceRotation '^runtime_mailbox_capability_update_status=acknowledged$' 'recipient-signed rotation ACK'
    $aliceRotated = Read-EvidenceText $Directory '04-alice-rotated.status'
    $bobRotated = Read-EvidenceText $Directory '04-bob-rotated.status'
    $aliceRotatedHead = Get-CapabilityHead $aliceRotated $manifest.conversation_id $manifest.bob_account_id $manifest.bob_device_id 'receive' 2 'true' $false 'active'
    $bobRotatedHead = Get-CapabilityHead $bobRotated $manifest.conversation_id $manifest.alice_account_id $manifest.alice_device_id 'write' 2 'not-applicable' $false 'active'
    if ($rotationPendingHead.update_id -ne $aliceRotatedHead.update_id -or
        $aliceRotatedHead.update_id -ne $bobRotatedHead.update_id -or
        $aliceRotatedHead.binding_id -ne $bobRotatedHead.binding_id) {
        throw 'rotation did not converge on the exact same signed update and binding'
    }

    $storeLog = Read-EvidenceText $Directory '05-store.log'
    Assert-TextMatch $storeLog '^storage_format=opaque-redb-v1$' 'opaque mailbox storage format'
    Assert-TextMatch $storeLog '^blind_mailbox_transport=reverse-proxy-https-required$' 'HTTPS reverse-proxy boundary'
    Assert-TextMatch $storeLog "^blind_mailbox_store_key=$([regex]::Escape([string]$manifest.mailbox_store_key))$" 'the pinned mailbox store key'
    foreach ($forbidden in @('account_id=', 'device_id=', 'conversation_id=', 'event_id=', 'message=')) {
        if ($storeLog.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "mailbox service evidence exposes forbidden application metadata: $forbidden"
        }
    }
    $bobMailboxSend = Read-EvidenceText $Directory '05-mailbox-send-bob.log'
    Assert-TextMatch $bobMailboxSend '^runtime_outbound_status=mailbox-stored$' 'store-signed mailbox fallback acceptance'
    $aliceMailboxReceive = Read-EvidenceText $Directory '05-mailbox-receive-alice.log'
    Assert-TextMatch $aliceMailboxReceive '^runtime_mailbox_inbound_status=deleted-after-commit$' 'application commit before mailbox deletion'
    $aliceMailboxStatus = Read-EvidenceText $Directory '05-alice-mailbox.status'
    if ((Get-UnsignedCounter $aliceMailboxStatus 'mailbox_received_commit_count') -lt 1 -or
        (Get-UnsignedCounter $aliceMailboxStatus 'mailbox_deleted_inbound_count') -lt 1) {
        throw 'Alice mailbox ledger does not retain receive-and-delete completion evidence'
    }

    $aliceRevocationPending = Read-EvidenceText $Directory '06-alice-revocation-pending.status'
    $revocationPendingHead = Get-CapabilityHead $aliceRevocationPending $manifest.conversation_id $manifest.bob_account_id $manifest.bob_device_id 'receive' 3 'false' $true 'revocation-pending'
    if ($revocationPendingHead.binding_id -ne $aliceRotatedHead.binding_id) {
        throw 'revocation does not target the current rotated binding'
    }
    $aliceRevocation = Read-EvidenceText $Directory '07-revocation-alice.log'
    Assert-RouteEvidence $aliceRevocation $manifest.revocation_route 'revocation'
    Assert-TextMatch $aliceRevocation '^mailbox_capability_generation=3$' 'generation-three revocation transfer'
    Assert-TextMatch $aliceRevocation '^runtime_mailbox_capability_update_status=acknowledged$' 'recipient-signed revocation ACK'
    $aliceRevoked = Read-EvidenceText $Directory '07-alice-revoked.status'
    $bobRevoked = Read-EvidenceText $Directory '07-bob-revoked.status'
    $aliceRevokedHead = Get-CapabilityHead $aliceRevoked $manifest.conversation_id $manifest.bob_account_id $manifest.bob_device_id 'receive' 3 'true' $true 'revoked'
    $bobRevokedHead = Get-CapabilityHead $bobRevoked $manifest.conversation_id $manifest.alice_account_id $manifest.alice_device_id 'write' 3 'not-applicable' $true 'revoked'
    if ($revocationPendingHead.update_id -ne $aliceRevokedHead.update_id -or
        $aliceRevokedHead.update_id -ne $bobRevokedHead.update_id) {
        throw 'revocation did not converge on the exact same signed update'
    }

    $boundaries = Read-EvidenceText $Directory '08-boundaries.log'
    foreach ($required in @(
        'blind_mailbox_boundary=verified',
        'runtime_mailbox_flow=verified',
        'mailbox_capability_lifecycle=verified',
        'mailbox_capability_convergence=verified',
        'mailbox_desktop_control=verified',
        'mailbox_field_test_boundary=verified'
    )) {
        Assert-TextMatch $boundaries "^$([regex]::Escape($required))$" $required
    }

    [PSCustomObject]@{
        run_id = [string]$manifest.run_id
        build_commit = [string]$manifest.build_commit
        activation_update_id = $aliceActiveHead.update_id
        rotation_update_id = $aliceRotatedHead.update_id
        revocation_update_id = $aliceRevokedHead.update_id
        lifecycle_routes = ($routes -join ',')
        mailbox_round_trip = 'stored-received-committed-deleted'
        result = 'verified'
    }
}

function New-SelfTestEvidence {
    param([Parameter(Mandatory)] [string] $Directory)

    New-Item -ItemType Directory -Path $Directory | Out-Null
    $aliceAccount = 'aa' * 32
    $aliceDevice = '11' * 32
    $bobAccount = 'bb' * 32
    $bobDevice = '22' * 32
    $conversation = '33' * 32
    $bindingOne = '41' * 32
    $bindingTwo = '42' * 32
    $updateOne = '51' * 32
    $updateTwo = '52' * 32
    $updateThree = '53' * 32
    $storeKey = '66' * 32
    @{
        schema = 1
        run_id = '20260906-120000'
        build_commit = '1234567'
        conversation_id = $conversation
        alice_account_id = $aliceAccount
        alice_device_id = $aliceDevice
        bob_account_id = $bobAccount
        bob_device_id = $bobDevice
        mailbox_service_url = 'https://mailbox.example.test'
        mailbox_store_key = $storeKey
        activation_route = 'direct'
        rotation_route = 'relay'
        revocation_route = 'relay'
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'manifest.json') -Encoding utf8

    function Head([string] $peerAccount, [string] $peerDevice, [string] $direction, [string] $binding, [string] $update, [int] $generation, [string] $acknowledged, [bool] $revoked, [string] $state) {
        "mailbox_capability_contact_id=$('44' * 32) conversation_id=$conversation peer_account_id=$peerAccount peer_device_id=$peerDevice direction=$direction binding_id=$binding update_id=$update generation=$generation acknowledged=$acknowledged revoked=$($revoked.ToString().ToLowerInvariant()) state=$state"
    }
    function Put([string] $name, [string[]] $lines) {
        Set-Content -LiteralPath (Join-Path $Directory $name) -Value $lines -Encoding utf8
    }

    Put '01-lost-ack-bob.log' @(
        'runtime_test_fault_armed=mailbox-capability-ack-drop-after-durable-apply-once',
        'transport_ready_path=direct',
        'mailbox_capability_generation=1',
        'mailbox_capability_update_store=Inserted',
        'runtime_test_fault=mailbox-capability-ack-dropped-after-durable-apply',
        'runtime_test_fault_ack_written=false',
        'runtime_test_fault_status=triggered-after-durable-apply-and-vault-mirror',
        'runtime_stop_reason=debug-mailbox-capability-ack-drop'
    )
    Put '01-alice-pending.status' @('mailbox_local_unacknowledged_update_count=1', (Head $bobAccount $bobDevice 'receive' $bindingOne $updateOne 1 'false' $false 'activation-pending'))
    Put '02-retry-alice.log' @('transport_ready_path=direct', 'mailbox_capability_generation=1', 'runtime_mailbox_capability_update_status=acknowledged', 'transport_path=direct')
    Put '02-retry-bob.log' @('mailbox_capability_update_store=AlreadyPresent', 'status=runtime-mailbox-capability-updated')
    Put '02-alice-active.status' @(Head $bobAccount $bobDevice 'receive' $bindingOne $updateOne 1 'true' $false 'active')
    Put '02-bob-active.status' @(Head $aliceAccount $aliceDevice 'write' $bindingOne $updateOne 1 'not-applicable' $false 'active')
    Put '03-alice-rotation-pending.status' @(Head $bobAccount $bobDevice 'receive' $bindingTwo $updateTwo 2 'false' $false 'rotation-pending')
    Put '04-rotation-alice.log' @('transport_ready_path=relay', 'mailbox_capability_generation=2', 'runtime_mailbox_capability_update_status=acknowledged', 'transport_path=relay')
    Put '04-alice-rotated.status' @(Head $bobAccount $bobDevice 'receive' $bindingTwo $updateTwo 2 'true' $false 'active')
    Put '04-bob-rotated.status' @(Head $aliceAccount $aliceDevice 'write' $bindingTwo $updateTwo 2 'not-applicable' $false 'active')
    Put '05-store.log' @('storage_format=opaque-redb-v1', "blind_mailbox_store_key=$storeKey", 'blind_mailbox_transport=reverse-proxy-https-required')
    Put '05-mailbox-send-bob.log' @('runtime_outbound_status=mailbox-stored')
    Put '05-mailbox-receive-alice.log' @('runtime_mailbox_inbound_status=deleted-after-commit')
    Put '05-alice-mailbox.status' @('mailbox_received_commit_count=1', 'mailbox_deleted_inbound_count=1')
    Put '06-alice-revocation-pending.status' @(Head $bobAccount $bobDevice 'receive' $bindingTwo $updateThree 3 'false' $true 'revocation-pending')
    Put '07-revocation-alice.log' @('transport_ready_path=relay', 'mailbox_capability_generation=3', 'runtime_mailbox_capability_update_status=acknowledged', 'transport_path=relay')
    Put '07-alice-revoked.status' @(Head $bobAccount $bobDevice 'receive' $bindingTwo $updateThree 3 'true' $true 'revoked')
    Put '07-bob-revoked.status' @(Head $aliceAccount $aliceDevice 'write' $bindingTwo $updateThree 3 'not-applicable' $true 'revoked')
    Put '08-boundaries.log' @(
        'blind_mailbox_boundary=verified',
        'runtime_mailbox_flow=verified',
        'mailbox_capability_lifecycle=verified',
        'mailbox_capability_convergence=verified',
        'mailbox_desktop_control=verified',
        'mailbox_field_test_boundary=verified'
    )
}

if ($SelfTest) {
    $root = Join-Path ([IO.Path]::GetTempPath()) ("kilogram-mailbox-field-verifier-" + [Guid]::NewGuid().ToString('N'))
    try {
        New-SelfTestEvidence $root
        $result = Test-FieldEvidence $root
        if ($result.result -ne 'verified') {
            throw 'valid field evidence self-test was not accepted'
        }
        Add-Content -LiteralPath (Join-Path $root '05-store.log') -Value 'conversation_id=forbidden'
        $rejected = $false
        try {
            $null = Test-FieldEvidence $root
        }
        catch {
            $rejected = $true
        }
        if (-not $rejected) {
            throw 'tampered field evidence self-test was accepted'
        }
        Write-Output 'mailbox_field_evidence_verifier_self_test=passed'
        Write-Output 'valid_evidence=accepted'
        Write-Output 'metadata_leak_evidence=rejected'
    }
    finally {
        if (Test-Path -LiteralPath $root) {
            Remove-Item -LiteralPath $root -Recurse -Force
        }
    }
    exit 0
}

if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    throw '-EvidenceDirectory is required unless -SelfTest is used'
}
$resolved = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$verified = Test-FieldEvidence $resolved
$verified | Format-List
