# RFC-0100: Toolchain-bundled LLD independent reproduction (M0.9.77)

Status: implemented and fixture-verified; a clean format-v3 release pair and a
matching GitHub-hosted build still require the explicit post-commit gates.

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

The first clean pair after selecting LLD had equal length and differed in only
20 bytes: the COFF timestamp, two debug-directory timestamps and eight bytes
of the LLD-generated CodeView GUID. Code, PE layout, imports, RVAs and payload
data were otherwise byte-identical. LLD derives those fields from PDB inputs
that contain host-local build paths, so `/Brepro` alone does not make that
identity portable across clean roots.

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

## 3. Bounded PE metadata normalization

Both builders copy the linked executable and then run the same parser-backed
normalization function. It validates DOS, PE32+, optional-header, section-table
and debug-directory bounds before changing bytes. It refuses a non-zero PE
checksum or an existing Authenticode certificate, so normalization can occur
only on the unsigned pre-release artifact. Version 1 of the policy:

- zeroes the four-byte COFF timestamp;
- zeroes the timestamp of every parsed `IMAGE_DEBUG_DIRECTORY` entry; and
- for a valid CodeView `RSDS` entry, zeroes only its 16-byte GUID.

The pass does not resize the file, move a section, rewrite code, imports, RVAs,
the PDB age or embedded PDB filename, or ignore any remaining difference. An
independent assertion reparses every output and rejects a non-zero normalized
field. Exact size and SHA-256 comparison then remains fail closed for every
other byte.

This differs from normalizing the original M0.9.59 binaries, whose linker
versions produced different PE layouts and a 1,024-byte size difference. Such
broad normalization remains forbidden. The v1 pass is justified only after
the controlled LLD made the residual difference finite, understood and
structurally bounded.

## 4. Reproducibility record v3

The local `REPRODUCIBILITY.json` and external
`INDEPENDENT-BUILDER.json` now use format version 3. In addition to the
existing source, Rust/Cargo, target, path-remap and artifact evidence, each
record contains:

- mode `rust-toolchain-bundled-lld`;
- source `rustc-sysroot-target-bin`;
- file `rust-lld.exe`;
- flavor `lld-link`;
- exact lowercase linker SHA-256 and byte length;
- reproducibility flag `/Brepro`.

The record also fixes `blake3_codegen=pure-rust-intrinsics` and
`pe_metadata_normalization=coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1`.
Both builders run the offline dependency/code-generation gate before
compilation and assert the normalized PE boundary before hashing.

The production verifier requires the local and GitHub records to identify the
same linker bytes before it compares the three executable copies or verifies
GitHub attestations. Its network-free self-test rejects both a mismatched
linker hash and a modified artifact.

Format-v1/v2 records remain historical diagnostic evidence and are
intentionally not accepted for a new release package or
independent-provenance claim. A clean format-v3 record must be generated for
the exact release commit.

## 5. Build and resource boundary

The two-clean-root local builder and manual-only GitHub workflow share the same
helper contract. The workflow remains `workflow_dispatch` only, uses two Cargo
jobs, pins every third-party action, does not package a ZIP and attests only an
exact hash match. Ordinary development still uses debug builds; the clean
release pair is run only when producing explicit reproducibility evidence.

M0.9.77 adds no runtime process, listener, protocol field, key material or
network dependency to Kilogram. A local smoke build confirmed that the real
`kilogram-offline` target links and runs with the bundled LLD, whose PE linker
field is 14.00 under the currently pinned Rust toolchain.

## 6. Completion boundary

Local implementation and fail-closed verification do not prove independent
cross-environment reproduction. That stronger claim requires:

1. committing and pushing the exact source revision;
2. producing the two-clean-root local format-v3 record for that clean commit;
3. manually dispatching the GitHub workflow with the exact commit and local
   artifact SHA-256;
4. obtaining an exact external match and GitHub attestations; and
5. running the production verifier against the downloaded evidence.

If the normalized artifacts still differ while the linker SHA-256 matches, the
remaining mutable input is diagnosed separately, with Windows SDK/import
libraries as the next candidate. No attestation is created on such a mismatch.
