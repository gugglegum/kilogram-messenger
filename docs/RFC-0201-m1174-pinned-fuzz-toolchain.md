# RFC-0201: M1.1.74 pinned instrumented test toolchain

## Status

Accepted locally. This stage prepares and verifies the host toolchain; it does
not claim a completed smoke or sustained campaign.

## Decision

The canonical sync v2 decoder target uses the checked-in `fuzz/Cargo.lock` and
an exact tool contract recorded in
`CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json`. The runner rejects an unexpected
Rust compiler commit or `cargo-fuzz` version before starting a campaign.

The Windows preflight discovers both the MSVC AddressSanitizer runtime DLL and
its import library under `VC/Tools/MSVC`. The old LLVM-only probe was not a
valid representation of a Visual Studio 2022 installation. The selected DLL
directory is prepended only to the child process environment and the original
`PATH` is restored afterwards.

## Evidence

On the prepared Windows x86-64 host, Rust nightly
`5d8fec0669a64607db9ccbf663e7c23125bf4748`, `cargo-fuzz 0.13.2`,
`libfuzzer-sys 0.4.13`, and the Visual Studio component
`Microsoft.VisualStudio.Component.VC.ASAN` produced the instrumented target
successfully. The target was built from source revision
`6c3e0c7280c023a34146d8268e73679539b86adb`; its SHA-256 and size are recorded
in the toolchain manifest.

The earlier diagnostic build without the required Windows component failed at
the final linker boundary, while the same standard build completed after the
component was installed. This proves the host prerequisite rather than
silently weakening the campaign configuration.

## Non-effects

This stage does not register or dial `kilogram/m1/sync/cbor/2`, does not alter
the active Postcard v6 transport, and does not close the sustained campaign or
fresh field-evidence gates.
