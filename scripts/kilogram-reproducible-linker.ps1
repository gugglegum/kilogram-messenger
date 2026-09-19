Set-StrictMode -Version Latest

function Get-KilogramBundledLldIdentity {
    [CmdletBinding()]
    param(
        [string]$Target = 'x86_64-pc-windows-msvc'
    )

    if ($Target -cne 'x86_64-pc-windows-msvc') {
        throw "Unsupported reproducible-linker target: $Target"
    }

    $sysrootLines = @(& rustc --print sysroot)
    if ($LASTEXITCODE -ne 0 -or $sysrootLines.Count -ne 1 -or
        [string]::IsNullOrWhiteSpace([string]$sysrootLines[0])) {
        throw 'Could not resolve the pinned Rust toolchain sysroot.'
    }
    $sysroot = [System.IO.Path]::GetFullPath(([string]$sysrootLines[0]).Trim())
    $path = [System.IO.Path]::GetFullPath(
        (Join-Path $sysroot "lib\rustlib\$Target\bin\rust-lld.exe")
    )
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "The pinned Rust toolchain does not contain rust-lld.exe: $path"
    }
    $item = Get-Item -LiteralPath $path -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "The reproducible linker must not be a reparse point: $path"
    }

    [PSCustomObject]@{
        mode = 'rust-toolchain-bundled-lld'
        source = 'rustc-sysroot-target-bin'
        file = 'rust-lld.exe'
        flavor = 'lld-link'
        sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        bytes = [int64]$item.Length
        reproducibility_flag = '/Brepro'
        path = $path
    }
}

function New-KilogramReproducibleRustFlags {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$SourceRoot,

        [Parameter(Mandatory = $true)]
        [object]$LinkerIdentity
    )

    $source = [System.IO.Path]::GetFullPath($SourceRoot)
    $linker = [System.IO.Path]::GetFullPath([string]$LinkerIdentity.path)
    if (-not (Test-Path -LiteralPath $linker -PathType Leaf)) {
        throw "The recorded reproducible linker is missing: $linker"
    }
    if ([string]$LinkerIdentity.mode -cne 'rust-toolchain-bundled-lld' -or
        [string]$LinkerIdentity.flavor -cne 'lld-link' -or
        [string]$LinkerIdentity.reproducibility_flag -cne '/Brepro') {
        throw 'The reproducible linker identity is not the supported bundled LLD contract.'
    }

    @(
        "--remap-path-prefix=$source=Z:/kilogram-source",
        '-C',
        "linker=$linker",
        '-C',
        'linker-flavor=lld-link',
        '-C',
        'link-arg=/Brepro'
    )
}

function Read-KilogramPeReproducibilityMetadata {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "PE artifact is missing: $resolved"
    }
    $bytes = [System.IO.File]::ReadAllBytes($resolved)
    if ($bytes.Length -lt 0x100 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
        throw "Artifact is not a bounded PE file: $resolved"
    }
    $peOffset = [System.BitConverter]::ToUInt32($bytes, 0x3c)
    if ($peOffset -gt ($bytes.Length - 24) -or
        $bytes[$peOffset] -ne 0x50 -or
        $bytes[$peOffset + 1] -ne 0x45 -or
        $bytes[$peOffset + 2] -ne 0 -or
        $bytes[$peOffset + 3] -ne 0) {
        throw "Artifact has an invalid PE header: $resolved"
    }
    $optionalHeader = [int64]$peOffset + 24
    if ($optionalHeader -gt ($bytes.Length - 120) -or
        [System.BitConverter]::ToUInt16($bytes, [int]$optionalHeader) -ne 0x20b) {
        throw "Artifact is not the expected PE32+ image: $resolved"
    }
    $optionalHeaderBytes = [System.BitConverter]::ToUInt16($bytes, [int64]$peOffset + 20)
    $sectionCount = [System.BitConverter]::ToUInt16($bytes, [int64]$peOffset + 6)
    $sectionTable = $optionalHeader + $optionalHeaderBytes
    if ($optionalHeaderBytes -lt 168 -or
        $sectionCount -le 0 -or
        $sectionTable + ([int64]$sectionCount * 40) -gt $bytes.Length) {
        throw "Artifact has an invalid PE section table: $resolved"
    }
    if ([System.BitConverter]::ToUInt32($bytes, [int]$optionalHeader + 64) -ne 0) {
        throw "Refusing to normalize a PE image with a non-zero checksum: $resolved"
    }

    $timestampOffsets = [System.Collections.Generic.List[int64]]::new()
    $guidOffsets = [System.Collections.Generic.List[int64]]::new()
    $timestampOffsets.Add([int64]$peOffset + 8)

    $numberOfDirectories = [System.BitConverter]::ToUInt32($bytes, [int]$optionalHeader + 108)
    if ($numberOfDirectories -gt 4) {
        $securityDirectory = [int64]$optionalHeader + 112 + (4 * 8)
        $certificateOffset = [System.BitConverter]::ToUInt32($bytes, [int]$securityDirectory)
        $certificateBytes = [System.BitConverter]::ToUInt32($bytes, [int]$securityDirectory + 4)
        if ($certificateOffset -ne 0 -or $certificateBytes -ne 0) {
            throw "Refusing to normalize an Authenticode-bearing PE image: $resolved"
        }
    }
    if ($numberOfDirectories -le 6) {
        return [PSCustomObject]@{
            Path = $resolved
            Bytes = $bytes
            TimestampOffsets = @($timestampOffsets)
            CodeViewGuidOffsets = @($guidOffsets)
        }
    }
    $debugDirectoryEntry = [int64]$optionalHeader + 112 + (6 * 8)
    if (($debugDirectoryEntry + 8) -gt ($optionalHeader + $optionalHeaderBytes) -or
        $debugDirectoryEntry -gt ($bytes.Length - 8)) {
        throw "Artifact has a truncated PE data-directory table: $resolved"
    }
    $debugRva = [System.BitConverter]::ToUInt32($bytes, [int]$debugDirectoryEntry)
    $debugBytes = [System.BitConverter]::ToUInt32($bytes, [int]$debugDirectoryEntry + 4)
    if ($debugRva -eq 0 -and $debugBytes -eq 0) {
        return [PSCustomObject]@{
            Path = $resolved
            Bytes = $bytes
            TimestampOffsets = @($timestampOffsets)
            CodeViewGuidOffsets = @($guidOffsets)
        }
    }
    if ($debugRva -eq 0 -or $debugBytes -eq 0 -or ($debugBytes % 28) -ne 0) {
        throw "Artifact has an invalid PE debug directory: $resolved"
    }

    $debugFileOffset = $null
    for ($sectionIndex = 0; $sectionIndex -lt $sectionCount; $sectionIndex++) {
        $section = $sectionTable + ([int64]$sectionIndex * 40)
        $virtualAddress = [System.BitConverter]::ToUInt32($bytes, [int]$section + 12)
        $rawSize = [System.BitConverter]::ToUInt32($bytes, [int]$section + 16)
        $rawPointer = [System.BitConverter]::ToUInt32($bytes, [int]$section + 20)
        if ($debugRva -ge $virtualAddress -and
            ([int64]$debugRva + $debugBytes) -le ([int64]$virtualAddress + $rawSize)) {
            $candidate = [int64]$rawPointer + ([int64]$debugRva - $virtualAddress)
            if ($candidate + $debugBytes -gt $bytes.Length) {
                throw "Artifact debug directory escapes its PE section: $resolved"
            }
            $debugFileOffset = $candidate
            break
        }
    }
    if ($null -eq $debugFileOffset) {
        throw "Artifact debug-directory RVA does not map to raw PE data: $resolved"
    }

    for ($entry = 0; $entry -lt ($debugBytes / 28); $entry++) {
        $entryOffset = [int64]$debugFileOffset + ([int64]$entry * 28)
        $timestampOffsets.Add($entryOffset + 4)
        $type = [System.BitConverter]::ToUInt32($bytes, [int]$entryOffset + 12)
        if ($type -ne 2) {
            continue
        }
        $dataBytes = [System.BitConverter]::ToUInt32($bytes, [int]$entryOffset + 16)
        $dataPointer = [System.BitConverter]::ToUInt32($bytes, [int]$entryOffset + 24)
        if ($dataBytes -lt 24 -or ([int64]$dataPointer + $dataBytes) -gt $bytes.Length -or
            $bytes[$dataPointer] -ne 0x52 -or
            $bytes[$dataPointer + 1] -ne 0x53 -or
            $bytes[$dataPointer + 2] -ne 0x44 -or
            $bytes[$dataPointer + 3] -ne 0x53) {
            throw "Artifact contains an invalid CodeView RSDS record: $resolved"
        }
        $guidOffsets.Add([int64]$dataPointer + 4)
    }

    [PSCustomObject]@{
        Path = $resolved
        Bytes = $bytes
        TimestampOffsets = @($timestampOffsets)
        CodeViewGuidOffsets = @($guidOffsets)
    }
}

function Assert-KilogramPeReproducibilityMetadataNormalized {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $metadata = Read-KilogramPeReproducibilityMetadata -Path $Path
    foreach ($offset in @($metadata.TimestampOffsets)) {
        for ($index = 0; $index -lt 4; $index++) {
            if ($metadata.Bytes[$offset + $index] -ne 0) {
                throw "PE reproducibility timestamp is not normalized: $($metadata.Path)"
            }
        }
    }
    foreach ($offset in @($metadata.CodeViewGuidOffsets)) {
        for ($index = 0; $index -lt 16; $index++) {
            if ($metadata.Bytes[$offset + $index] -ne 0) {
                throw "PE CodeView GUID is not normalized: $($metadata.Path)"
            }
        }
    }
}

function Normalize-KilogramPeReproducibilityMetadata {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $metadata = Read-KilogramPeReproducibilityMetadata -Path $Path
    foreach ($offset in @($metadata.TimestampOffsets)) {
        for ($index = 0; $index -lt 4; $index++) {
            $metadata.Bytes[$offset + $index] = 0
        }
    }
    foreach ($offset in @($metadata.CodeViewGuidOffsets)) {
        for ($index = 0; $index -lt 16; $index++) {
            $metadata.Bytes[$offset + $index] = 0
        }
    }
    [System.IO.File]::WriteAllBytes($metadata.Path, $metadata.Bytes)
    Assert-KilogramPeReproducibilityMetadataNormalized -Path $metadata.Path
}
