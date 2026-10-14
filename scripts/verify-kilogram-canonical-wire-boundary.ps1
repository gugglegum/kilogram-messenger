[CmdletBinding()]
param(
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-WIRE-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-WIRE-VECTORS.json'

function Assert-ExactStringSet {
    param(
        [AllowEmptyCollection()][string[]]$Actual,
        [AllowEmptyCollection()][string[]]$Expected,
        [string]$Label
    )

    $actualSorted = @($Actual | Sort-Object)
    $expectedSorted = @($Expected | Sort-Object)
    if ($actualSorted.Count -ne $expectedSorted.Count) {
        throw "$Label count mismatch: expected $($expectedSorted.Count), got $($actualSorted.Count)"
    }
    for ($index = 0; $index -lt $expectedSorted.Count; $index++) {
        if ($actualSorted[$index] -cne $expectedSorted[$index]) {
            throw "$Label mismatch at index ${index}: expected '$($expectedSorted[$index])', got '$($actualSorted[$index])'"
        }
    }
}

function Assert-ExactPropertySet {
    param(
        [object]$Value,
        [string[]]$Expected,
        [string]$Label
    )

    if ($null -eq $Value) {
        throw "$Label is null"
    }
    $actual = @($Value.PSObject.Properties | ForEach-Object { $_.Name })
    Assert-ExactStringSet $actual $Expected "$Label properties"
}

function Assert-Rejected {
    param(
        [scriptblock]$Action,
        [string]$Label
    )

    try {
        & $Action
    }
    catch {
        Write-Output "$Label=rejected"
        return
    }
    throw "$Label was unexpectedly accepted"
}

function ConvertFrom-CanonicalHex {
    param(
        [string]$Value,
        [int]$ExpectedBytes = -1,
        [string]$Label = 'hex value'
    )

    if ([string]::IsNullOrEmpty($Value) -or
        $Value.Length % 2 -ne 0 -or
        $Value -cnotmatch '^[0-9a-f]+$') {
        throw "$Label is not non-empty lowercase hexadecimal"
    }
    $bytes = [byte[]]::new($Value.Length / 2)
    for ($index = 0; $index -lt $bytes.Length; $index++) {
        $bytes[$index] = [Convert]::ToByte($Value.Substring($index * 2, 2), 16)
    }
    if ($ExpectedBytes -ge 0 -and $bytes.Length -ne $ExpectedBytes) {
        throw "$Label has $($bytes.Length) bytes; expected $ExpectedBytes"
    }
    return ,$bytes
}

function ConvertTo-CanonicalHex {
    param([byte[]]$Value)

    return ([BitConverter]::ToString($Value) -replace '-', '').ToLowerInvariant()
}

function Add-CborHead {
    param(
        [System.Collections.Generic.List[byte]]$Buffer,
        [byte]$MajorType,
        [UInt64]$Argument
    )

    $major = [byte]($MajorType -shl 5)
    if ($Argument -le 23) {
        $Buffer.Add([byte]($major -bor [byte]$Argument))
        return
    }
    if ($Argument -le [byte]::MaxValue) {
        $Buffer.Add([byte]($major -bor 24))
        $Buffer.Add([byte]$Argument)
        return
    }
    if ($Argument -le [UInt16]::MaxValue) {
        $Buffer.Add([byte]($major -bor 25))
        $encoded = [BitConverter]::GetBytes([UInt16]$Argument)
    }
    elseif ($Argument -le [UInt32]::MaxValue) {
        $Buffer.Add([byte]($major -bor 26))
        $encoded = [BitConverter]::GetBytes([UInt32]$Argument)
    }
    else {
        $Buffer.Add([byte]($major -bor 27))
        $encoded = [BitConverter]::GetBytes([UInt64]$Argument)
    }
    if ([BitConverter]::IsLittleEndian) {
        [Array]::Reverse($encoded)
    }
    $Buffer.AddRange([byte[]]$encoded)
}

function Add-CborUnsigned {
    param(
        [System.Collections.Generic.List[byte]]$Buffer,
        [UInt64]$Value
    )
    Add-CborHead $Buffer 0 $Value
}

function Add-CborByteString {
    param(
        [System.Collections.Generic.List[byte]]$Buffer,
        [byte[]]$Value
    )
    Add-CborHead $Buffer 2 ([UInt64]$Value.Length)
    $Buffer.AddRange($Value)
}

function Add-CborArrayHeader {
    param(
        [System.Collections.Generic.List[byte]]$Buffer,
        [UInt64]$Length
    )
    Add-CborHead $Buffer 4 $Length
}

function Assert-CanonicalOrder {
    param(
        [object[]]$Values,
        [string]$Label
    )

    for ($index = 1; $index -lt $Values.Count; $index++) {
        if ([StringComparer]::Ordinal.Compare([string]$Values[$index - 1], [string]$Values[$index]) -ge 0) {
            throw "$Label is not in strict unsigned bytewise lexicographic order"
        }
    }
}

function Assert-RustBindings {
    param(
        [object[]]$Bindings,
        [string]$SourceRoot
    )

    $expectedSymbols = @(
        'ACKNOWLEDGEMENT_PAYLOAD_VARIANT',
        'CANONICAL_EVENT_ID_DOMAIN',
        'CANONICAL_EVENT_SIGNATURE_DOMAIN',
        'CANONICAL_PROFILE_VERSION',
        'CANONICAL_SYNC_INVENTORY_SIGNATURE_DOMAIN'
    )
    Assert-ExactStringSet @($Bindings | ForEach-Object { [string]$_.symbol }) $expectedSymbols 'canonical Rust bindings'
    $orderedSymbols = @($Bindings | ForEach-Object { [string]$_.symbol })
    for ($index = 0; $index -lt $expectedSymbols.Count; $index++) {
        if ($orderedSymbols[$index] -cne $expectedSymbols[$index]) {
            throw 'canonical Rust bindings must be stored in stable symbol order'
        }
    }

    foreach ($binding in $Bindings) {
        Assert-ExactPropertySet $binding @('path', 'symbol', 'rust_type', 'value') 'canonical Rust binding'
        if ([string]$binding.path -cne 'crates/kilogram-protocol/src/canonical_v1.rs') {
            throw "unexpected canonical Rust binding path '$($binding.path)'"
        }
        $sourcePath = Join-Path $SourceRoot ([string]$binding.path -replace '/', '\')
        $source = Get-Content -LiteralPath $sourcePath -Raw
        $symbol = [regex]::Escape([string]$binding.symbol)
        switch ([string]$binding.rust_type) {
            'u64' {
                $value = [UInt64]$binding.value
                $pattern = "(?m)^\s*pub\s+const\s+$symbol\s*:\s*u64\s*=\s*$value\s*;"
            }
            'bytes' {
                $value = [regex]::Escape([string]$binding.value)
                $pattern = "(?ms)^\s*pub\s+const\s+$symbol\s*:\s*&\[u8\]\s*=\s*b`"$value`"\s*;"
            }
            default {
                throw "unsupported canonical Rust binding type '$($binding.rust_type)'"
            }
        }
        if ([regex]::Matches($source, $pattern).Count -ne 1) {
            throw "canonical Rust binding '$($binding.symbol)' does not have one exact source declaration"
        }
    }
}

function Assert-CanonicalSchema {
    param(
        [object]$Schema,
        [string]$SourceRoot
    )

    Assert-ExactPropertySet $Schema @(
        'schema_version', 'milestone', 'status', 'changes_active_wire', 'encoding',
        'activation', 'rust_bindings', 'objects'
    ) 'canonical schema'
    if ([UInt64]$Schema.schema_version -ne 1 -or
        [string]$Schema.milestone -cne 'M1.1.1' -or
        [string]$Schema.status -cne 'candidate-not-active' -or
        [bool]$Schema.changes_active_wire) {
        throw 'canonical schema header changed or claims active wire use'
    }

    Assert-ExactPropertySet $Schema.encoding @(
        'standard', 'standard_url', 'profile', 'allowed_major_types',
        'preferred_serialization_required', 'definite_lengths_only', 'maps_allowed',
        'text_strings_allowed', 'negative_integers_allowed', 'floating_point_allowed',
        'tags_allowed', 'simple_values_allowed'
    ) 'canonical encoding'
    if ([string]$Schema.encoding.standard -cne 'RFC 8949' -or
        [string]$Schema.encoding.standard_url -cne 'https://www.rfc-editor.org/rfc/rfc8949.html' -or
        [string]$Schema.encoding.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical encoding identity changed'
    }
    if (@($Schema.encoding.allowed_major_types).Count -ne 3 -or
        [int]$Schema.encoding.allowed_major_types[0] -ne 0 -or
        [int]$Schema.encoding.allowed_major_types[1] -ne 2 -or
        [int]$Schema.encoding.allowed_major_types[2] -ne 4) {
        throw 'canonical encoding must allow only CBOR major types 0, 2 and 4'
    }
    if (-not [bool]$Schema.encoding.preferred_serialization_required -or
        -not [bool]$Schema.encoding.definite_lengths_only -or
        [bool]$Schema.encoding.maps_allowed -or
        [bool]$Schema.encoding.text_strings_allowed -or
        [bool]$Schema.encoding.negative_integers_allowed -or
        [bool]$Schema.encoding.floating_point_allowed -or
        [bool]$Schema.encoding.tags_allowed -or
        [bool]$Schema.encoding.simple_values_allowed) {
        throw 'canonical deterministic CBOR restrictions were weakened'
    }

    Assert-ExactPropertySet $Schema.activation @(
        'current_runtime_encoding', 'current_runtime_changed', 'required_before_activation'
    ) 'canonical activation policy'
    if ([string]$Schema.activation.current_runtime_encoding -cne 'postcard' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical candidate must not claim current runtime activation'
    }
    Assert-ExactStringSet @($Schema.activation.required_before_activation) @(
        'new-explicit-protocol-version-or-parallel-identifier',
        'dual-reader-and-downgrade-tests',
        'complete-object-schema-coverage',
        'independent-non-rust-vector-verification'
    ) 'canonical activation requirements'

    Assert-RustBindings @($Schema.rust_bindings) $SourceRoot

    $expectedObjects = [ordered]@{
        'acknowledgement-event-content-v1' = [pscustomobject][ordered]@{
            kind = 'signed-content'
            array_length = 6
            fields = @(
                [pscustomobject][ordered]@{ index = 0; name = 'schema_version'; type = 'unsigned-integer'; exact = 1 },
                [pscustomobject][ordered]@{ index = 1; name = 'conversation_id'; type = 'byte-string'; exact_bytes = 32 },
                [pscustomobject][ordered]@{ index = 2; name = 'author_device_id'; type = 'byte-string'; exact_bytes = 32 },
                [pscustomobject][ordered]@{ index = 3; name = 'author_sequence'; type = 'unsigned-integer' },
                [pscustomobject][ordered]@{
                    index = 4
                    name = 'parents'
                    type = 'array'
                    item_type = 'byte-string-32'
                    maximum_items = 64
                    ordering = 'unsigned-bytewise-lexicographic-strict'
                },
                [pscustomobject][ordered]@{
                    index = 5
                    name = 'payload'
                    type = 'object'
                    object = 'acknowledgement-payload-v1'
                }
            )
        }
        'acknowledgement-payload-v1' = [pscustomobject][ordered]@{
            kind = 'event-payload'
            array_length = 2
            fields = @(
                [pscustomobject][ordered]@{ index = 0; name = 'variant'; type = 'unsigned-integer'; exact = 1 },
                [pscustomobject][ordered]@{ index = 1; name = 'acknowledged_event_id'; type = 'byte-string'; exact_bytes = 32 }
            )
        }
        'signed-acknowledgement-event-v1' = [pscustomobject][ordered]@{
            kind = 'signed-envelope'
            array_length = 3
            fields = @(
                [pscustomobject][ordered]@{ index = 0; name = 'schema_version'; type = 'unsigned-integer'; exact = 1 },
                [pscustomobject][ordered]@{
                    index = 1
                    name = 'content'
                    type = 'object'
                    object = 'acknowledgement-event-content-v1'
                },
                [pscustomobject][ordered]@{ index = 2; name = 'signature'; type = 'byte-string'; exact_bytes = 64 }
            )
        }
        'signed-sync-inventory-v1' = [pscustomobject][ordered]@{
            kind = 'signed-envelope'
            array_length = 3
            fields = @(
                [pscustomobject][ordered]@{ index = 0; name = 'schema_version'; type = 'unsigned-integer'; exact = 1 },
                [pscustomobject][ordered]@{
                    index = 1
                    name = 'content'
                    type = 'object'
                    object = 'sync-inventory-content-v1'
                },
                [pscustomobject][ordered]@{ index = 2; name = 'signature'; type = 'byte-string'; exact_bytes = 64 }
            )
        }
        'sync-inventory-content-v1' = [pscustomobject][ordered]@{
            kind = 'signed-content'
            array_length = 5
            fields = @(
                [pscustomobject][ordered]@{ index = 0; name = 'schema_version'; type = 'unsigned-integer'; exact = 1 },
                [pscustomobject][ordered]@{ index = 1; name = 'conversation_id'; type = 'byte-string'; exact_bytes = 32 },
                [pscustomobject][ordered]@{ index = 2; name = 'requester_device_id'; type = 'byte-string'; exact_bytes = 32 },
                [pscustomobject][ordered]@{ index = 3; name = 'session_binding'; type = 'byte-string'; exact_bytes = 32 },
                [pscustomobject][ordered]@{
                    index = 4
                    name = 'event_ids'
                    type = 'array'
                    item_type = 'byte-string-32'
                    maximum_items = 4096
                    ordering = 'unsigned-bytewise-lexicographic-strict'
                }
            )
        }
    }
    $objects = @($Schema.objects)
    Assert-ExactStringSet @($objects | ForEach-Object { [string]$_.id }) @($expectedObjects.Keys) 'canonical object inventory'
    for ($objectIndex = 0; $objectIndex -lt $objects.Count; $objectIndex++) {
        $object = $objects[$objectIndex]
        $expectedId = [string]@($expectedObjects.Keys)[$objectIndex]
        if ([string]$object.id -cne $expectedId) {
            throw 'canonical objects must be stored in stable ID order'
        }
        Assert-ExactPropertySet $object @('id', 'kind', 'array_length', 'fields') "canonical object '$expectedId'"
        $expectedObject = $expectedObjects[$expectedId]
        $fields = @($object.fields)
        $expectedFields = @($expectedObject.fields)
        if ([string]$object.kind -cne [string]$expectedObject.kind -or
            [int]$object.array_length -ne [int]$expectedObject.array_length -or
            $fields.Count -ne $expectedFields.Count) {
            throw "canonical object '$expectedId' shape changed"
        }
        for ($fieldIndex = 0; $fieldIndex -lt $fields.Count; $fieldIndex++) {
            $actualFieldJson = ConvertTo-Json $fields[$fieldIndex] -Compress -Depth 10
            $expectedFieldJson = ConvertTo-Json $expectedFields[$fieldIndex] -Compress -Depth 10
            if ($actualFieldJson -cne $expectedFieldJson) {
                throw "canonical object '$expectedId' positional schema changed at index $fieldIndex"
            }
        }
    }
}

function Assert-CanonicalVectors {
    param([object]$Vectors)

    Assert-ExactPropertySet $Vectors @(
        'schema_version', 'profile', 'activation_status', 'acknowledgement_event', 'sync_inventory'
    ) 'canonical vectors'
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical vector header changed or claims activation'
    }

    $event = $Vectors.acknowledgement_event
    Assert-ExactPropertySet $event @('input', 'expected') 'acknowledgement vector'
    Assert-ExactPropertySet $event.input @(
        'author_secret_key_hex', 'conversation_id_hex', 'author_sequence', 'parents_hex',
        'acknowledged_event_id_hex'
    ) 'acknowledgement vector input'
    Assert-ExactPropertySet $event.expected @(
        'author_device_id_hex', 'content_cbor_hex', 'signature_domain_hex',
        'signing_preimage_hex', 'signature_hex', 'signed_event_cbor_hex',
        'event_id_domain_hex', 'event_id_hex'
    ) 'acknowledgement vector expected'

    ConvertFrom-CanonicalHex $event.input.author_secret_key_hex 32 'event test secret' | Out-Null
    $conversation = ConvertFrom-CanonicalHex $event.input.conversation_id_hex 32 'event conversation ID'
    $author = ConvertFrom-CanonicalHex $event.expected.author_device_id_hex 32 'event author Device ID'
    $parentsHex = @($event.input.parents_hex | ForEach-Object { [string]$_ })
    Assert-CanonicalOrder $parentsHex 'event parents'
    $parents = @($parentsHex | ForEach-Object { ConvertFrom-CanonicalHex $_ 32 'event parent ID' })
    $acknowledged = ConvertFrom-CanonicalHex $event.input.acknowledged_event_id_hex 32 'acknowledged event ID'

    $content = [System.Collections.Generic.List[byte]]::new()
    Add-CborArrayHeader $content 6
    Add-CborUnsigned $content 1
    Add-CborByteString $content $conversation
    Add-CborByteString $content $author
    Add-CborUnsigned $content ([UInt64]$event.input.author_sequence)
    Add-CborArrayHeader $content ([UInt64]$parents.Count)
    foreach ($parent in $parents) {
        Add-CborByteString $content $parent
    }
    Add-CborArrayHeader $content 2
    Add-CborUnsigned $content 1
    Add-CborByteString $content $acknowledged
    $contentHex = ConvertTo-CanonicalHex $content.ToArray()
    if ($contentHex -cne [string]$event.expected.content_cbor_hex) {
        throw 'acknowledgement content does not match independently encoded CBOR'
    }

    $eventDomain = ConvertFrom-CanonicalHex $event.expected.signature_domain_hex -1 'event signature domain'
    if ((ConvertTo-CanonicalHex $eventDomain) -cne '6b696c6f6772616d3a6576656e742d7369676e61747572653a63626f722d763100') {
        throw 'event signature domain changed'
    }
    $preimage = [System.Collections.Generic.List[byte]]::new()
    $preimage.AddRange($eventDomain)
    $preimage.AddRange($content.ToArray())
    if ((ConvertTo-CanonicalHex $preimage.ToArray()) -cne [string]$event.expected.signing_preimage_hex) {
        throw 'event signing preimage is not domain plus canonical content'
    }
    $signature = ConvertFrom-CanonicalHex $event.expected.signature_hex 64 'event signature'
    $signedEvent = [System.Collections.Generic.List[byte]]::new()
    Add-CborArrayHeader $signedEvent 3
    Add-CborUnsigned $signedEvent 1
    $signedEvent.AddRange($content.ToArray())
    Add-CborByteString $signedEvent $signature
    if ((ConvertTo-CanonicalHex $signedEvent.ToArray()) -cne [string]$event.expected.signed_event_cbor_hex) {
        throw 'signed acknowledgement event does not match independently encoded CBOR'
    }
    if ((ConvertTo-CanonicalHex (ConvertFrom-CanonicalHex $event.expected.event_id_domain_hex -1 'event ID domain')) -cne
        '6b696c6f6772616d3a6576656e742d69643a63626f722d763100') {
        throw 'event ID domain changed'
    }
    ConvertFrom-CanonicalHex $event.expected.event_id_hex 32 'event ID' | Out-Null

    $inventory = $Vectors.sync_inventory
    Assert-ExactPropertySet $inventory @('input', 'expected') 'sync inventory vector'
    Assert-ExactPropertySet $inventory.input @(
        'author_secret_key_hex', 'conversation_id_hex', 'session_binding_hex', 'event_ids_hex'
    ) 'sync inventory vector input'
    Assert-ExactPropertySet $inventory.expected @(
        'requester_device_id_hex', 'content_cbor_hex', 'signature_domain_hex',
        'signing_preimage_hex', 'signature_hex', 'signed_inventory_cbor_hex'
    ) 'sync inventory vector expected'
    if ([string]$inventory.input.author_secret_key_hex -cne [string]$event.input.author_secret_key_hex) {
        throw 'golden vectors must retain one explicit deterministic test identity'
    }
    $inventoryConversation = ConvertFrom-CanonicalHex $inventory.input.conversation_id_hex 32 'inventory conversation ID'
    $requester = ConvertFrom-CanonicalHex $inventory.expected.requester_device_id_hex 32 'inventory requester Device ID'
    if ((ConvertTo-CanonicalHex $requester) -cne (ConvertTo-CanonicalHex $author)) {
        throw 'golden vectors disagree about the deterministic test Device ID'
    }
    $session = ConvertFrom-CanonicalHex $inventory.input.session_binding_hex 32 'inventory session binding'
    $eventIdsHex = @($inventory.input.event_ids_hex | ForEach-Object { [string]$_ })
    Assert-CanonicalOrder $eventIdsHex 'inventory event IDs'
    $eventIds = @($eventIdsHex | ForEach-Object { ConvertFrom-CanonicalHex $_ 32 'inventory event ID' })
    $inventoryContent = [System.Collections.Generic.List[byte]]::new()
    Add-CborArrayHeader $inventoryContent 5
    Add-CborUnsigned $inventoryContent 1
    Add-CborByteString $inventoryContent $inventoryConversation
    Add-CborByteString $inventoryContent $requester
    Add-CborByteString $inventoryContent $session
    Add-CborArrayHeader $inventoryContent ([UInt64]$eventIds.Count)
    foreach ($eventId in $eventIds) {
        Add-CborByteString $inventoryContent $eventId
    }
    if ((ConvertTo-CanonicalHex $inventoryContent.ToArray()) -cne [string]$inventory.expected.content_cbor_hex) {
        throw 'sync inventory content does not match independently encoded CBOR'
    }
    $inventoryDomain = ConvertFrom-CanonicalHex $inventory.expected.signature_domain_hex -1 'inventory signature domain'
    if ((ConvertTo-CanonicalHex $inventoryDomain) -cne
        '6b696c6f6772616d3a73796e632d696e76656e746f72792d7369676e61747572653a63626f722d763100') {
        throw 'sync inventory signature domain changed'
    }
    $inventoryPreimage = [System.Collections.Generic.List[byte]]::new()
    $inventoryPreimage.AddRange($inventoryDomain)
    $inventoryPreimage.AddRange($inventoryContent.ToArray())
    if ((ConvertTo-CanonicalHex $inventoryPreimage.ToArray()) -cne [string]$inventory.expected.signing_preimage_hex) {
        throw 'sync inventory signing preimage is not domain plus canonical content'
    }
    $inventorySignature = ConvertFrom-CanonicalHex $inventory.expected.signature_hex 64 'inventory signature'
    $signedInventory = [System.Collections.Generic.List[byte]]::new()
    Add-CborArrayHeader $signedInventory 3
    Add-CborUnsigned $signedInventory 1
    $signedInventory.AddRange($inventoryContent.ToArray())
    Add-CborByteString $signedInventory $inventorySignature
    if ((ConvertTo-CanonicalHex $signedInventory.ToArray()) -cne [string]$inventory.expected.signed_inventory_cbor_hex) {
        throw 'signed sync inventory does not match independently encoded CBOR'
    }
}

foreach ($path in @($schemaPath, $vectorsPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical wire artifact is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw -Encoding UTF8 | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-CanonicalSchema $schema $workspace
Assert-CanonicalVectors $vectors

if ($SelfTest) {
    $changedStatus = ConvertFrom-Json (ConvertTo-Json $schema -Depth 30)
    $changedStatus.status = 'active'
    Assert-Rejected { Assert-CanonicalSchema $changedStatus $workspace } 'active_without_migration'

    $removedObject = ConvertFrom-Json (ConvertTo-Json $schema -Depth 30)
    $removedObject.objects = @($removedObject.objects | Select-Object -Skip 1)
    Assert-Rejected { Assert-CanonicalSchema $removedObject $workspace } 'removed_schema_object'

    $changedBytes = ConvertFrom-Json (ConvertTo-Json $vectors -Depth 30)
    $changedBytes.acknowledgement_event.expected.content_cbor_hex = '00'
    Assert-Rejected { Assert-CanonicalVectors $changedBytes } 'changed_golden_bytes'

    $uppercaseHex = ConvertFrom-Json (ConvertTo-Json $vectors -Depth 30)
    $uppercaseHex.sync_inventory.input.session_binding_hex =
        ([string]$uppercaseHex.sync_inventory.input.session_binding_hex).ToUpperInvariant()
    Assert-Rejected { Assert-CanonicalVectors $uppercaseHex } 'noncanonical_hex'

    Write-Output 'canonical_wire_self_test=passed'
    Write-Output 'network_executed=false'
    Write-Output 'test_executables_started=false'
    Write-Output 'release_build=false'
    Write-Output 'archive_created=false'
    exit 0
}

Write-Output 'canonical_wire_boundary=verified'
Write-Output 'canonical_wire_milestone=M1.1.1'
Write-Output 'canonical_wire_profile=kilogram-deterministic-cbor-v1'
Write-Output 'canonical_wire_schema_objects=5'
Write-Output 'canonical_wire_golden_vectors=2'
Write-Output 'canonical_wire_activation_status=not-active'
Write-Output 'current_runtime_encoding=postcard'
Write-Output 'wire_changed=false'
Write-Output 'network_executed=false'
Write-Output 'test_executables_started=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
