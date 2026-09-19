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
