# RFC-0102: Hash-locked Windows native toolchain (M0.9.79)

Status: implemented locally; a fresh exact-commit GitHub run is required.

## 1. Problem

M0.9.78 proved that identical source, Rust, Cargo and bundled LLD can still
produce different Windows executables when `rustc` discovers different MSVC
and Windows SDK import libraries. Selecting the newest installed SDK is mutable
and a version string alone does not identify file bytes.

An experiment also showed that setting `LIB` is insufficient: Rust's detected
native search paths still selected the newer SDK. The final package link needs
explicit, ordered native search paths plus proof of the files actually
consumed.

## 2. Repository lock

`WINDOWS-NATIVE-LINK-INPUTS.lock` contains the canonical SHA-256, byte length
and logical path for the exact ten-library set:

- MSVC `14.44.35207`: `msvcrt.lib`, `vcruntime.lib`;
- Windows SDK `10.0.19041.0` UCRT: `ucrt.lib`;
- Windows SDK `10.0.19041.0` UM: `advapi32.lib`, `bcrypt.lib`, `dbghelp.lib`,
  `kernel32.lib`, `ntdll.lib`, `userenv.lib`, `ws2_32.lib`.

SDK 10.0.19041.0 is selected instead of the local preview SDK 10.0.28000.0 so
the contract targets a non-preview SDK available on the development host and
expected on the pinned `windows-2022` hosted image. Availability is not trusted:
the builder checks every byte before compilation and fails before publishing
evidence if the exact set is absent.

The current official
[`windows-2022` image inventory](https://github.com/actions/runner-images/blob/main/images/windows/Windows2022-Readme.md)
lists Windows SDK 10.0.19041.0 and the Visual C++ x86/x64 tools component. That
inventory is only an availability hint; it does not replace the exact lock or
prove that the installed library payloads match.

The Microsoft libraries are not copied into Git, an artifact or a package.
Only their public cryptographic identities are retained; each builder must use
its own installed, licensed Visual Studio Build Tools and Windows SDK copy.

## 3. Explicit selection and post-link proof

`Get-KilogramPinnedNativeToolchainIdentity`:

1. validates an exact ten-entry, one-MSVC-version, one-SDK-version lock;
2. locates installed Visual Studio editions without depending on an edition
   name;
3. verifies the length and SHA-256 of every selected library;
4. returns three ordered search paths: MSVC x64, UCRT x64, UM x64; and
5. exposes no absolute path in retained evidence.

Only the final `kilogram-offline` invocation receives those paths through
explicit `rustc -L native=...` arguments. Dependencies remain covered by the
locked Rust/Cargo graph. After linking, the transient LLD `/reproduce` archive
is converted into `NATIVE-LINK-INPUTS.sha256`; that manifest must equal the
repository lock line for line. Thus path selection is an instruction, while
the post-link manifest is evidence that LLD obeyed it.

## 4. Evidence format v5

Local and external records add `native_toolchain` with:

- mode `repository-hash-locked-installed-libraries`;
- selection `explicit-final-rustc-native-search-paths`;
- lock file and canonical CRLF SHA-256;
- exact MSVC/SDK versions, architecture and entry count; and
- `libraries_bundled=false`.

The local two-root builder copies the lock into its bounded record. The GitHub
workflow uses the explicit `windows-2022` label, copies the same lock into its
bounded evidence and includes it as an attestation subject only after exact EXE
equality. The production verifier requires local lock, external lock and both
observed manifests to agree before checking the executable attestations.

The hosted image label is still mutable. Security therefore rests on the ten
file hashes, the pinned Rust/LLD hashes and the observed manifest, not on the
label or an installed-version inventory.

## 5. Failure and scope

Missing toolsets, unexpected library bytes, extra/missing logical libraries,
ignored search paths, mismatched locks and byte-divergent executables all fail
closed. No fallback to the newest SDK is allowed and no normalization is
expanded.

This stage adds no runtime code, protocol field, listener, network process,
release ZIP or long-lived build archive. The next acceptance step is a clean
format-v5 local record followed by a manual GitHub run for that exact commit.
