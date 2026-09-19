# RFC-0101: Exact native link-input provenance (M0.9.78)

Status: implemented locally; a fresh GitHub-hosted run is required to compare
the independently captured manifest.

## 1. Problem

The M0.9.77 GitHub run for source revision
`4711dd337dce4ba82c19d50970dc29f75b6bd260` used the same Rust/Cargo versions,
the same 113,421,312-byte `rust-lld.exe` and the same bounded PE metadata
normalization as the local build, but remained fail closed. The local artifact
was 2,433,536 bytes at SHA-256
`25b24880f3f7f8ee34da48b4239b0dd730f5799e22275ef508640747cb9b6540`;
the GitHub artifact was 2,434,560 bytes at SHA-256
`3dd8e4b78e4326663393c126a3a1533dc1fdd7f9a5dcf45751e80034b96253ff`.
The 1,024-byte difference was structural: `.rdata` and later sections moved,
so further output normalization would be unsafe.

The remaining explanation was native MSVC/Windows SDK link inputs, but the
format-v3 evidence recorded only the linker executable. An environment version
or installed-SDK inventory is insufficient: it does not prove which library
files the final link actually consumed.

## 2. Exact capture

Both release builders now invoke the final binary through `cargo rustc` and add
LLD's `/reproduce:<archive>` argument only to the `kilogram-offline` binary
link. LLD writes its resolved inputs and response file into a reproduction TAR.
The builder then:

1. enumerates the archive with Windows `tar.exe`;
2. accepts only native `.lib` entries under a recognized Windows SDK or
   `VC/Tools/MSVC` layout;
3. extracts each selected entry into a fresh guarded temporary directory;
4. records its exact SHA-256, byte length and path-independent logical name;
5. validates the sorted manifest; and
6. removes both the extraction directory and the large reproduction TAR.

An unknown native-library layout, duplicate logical identity, empty library,
unsafe archive path, missing UM/UCRT/MSVC class, malformed manifest or retained
archive fails closed. Rust `.rlib` files and project objects remain covered by
the artifact/source/toolchain evidence; they are not duplicated into this
native-platform manifest.

The retained `NATIVE-LINK-INPUTS.sha256` format is one line per library:

```text
<lowercase-sha256>  <byte-length>  <logical-platform-path>
```

For example, logical paths distinguish
`windows-sdk/<version>/um/x64/kernel32.lib`,
`windows-sdk/<version>/ucrt/x64/ucrt.lib`, and
`msvc/<version>/lib/x64/msvcrt.lib` without exposing a username or absolute
host path.

## 3. Evidence format v4

`REPRODUCIBILITY.json` and `INDEPENDENT-BUILDER.json` move to format version 4.
Their `native_link_inputs` object binds:

- capture mode `lld-link-reproduce-archive`;
- manifest format `sha256-bytes-logical-path-v1`;
- stable file name `NATIVE-LINK-INPUTS.sha256`;
- exact manifest SHA-256 and entry count; and
- `archive_retained=false`.

The same-host builder captures the manifest independently for both clean roots
and refuses to continue unless the manifests match exactly. It retains one
validated manifest and includes it in `SHA256SUMS`. The GitHub workflow retains
and uploads the same bounded file, includes it in `SHA256SUMS`, and attests it
only when the executable itself matches.

The production verifier now requires local and external native manifests to be
valid and byte-identical before artifact or attestation acceptance. Its
network-free self-test rejects a substituted native-input identity separately
from linker and executable tampering.

## 4. Local observation

The first real LLD capture contained 175 archive entries and a 157,723,136-byte
temporary TAR. Only ten entries were native platform libraries. The bounded
manifest identifies:

- MSVC `14.44.35207`: `msvcrt.lib`, `vcruntime.lib`;
- Windows SDK `10.0.28000.0` UCRT: `ucrt.lib`;
- Windows SDK `10.0.28000.0` UM: `advapi32.lib`, `bcrypt.lib`, `dbghelp.lib`,
  `kernel32.lib`, `ntdll.lib`, `userenv.lib`, and `ws2_32.lib`.

The manifest SHA-256 was
`182c33c504ac2bce811459acd1a9f3fcd35fcb414be1b710b32651c9c794d61c`.
The probe archive and extracted copies were deleted after validation. Adding
`/reproduce` did not alter the normalized executable: it retained the M0.9.77
SHA-256 and byte length.

## 5. Scope and next boundary

M0.9.78 is a provenance/diagnostic stage, not a claim that Windows SDK and CRT
inputs are now hermetic. The next explicit GitHub dispatch will reveal the
exact external versions and hashes. If the manifests differ, the evidence will
identify every differing native file and the following stage can pin or bundle
the required redistributable/link inputs under an auditable license boundary.
If the manifests match but the executable still differs, investigation moves
to the remaining response-file arguments and Rust-produced archives rather
than weakening byte equality.

No runtime code, listener, protocol, key material, release ZIP or background
process is added by this stage.
