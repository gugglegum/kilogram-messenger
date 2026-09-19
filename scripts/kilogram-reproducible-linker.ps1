Set-StrictMode -Version Latest

function Get-KilogramCargoRegistrySourceRoot {
    [CmdletBinding()]
    param()

    $cargoHome = if (-not [string]::IsNullOrWhiteSpace($env:CARGO_HOME)) {
        if (-not [System.IO.Path]::IsPathRooted($env:CARGO_HOME)) {
            throw 'CARGO_HOME must be an absolute path for a reproducible build.'
        }
        [System.IO.Path]::GetFullPath($env:CARGO_HOME)
    }
    else {
        if ([string]::IsNullOrWhiteSpace($env:USERPROFILE) -or
            -not [System.IO.Path]::IsPathRooted($env:USERPROFILE)) {
            throw 'Could not resolve the user profile used by the default Cargo home.'
        }
        [System.IO.Path]::GetFullPath((Join-Path $env:USERPROFILE '.cargo'))
    }
    $registrySource = [System.IO.Path]::GetFullPath((Join-Path $cargoHome 'registry\src'))
    if (-not (Test-Path -LiteralPath $registrySource -PathType Container)) {
        throw "Cargo registry source root is missing; fetch locked dependencies first: $registrySource"
    }
    $item = Get-Item -LiteralPath $registrySource -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Cargo registry source root must not be a reparse point: $registrySource"
    }
    $registrySource
}

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
        [string]$CargoRegistrySourceRoot,

        [Parameter(Mandatory = $true)]
        [object]$LinkerIdentity
    )

    $source = [System.IO.Path]::GetFullPath($SourceRoot)
    $cargoRegistrySource = [System.IO.Path]::GetFullPath($CargoRegistrySourceRoot)
    if (-not (Test-Path -LiteralPath $cargoRegistrySource -PathType Container)) {
        throw "Cargo registry source root is missing: $cargoRegistrySource"
    }
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
        "--remap-path-prefix=$cargoRegistrySource=Z:/cargo-registry-src",
        '-C',
        "linker=$linker",
        '-C',
        'linker-flavor=lld-link',
        '-C',
        'link-arg=/Brepro'
    )
}

function Assert-KilogramPeCanonicalPathRemapping {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [string]$HostSourceRoot,

        [string]$CargoRegistrySourceRoot
    )

    $resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "PE artifact is missing for embedded-path verification: $resolved"
    }
    $item = Get-Item -LiteralPath $resolved -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
        $item.Length -le 0 -or $item.Length -gt 256MB) {
        throw "PE artifact has an invalid embedded-path verification boundary: $resolved"
    }

    $ascii = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($resolved))
    $canonicalCargoRegistryRoot = 'Z:/cargo-registry-src'
    if ($ascii.IndexOf($canonicalCargoRegistryRoot, [System.StringComparison]::Ordinal) -lt 0) {
        throw 'PE artifact does not contain the canonical Cargo registry source root.'
    }
    foreach ($rawCargoMarker in @('.cargo\registry\src', '.cargo/registry/src')) {
        if ($ascii.IndexOf($rawCargoMarker, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "PE artifact leaks a host Cargo registry source path: $rawCargoMarker"
        }
    }
    foreach ($hostRoot in @($HostSourceRoot, $CargoRegistrySourceRoot)) {
        if ([string]::IsNullOrWhiteSpace($hostRoot)) {
            continue
        }
        $fullRoot = [System.IO.Path]::GetFullPath($hostRoot)
        foreach ($variant in @($fullRoot, $fullRoot.Replace('\', '/'))) {
            if ($ascii.IndexOf($variant, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                throw "PE artifact leaks a host build path: $variant"
            }
        }
    }

    [PSCustomObject]@{
        mode = 'rustc-dual-prefix-remap-with-pe-leak-check-v1'
        source_root = '<BUILD_ROOT>=Z:/kilogram-source'
        cargo_registry_source_root = '<CARGO_REGISTRY_SOURCE_ROOT>=Z:/cargo-registry-src'
        canonical_cargo_registry_source_present = $true
        raw_cargo_registry_source_absent = $true
    }
}

function Assert-KilogramNativeLinkInputManifest {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "Native link-input manifest is missing: $resolved"
    }
    $item = Get-Item -LiteralPath $resolved -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
        $item.Length -le 0 -or $item.Length -gt 64KB) {
        throw "Native link-input manifest has an invalid file boundary: $resolved"
    }

    $lines = @(Get-Content -LiteralPath $resolved)
    if ($lines.Count -le 0 -or $lines.Count -gt 256) {
        throw "Native link-input manifest has an invalid entry count: $resolved"
    }
    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    $logicalNames = [System.Collections.Generic.List[string]]::new()
    $hasWindowsUm = $false
    $hasWindowsUcrt = $false
    $hasMsvc = $false
    foreach ($line in $lines) {
        if ($line -cnotmatch '^([0-9a-f]{64})  ([1-9][0-9]*)  ((?:windows-sdk/[A-Za-z0-9._-]+/(?:um|ucrt)/x64/[a-z0-9._-]+\.lib)|(?:msvc/[A-Za-z0-9._-]+/lib/x64/[a-z0-9._-]+\.lib))$') {
            throw "Native link-input manifest contains an invalid entry: $line"
        }
        $logicalName = $Matches[3]
        if (-not $seen.Add($logicalName)) {
            throw "Native link-input manifest contains a duplicate logical path: $logicalName"
        }
        $logicalNames.Add($logicalName)
        if ($logicalName.StartsWith('windows-sdk/', [System.StringComparison]::Ordinal)) {
            if ($logicalName.Contains('/um/')) {
                $hasWindowsUm = $true
            }
            if ($logicalName.Contains('/ucrt/')) {
                $hasWindowsUcrt = $true
            }
        }
        elseif ($logicalName.StartsWith('msvc/', [System.StringComparison]::Ordinal)) {
            $hasMsvc = $true
        }
    }
    $sortedLogicalNames = @($logicalNames | Sort-Object)
    for ($index = 0; $index -lt $logicalNames.Count; $index++) {
        if ($logicalNames[$index] -cne $sortedLogicalNames[$index]) {
            throw 'Native link-input manifest entries are not sorted by logical path.'
        }
    }
    if (-not $hasWindowsUm -or -not $hasWindowsUcrt -or -not $hasMsvc) {
        throw 'Native link-input manifest does not cover Windows UM, UCRT, and MSVC libraries.'
    }

    [PSCustomObject]@{
        file = [System.IO.Path]::GetFileName($resolved)
        sha256 = (Get-FileHash -LiteralPath $resolved -Algorithm SHA256).Hash.ToLowerInvariant()
        count = $lines.Count
    }
}

function Get-KilogramCanonicalNativeLinkInputSha256 {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Lines
    )

    if ($Lines.Count -le 0) {
        throw 'Cannot hash an empty native link-input line set.'
    }
    $payload = ($Lines -join "`r`n") + "`r`n"
    $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($payload)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        ([System.BitConverter]::ToString($algorithm.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
    }
}

function Test-KilogramLockedNativeFile {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Sha256,

        [Parameter(Mandatory = $true)]
        [int64]$Bytes
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $false
    }
    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
        [int64]$item.Length -ne $Bytes) {
        return $false
    }
    ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ceq $Sha256)
}

function Get-KilogramPinnedNativeToolchainIdentity {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$LockPath
    )

    $lock = [System.IO.Path]::GetFullPath($LockPath)
    $lockIdentity = Assert-KilogramNativeLinkInputManifest -Path $lock
    $lines = @(Get-Content -LiteralPath $lock)
    $entries = [System.Collections.Generic.List[object]]::new()
    foreach ($line in $lines) {
        if ($line -cnotmatch '^([0-9a-f]{64})  ([1-9][0-9]*)  (.+)$') {
            throw "Pinned native link-input lock contains an invalid entry: $line"
        }
        $entries.Add([PSCustomObject]@{
            sha256 = $Matches[1]
            bytes = [int64]$Matches[2]
            logical_name = $Matches[3]
        })
    }

    $expectedSuffixes = @(
        'msvc/lib/x64/msvcrt.lib',
        'msvc/lib/x64/vcruntime.lib',
        'windows-sdk/ucrt/x64/ucrt.lib',
        'windows-sdk/um/x64/advapi32.lib',
        'windows-sdk/um/x64/bcrypt.lib',
        'windows-sdk/um/x64/dbghelp.lib',
        'windows-sdk/um/x64/kernel32.lib',
        'windows-sdk/um/x64/ntdll.lib',
        'windows-sdk/um/x64/userenv.lib',
        'windows-sdk/um/x64/ws2_32.lib'
    )
    $actualSuffixes = @($entries | ForEach-Object {
        if ($_.logical_name -cmatch '^msvc/[^/]+/(lib/x64/.+)$') {
            "msvc/$($Matches[1])"
        }
        elseif ($_.logical_name -cmatch '^windows-sdk/[^/]+/((?:um|ucrt)/x64/.+)$') {
            "windows-sdk/$($Matches[1])"
        }
        else {
            throw "Pinned native link-input lock has an unsupported logical path: $($_.logical_name)"
        }
    } | Sort-Object)
    if (($actualSuffixes -join "`n") -cne (($expectedSuffixes | Sort-Object) -join "`n")) {
        throw 'Pinned native link-input lock does not contain the exact supported ten-library set.'
    }

    $msvcVersions = @($entries | ForEach-Object {
        if ($_.logical_name -cmatch '^msvc/([^/]+)/') { $Matches[1] }
    } | Sort-Object -Unique)
    $sdkVersions = @($entries | ForEach-Object {
        if ($_.logical_name -cmatch '^windows-sdk/([^/]+)/') { $Matches[1] }
    } | Sort-Object -Unique)
    if ($msvcVersions.Count -ne 1 -or $sdkVersions.Count -ne 1) {
        throw 'Pinned native link-input lock must select exactly one MSVC and one Windows SDK version.'
    }
    $msvcVersion = [string]$msvcVersions[0]
    $sdkVersion = [string]$sdkVersions[0]

    $programRoots = @($env:ProgramFiles, ${env:ProgramFiles(x86)}) |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
        Sort-Object -Unique
    $msvcCandidates = [System.Collections.Generic.List[string]]::new()
    foreach ($programRoot in $programRoots) {
        $visualStudioRoot = Join-Path $programRoot 'Microsoft Visual Studio'
        if (-not (Test-Path -LiteralPath $visualStudioRoot -PathType Container)) {
            continue
        }
        foreach ($generation in @(Get-ChildItem -LiteralPath $visualStudioRoot -Directory -ErrorAction SilentlyContinue)) {
            foreach ($edition in @(Get-ChildItem -LiteralPath $generation.FullName -Directory -ErrorAction SilentlyContinue)) {
                $candidate = Join-Path $edition.FullName "VC\Tools\MSVC\$msvcVersion"
                if (Test-Path -LiteralPath $candidate -PathType Container) {
                    $msvcCandidates.Add([System.IO.Path]::GetFullPath($candidate))
                }
            }
        }
    }
    $msvcEntries = @($entries | Where-Object { $_.logical_name.StartsWith('msvc/', [System.StringComparison]::Ordinal) })
    $matchingMsvcRoots = @($msvcCandidates | Sort-Object -Unique | Where-Object {
        $candidate = $_
        $valid = $true
        foreach ($entry in $msvcEntries) {
            $name = [System.IO.Path]::GetFileName([string]$entry.logical_name)
            $path = Join-Path $candidate "lib\x64\$name"
            if (-not (Test-KilogramLockedNativeFile -Path $path -Sha256 $entry.sha256 -Bytes $entry.bytes)) {
                $valid = $false
                break
            }
        }
        $valid
    })
    if ($matchingMsvcRoots.Count -le 0) {
        throw "Pinned MSVC native inputs are not installed exactly: $msvcVersion"
    }
    $msvcRoot = [string]$matchingMsvcRoots[0]

    $sdkCandidates = [System.Collections.Generic.List[string]]::new()
    foreach ($programRoot in $programRoots) {
        $candidate = Join-Path $programRoot "Windows Kits\10\Lib\$sdkVersion"
        if (Test-Path -LiteralPath $candidate -PathType Container) {
            $sdkCandidates.Add([System.IO.Path]::GetFullPath($candidate))
        }
    }
    $sdkEntries = @($entries | Where-Object { $_.logical_name.StartsWith('windows-sdk/', [System.StringComparison]::Ordinal) })
    $matchingSdkRoots = @($sdkCandidates | Sort-Object -Unique | Where-Object {
        $candidate = $_
        $valid = $true
        foreach ($entry in $sdkEntries) {
            if ($entry.logical_name -cnotmatch '^windows-sdk/[^/]+/(um|ucrt)/x64/([^/]+)$') {
                $valid = $false
                break
            }
            $path = Join-Path $candidate "$($Matches[1])\x64\$($Matches[2])"
            if (-not (Test-KilogramLockedNativeFile -Path $path -Sha256 $entry.sha256 -Bytes $entry.bytes)) {
                $valid = $false
                break
            }
        }
        $valid
    })
    if ($matchingSdkRoots.Count -le 0) {
        throw "Pinned Windows SDK native inputs are not installed exactly: $sdkVersion"
    }
    $sdkRoot = [string]$matchingSdkRoots[0]

    $searchPaths = @(
        (Join-Path $msvcRoot 'lib\x64'),
        (Join-Path $sdkRoot 'ucrt\x64'),
        (Join-Path $sdkRoot 'um\x64')
    )
    $rustcArguments = [System.Collections.Generic.List[string]]::new()
    foreach ($searchPath in $searchPaths) {
        $rustcArguments.Add('-L')
        $rustcArguments.Add("native=$searchPath")
    }

    [PSCustomObject]@{
        mode = 'repository-hash-locked-installed-libraries'
        selection = 'explicit-final-rustc-native-search-paths'
        lock_file = [System.IO.Path]::GetFileName($lock)
        lock_sha256 = Get-KilogramCanonicalNativeLinkInputSha256 -Lines $lines
        count = $lockIdentity.count
        msvc_version = $msvcVersion
        windows_sdk_version = $sdkVersion
        architecture = 'x64'
        libraries_bundled = $false
        rustc_arguments = @($rustcArguments)
    }
}

function Assert-KilogramNativeLinkInputManifestMatchesLock {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ManifestPath,

        [Parameter(Mandatory = $true)]
        [string]$LockPath
    )

    $manifestIdentity = Assert-KilogramNativeLinkInputManifest -Path $ManifestPath
    $null = Assert-KilogramNativeLinkInputManifest -Path $LockPath
    $manifestLines = @(Get-Content -LiteralPath $ManifestPath)
    $lockLines = @(Get-Content -LiteralPath $LockPath)
    if (($manifestLines -join "`n") -cne ($lockLines -join "`n")) {
        throw 'The final link did not consume the exact repository-locked native input set.'
    }
    $canonicalLockHash = Get-KilogramCanonicalNativeLinkInputSha256 -Lines $lockLines
    if ([string]$manifestIdentity.sha256 -cne $canonicalLockHash) {
        throw 'The generated native link-input manifest is not in canonical CRLF form.'
    }
    $manifestIdentity
}

function Write-KilogramNativeLinkInputManifest {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ArchivePath,

        [Parameter(Mandatory = $true)]
        [string]$ManifestPath
    )

    $archive = [System.IO.Path]::GetFullPath($ArchivePath)
    $manifest = [System.IO.Path]::GetFullPath($ManifestPath)
    if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
        throw "LLD link-reproduction archive is missing: $archive"
    }
    $archiveItem = Get-Item -LiteralPath $archive -Force
    if (($archiveItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
        $archiveItem.Length -le 0 -or $archiveItem.Length -gt 2GB) {
        throw "LLD link-reproduction archive has an invalid file boundary: $archive"
    }
    if (Test-Path -LiteralPath $manifest) {
        throw "Native link-input manifest already exists: $manifest"
    }
    $manifestParent = Split-Path -Parent $manifest
    if (-not (Test-Path -LiteralPath $manifestParent -PathType Container)) {
        throw "Native link-input manifest directory is missing: $manifestParent"
    }
    $tar = Get-Command tar.exe -ErrorAction SilentlyContinue
    if ($null -eq $tar) {
        throw 'Windows tar.exe is required to inspect the bounded LLD reproduction archive.'
    }

    $archiveEntries = @(& $tar.Source -tf $archive)
    if ($LASTEXITCODE -ne 0 -or $archiveEntries.Count -le 0) {
        throw "Could not list the LLD link-reproduction archive: $archive"
    }
    $nativeEntries = @($archiveEntries | Where-Object { $_ -cmatch '(?i)\.lib$' })
    if ($nativeEntries.Count -le 0 -or $nativeEntries.Count -gt 256) {
        throw "LLD link-reproduction archive has an invalid native library count: $($nativeEntries.Count)"
    }

    $extractRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
        'kilogram-native-link-inputs-' + [Guid]::NewGuid().ToString('N')
    )
    New-Item -ItemType Directory -Path $extractRoot | Out-Null
    $extractRootResolved = [System.IO.Path]::GetFullPath($extractRoot)
    $extractPrefix = $extractRootResolved.TrimEnd('\') + '\'
    $records = [System.Collections.Generic.List[object]]::new()
    $logicalNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    try {
        foreach ($entry in $nativeEntries) {
            if ([string]::IsNullOrWhiteSpace($entry) -or
                $entry.StartsWith('/', [System.StringComparison]::Ordinal) -or
                $entry.Contains('\') -or
                $entry.Contains(':') -or
                $entry -match '(^|/)\.\.(/|$)') {
                throw "LLD link-reproduction archive contains an unsafe native library path: $entry"
            }

            $logicalName = $null
            if ($entry -match '(?i)/Windows Kits/10/lib/([^/]+)/(um|ucrt)/(x64)/([^/]+\.lib)$') {
                $logicalName = 'windows-sdk/{0}/{1}/{2}/{3}' -f
                    $Matches[1],
                    $Matches[2].ToLowerInvariant(),
                    $Matches[3].ToLowerInvariant(),
                    $Matches[4].ToLowerInvariant()
            }
            elseif ($entry -match '(?i)/VC/Tools/MSVC/([^/]+)/lib/(x64)/([^/]+\.lib)$') {
                $logicalName = 'msvc/{0}/lib/{1}/{2}' -f
                    $Matches[1],
                    $Matches[2].ToLowerInvariant(),
                    $Matches[3].ToLowerInvariant()
            }
            else {
                throw "LLD consumed an unclassified native library: $entry"
            }
            if (-not $logicalNames.Add($logicalName)) {
                throw "LLD consumed duplicate native library identities: $logicalName"
            }

            & $tar.Source -xf $archive -C $extractRootResolved -- $entry
            if ($LASTEXITCODE -ne 0) {
                throw "Could not extract a native link input from the LLD archive: $entry"
            }
            $extracted = [System.IO.Path]::GetFullPath((Join-Path $extractRootResolved $entry.Replace('/', '\')))
            if (-not $extracted.StartsWith($extractPrefix, [System.StringComparison]::OrdinalIgnoreCase) -or
                -not (Test-Path -LiteralPath $extracted -PathType Leaf)) {
                throw "Extracted native link input escaped or is missing: $entry"
            }
            $extractedItem = Get-Item -LiteralPath $extracted -Force
            if (($extractedItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
                $extractedItem.Length -le 0) {
                throw "Extracted native link input has an invalid file boundary: $entry"
            }
            $records.Add([PSCustomObject]@{
                logical_name = $logicalName
                sha256 = (Get-FileHash -LiteralPath $extracted -Algorithm SHA256).Hash.ToLowerInvariant()
                bytes = [int64]$extractedItem.Length
            })
        }

        $lines = @($records | Sort-Object logical_name | ForEach-Object {
            "$($_.sha256)  $($_.bytes)  $($_.logical_name)"
        })
        [System.IO.File]::WriteAllText(
            $manifest,
            (($lines -join "`r`n") + "`r`n"),
            [System.Text.UTF8Encoding]::new($false)
        )
        Assert-KilogramNativeLinkInputManifest -Path $manifest
    }
    finally {
        $resolvedTemp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if ($extractRootResolved.StartsWith($resolvedTemp, [System.StringComparison]::OrdinalIgnoreCase) -and
            [System.IO.Path]::GetFileName($extractRootResolved).StartsWith(
                'kilogram-native-link-inputs-',
                [System.StringComparison]::Ordinal
            ) -and
            (Test-Path -LiteralPath $extractRootResolved)) {
            Remove-Item -LiteralPath $extractRootResolved -Recurse -Force
        }
    }
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
