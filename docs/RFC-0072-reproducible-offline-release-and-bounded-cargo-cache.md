# RFC-0072: Reproducible offline release and bounded Cargo cache (M0.9.50)

Status: implemented in M0.9.50.

## 1. Problem

M0.9.48 made the portable ZIP deterministic, but both packages reused one
already-built executable. Equal ZIP hashes therefore proved deterministic
packaging, not repeatable compilation from reviewed source. At the same time,
repeated workspace test builds accumulated multiple generations of incremental
objects and full Windows PDB files in `target/debug`; the local Cargo target
tree eventually reached 105.66 GiB.

M0.9.50 adds a fail-closed clean-root rebuild comparison for the isolated
`kilogram-offline` executable and separates fast development caches from the
larger verification workload.

## 2. Reproducibility gate

`scripts/build-kilogram-offline-reproducible.ps1` accepts a clean Git worktree
by default and:

1. resolves the exact tracked source set and writes a sorted SHA-256 source
   manifest;
2. copies that set into two separate clean source roots;
3. gives each build a separate Cargo target root;
4. builds only `kilogram-offline` for `x86_64-pc-windows-msvc` using
   `cargo --frozen --release`, `CARGO_INCREMENTAL=0`, the commit timestamp as
   `SOURCE_DATE_EPOCH`, a common source-path remap and `/Brepro`;
5. requires the two executable lengths and SHA-256 hashes to match exactly;
6. writes a bounded JSON record with source, lockfile, toolchain, command
   boundary and both artifact identities; and
7. removes the temporary source and target roots unless explicitly retained.

The repository pins Rust 1.98.0, the minimal rustup profile, rustfmt, Clippy and
the Windows MSVC target in `rust-toolchain.toml`. `Cargo.lock` and the toolchain
file are copied into the record and hashed. `--frozen` prevents an unnoticed
lockfile update or network fetch during either comparison build.

`scripts/verify-kilogram-offline-reproducibility-record.ps1` is a separate
bounded verifier. It rejects unknown build-boundary values, unsafe file names,
reparse-point payloads, malformed hashes, changed inputs and any mismatch
between the two executables.

## 3. Release package gate

A clean release invocation of `scripts/package-kilogram-offline.ps1` now
requires a verified reproducibility-record directory whose clean source
revision is exactly the current `HEAD`. It packages the reproduced executable,
record, source manifest, lockfile and toolchain file. `BUILD-INFO.txt` records
`reproducibility_verified=true` and the artifact/record/input hashes.

`-AllowDirty` remains an explicit development escape hatch. Such a package is
marked `reproducibility_verified=false`; it cannot reuse a release
reproducibility record. This prevents an ordinary clean release from silently
falling back to a single unverified local build.

The package SHA-256 manifest detects accidental corruption. It is not a
signature and must not be used as the only authenticated distribution channel.

## 4. Cargo cache policy

Everyday `cargo check` keeps Cargo's normal incremental development profile.
Full workspace test-harness compilation uses the new `verification` profile:

- `inherits = "test"`;
- line-table debug information instead of full PDB-oriented debug data; and
- incremental compilation disabled.

`scripts/run-cargo-tests-stable.ps1` selects this profile by default and still
only compiles/copies stable-name harnesses unless `-Run` is explicitly passed.
No network-bearing test executable is started by cache maintenance.

Cold Cargo builds also follow an interactive resource policy. Project scripts
lower their parent process priority to `BelowNormal` and use half of the
available logical processors by default (12 jobs on the 24-thread development
machine). `-CargoJobs` or `KILOGRAM_CARGO_JOBS` can override that value.
`scripts/invoke-cargo-friendly.ps1` applies the same policy to ad-hoc build,
check, Clippy and test commands. This deliberately trades some cold-build time
for responsive typing, mouse input and VMware guests; warm incremental builds
keep their usual cache advantage.

`scripts/cargo-cache-maintenance.ps1` is read-only by default. It reports the
development incremental, complete debug, verification, release and total
sizes and warns above 40 GiB. The stable harness workflow invokes this
read-only report after every compile/run, so milestone verification cannot
silently grow past the threshold. `-PruneVerification` removes only the
disposable verification profile. `-VacuumAndWarm` is the explicit full reset: it deletes
the exact workspace `target`, then warms check, verification-compile and
release caches. Every destructive path is resolved and checked before removal.

The one-time M0.9.50 reset removed 145,230 files / 105.66 GiB. After workspace
check, verification-harness compilation and the first offline release build,
the measured target tree was 3.99 GiB. Future source changes can still grow the
cache, but the dominant full-debug incremental test generations no longer
share the everyday profile, and the disposable verification cache has a
targeted cleanup path.

## 5. Security boundary and limitations

The current record proves two byte-identical builds from separate clean source
and target roots on the same Windows host. It does not prove reproduction on a
second independent machine, a pinned VM/container image or a separately
administered build service. The global Cargo registry/cache and host OS/tooling
are outside the clean roots. `Cargo.lock` authenticates registry crate content,
but the record is not signed release provenance.

The release process therefore must describe this result as
`same-host-separate-clean-roots`, not as independent multi-party reproduction.
A second builder, published/signed provenance and authenticated release hashes
remain required before a public security-sensitive release.

No wire, identity, Root, conflict-artifact or network protocol changes in this
milestone. Network-bearing Rust harnesses were not executed, avoiding an
unexpected Windows Firewall approval dialog.
