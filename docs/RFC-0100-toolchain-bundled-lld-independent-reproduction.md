# RFC-0100: Toolchain-bundled LLD independent reproduction (M0.9.77)

Status: implemented and locally verified; a matching GitHub-hosted build and
attestation still require an explicit post-push workflow dispatch.

## 1. Problem

The first M0.9.59 independent Windows build compiled the intended source but
failed byte equality. The local executable used Microsoft `link.exe` 14.44,
while the GitHub-hosted executable used 14.51. Their PE layout differed beyond
timestamps: section sizes, import/IAT placement and RVAs changed, leaving a
1,024-byte whole-file difference. Normalizing metadata would therefore weaken
the artifact claim rather than fix the build input.

Rust and Cargo were already pinned by `rust-toolchain.toml`; the selected MSVC
linker was not. M0.9.77 removes that mutable choice from the release boundary.
The target graph also contained BLAKE3's optional C/assembly build path, so the
offline application now selects the crate's supported `pure` feature rather
than allowing a hosted MSVC version to affect linked native objects.

## 2. Controlled linker

`scripts/kilogram-reproducible-linker.ps1` resolves
`rust-lld.exe` from the target-specific `bin` directory below the active
`rustc --print sysroot`. The helper requires the Windows MSVC target, rejects a
missing or reparse-point linker and records its exact SHA-256 and length.

Both reproducible builders pass the absolute resolved path through stable
compiler options:

- `-C linker=<PINNED_RUST_SYSROOT>/.../rust-lld.exe`;
- `-C linker-flavor=lld-link`;
- `-C link-arg=/Brepro`.

The unstable `-C linker-features=+lld` option is deliberately forbidden. The
path itself is host-local and is not claimed to be portable; byte identity is
bound to the hash and length of the executable found inside the already pinned
Rust toolchain. Each builder resolves the identity again after linking and
fails if the path, hash or length changed during the build.

For `kilogram-offline`, the direct BLAKE3 dependency enables `pure`. Cargo
feature unification applies it to every BLAKE3 use in that target graph, using
Rust SIMD intrinsics rather than linked C/assembly implementations. The offline
boundary checks both the manifest declaration and the resolved Windows feature
tree. This is scoped to the security appliance target; ordinary client builds
do not lose their default BLAKE3 configuration when built separately.

## 3. Reproducibility record v2

The local `REPRODUCIBILITY.json` and external
`INDEPENDENT-BUILDER.json` now use format version 2. In addition to the
existing source, Rust/Cargo, target, path-remap and artifact evidence, each
record contains:

- mode `rust-toolchain-bundled-lld`;
- source `rustc-sysroot-target-bin`;
- file `rust-lld.exe`;
- flavor `lld-link`;
- exact lowercase linker SHA-256 and byte length;
- reproducibility flag `/Brepro`.

The record also fixes `blake3_codegen=pure-rust-intrinsics`, and both builders
run the offline dependency/code-generation gate before compilation.

The production verifier requires the local and GitHub records to identify the
same linker bytes before it compares the three executable copies or verifies
GitHub attestations. Its network-free self-test rejects both a mismatched
linker hash and a modified artifact.

Format-v1 records remain historical diagnostic evidence and are intentionally
not accepted for a new release package or independent-provenance claim. A clean
format-v2 record must be generated for the exact release commit.

## 4. Build and resource boundary

The two-clean-root local builder and manual-only GitHub workflow share the same
helper contract. The workflow remains `workflow_dispatch` only, uses two Cargo
jobs, pins every third-party action, does not package a ZIP and attests only an
exact hash match. Ordinary development still uses debug builds; the clean
release pair is run only when producing explicit reproducibility evidence.

M0.9.77 adds no runtime process, listener, protocol field, key material or
network dependency to Kilogram. A local smoke build confirmed that the real
`kilogram-offline` target links and runs with the bundled LLD, whose PE linker
field is 14.00 under the currently pinned Rust toolchain.

## 5. Completion boundary

Local implementation and fail-closed verification do not prove independent
cross-environment reproduction. That stronger claim requires:

1. committing and pushing the exact source revision;
2. producing the two-clean-root local format-v2 record for that clean commit;
3. manually dispatching the GitHub workflow with the exact commit and local
   artifact SHA-256;
4. obtaining an exact external match and GitHub attestations; and
5. running the production verifier against the downloaded evidence.

If the artifacts still differ while the linker SHA-256 matches, the remaining
mutable input is diagnosed separately, with the Windows SDK/import libraries
as the next candidate. No attestation is created on such a mismatch.
