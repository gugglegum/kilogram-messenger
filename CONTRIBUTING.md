# Contributing to Kilogram

Thank you for helping with Kilogram. The repository currently represents an
experimental Windows technical proof of concept. It is not a production
security release, and changes must not describe it as one.

## Before starting

- Search existing issues before opening a new one.
- Small, isolated fixes may go directly to a pull request.
- Open an issue before substantial protocol, cryptography, identity, recovery,
  storage, transport, privacy, or user-interface work. Security-sensitive
  design changes normally need an RFC in `docs/`.
- Report vulnerabilities through the private process in
  [`SECURITY.md`](SECURITY.md), never through a public issue.
- Do not commit real seed phrases, private keys, IPC bearer descriptors,
  personal messages, machine identifiers, or field-test state.

## Development environment

The repository pins its development toolchain in `rust-toolchain.toml`. Cargo
metadata declares the minimum supported Rust version separately. Install
Rustup, clone the complete repository, and confirm the selected toolchain:

```powershell
rustup show active-toolchain
rustc -vV
cargo -vV
```

Keep `Cargo.lock` intact. Fetch dependencies explicitly when necessary, then
prefer locked and offline verification:

```powershell
cargo fetch --locked --target x86_64-pc-windows-msvc
$env:CARGO_NET_OFFLINE = 'true'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Tests on Windows

Do not execute network-bearing Rust harnesses directly from Cargo on Windows.
Cargo gives rebuilt test executables hash-derived paths, which can repeatedly
trigger Windows Firewall prompts. Use the stable-path runner instead:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File .\scripts\run-cargo-tests-stable.ps1 `
  -Profile test `
  -CargoJobs 2 `
  -IsolatedTestsFile .\scripts\windows-ci-isolated-tests.json `
  -IsolatedTestOrder BeforeAggregate `
  -Run `
  -NoFailFast
```

The explicit two-job limit keeps the development computer responsive. The
first stable executable path may still need one deliberate firewall decision.
Do not run live-network or physical field tests unless the pull request
actually changes the protected network surface and the test is coordinated.

For the complete non-release readiness gate:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File .\scripts\verify-kilogram-m1-readiness.ps1 `
  -AcceptedTag M1.0.0
```

This post-M1 mode verifies the immutable accepted tag and then checks the
current boundaries, formatting, and strict Clippy state. Omitting
`-AcceptedTag` deliberately selects the stricter candidate-preparation mode,
which requires `HEAD` itself to match the current candidate evidence.

Current GitHub CI runs on Windows. Host-compatible tests may also be run on
other operating systems, but they do not replace the recorded Windows
acceptance evidence.

## Pull requests

Keep pull requests focused and explain:

1. the problem and intended boundary;
2. security, privacy, compatibility, and migration consequences;
3. tests performed and tests deliberately not performed;
4. documentation or RFC changes;
5. whether existing field or reproducibility evidence remains applicable.

Do not edit accepted evidence merely to make a gate pass. When a protected
surface changes, record that the old evidence is stale and collect the
required new evidence. Avoid drive-by formatting or generated release/ZIP
artifacts.

## Contribution license

Unless explicitly agreed otherwise before submission, contributions are
provided under the repository's `AGPL-3.0-or-later` license. Project-specific
copyright headers are not currently required.
