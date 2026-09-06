# RFC-0070: Minimal offline publication-conflict appliance (M0.9.48)

Status: implemented in M0.9.48.

## 1. Problem

M0.9.47 made publication-conflict authorization explicit and confirmation
gated, but all three offline commands still lived in `kilogram-cli`. That
binary also contains network transports, HTTP publication, runtime IPC, local
state migration, messaging, synchronization and many unrelated authority
commands. Keeping an Account Root offline is less useful if the program allowed
to load it has a much larger command and dependency surface than the ceremony
requires.

M0.9.48 provides a purpose-built executable and a deterministic portable
package without changing any signed format.

## 2. Executable boundary

`kilogram-offline` exposes exactly three subcommands:

1. `inspect-request` authenticates a `.pcrq`, displays every security field and
   can create or exact-match a bounded QR claim;
2. `authorize` repeats public inspection and QR matching, checks the mandatory
   KPC1 code before resolving or loading Account Root, verifies the exact
   current requester/detector roster and writes one no-clobber `.pcrp`; and
3. `inspect-response` authenticates a `.pcrp` and can create or exact-match its
   distinct response QR.

There is no command to create, recover, export, mutate or display Root material.
Inspection never loads Root. Authorization calls `AccountRootState::load` only
after public validation and the operator confirmation gate. Request, response
and QR paths must be regular non-symlink bounded files; untrusted input and new
output must remain outside the Root directory.

## 3. Dependency boundary

The offline package deliberately has no normal dependency on:

- Iroh or `kilogram-transport-iroh`;
- Tokio or Reqwest;
- runtime IPC;
- application protocol or session crates; or
- runtime state and message stores.

`scripts/verify-kilogram-offline-boundary.ps1` evaluates the locked normal
dependency graph and fails if any denied package appears. At M0.9.48 the graph
contains 204 unique packages versus 480 for `kilogram-cli`. The remaining graph
includes identity/Account Root handling, device and prekey-directory
verification, ticket publication keys, Postcard/JSON, BLAKE3 and bounded image/
QR decoding. Those dependencies remain part of the offline trusted computing
base; the executable is smaller, not formally minimal.

The binary prints `network_surface_compiled=false` and
`runtime_state_loaded=false` as diagnostic assertions of the intended build and
execution boundary. The dependency gate, rather than those strings, is the
enforced build property.

## 4. Iroh-free connection-ticket verification

The signed v11 connection ticket is JSON and contains an Iroh endpoint. Pulling
the Iroh parser into the offline build would defeat the dependency split. The
offline verifier therefore deserializes the endpoint as `serde_json::RawValue`.
When reconstructing the Device-signature bytes, the exact compact endpoint JSON
is emitted unchanged while all other signed fields use their shared public
types and canonical struct order.

The raw endpoint is also parsed into a bounded public summary: a 32-byte hex
Endpoint ID, no more than 64 relay/IP addresses, bounded ASCII relay URLs and
valid socket addresses. The verifier additionally authenticates the listener
certificate, current complete prekey directory, exact certificate membership,
publication write key, route, requester Account and listener Device signature.

This is a compatibility adapter, not a second transport implementation. It
never dials, binds or resolves an address.

## 5. Wire compatibility

M0.9.48 does not change:

- connection ticket v11;
- `.pcrq` request v1;
- Root resolution v2 or `.pcrp` response v1;
- KPC1 confirmation code; or
- publication-conflict QR claim v1.

The offline library currently contains a compatibility decoder for the signed
observation, proof, request and response layouts. The Three-Device regression
is the release gate against drift: the online runtime creates the request,
both inspectors must report identical request ID/digest/KPC1, the offline
library authorizes it, both sides must report the same response IDs, and the
online runtime must apply it and continue sibling convergence.

This duplicated-format residual risk was removed in M0.9.49: online and offline
paths now share the network-free artifact implementation specified by
[`RFC-0071-shared-publication-conflict-artifacts.md`](RFC-0071-shared-publication-conflict-artifacts.md).

## 6. Portable package

`scripts/package-kilogram-offline.ps1`:

- checks the dependency boundary;
- rejects a dirty worktree unless explicitly producing a development-only
  package;
- runs `cargo build --locked --release -p kilogram-offline`;
- refuses to overwrite its output directory or ZIP;
- copies the stable `kilogram-offline.exe` name and operator instructions;
- records source revision, Rust/Cargo versions, target and boundary flags;
- writes SHA-256 checksums for every payload; and
- creates ZIP entries in a fixed order with the DOS epoch timestamp.

Two packages made from the same dirty development revision, toolchain and
binary produced byte-identical ZIP hashes during implementation. This proves
deterministic packaging of identical inputs, not a complete reproducible-build
claim. A pinned clean builder image, independent rebuild, signed provenance and
published source/toolchain hashes remain required for a public release.

M0.9.50 closes the single-local-binary release fallback: a clean package now
requires the verified two-clean-root record described by
[`RFC-0072-reproducible-offline-release-and-bounded-cargo-cache.md`](RFC-0072-reproducible-offline-release-and-bounded-cargo-cache.md).
That is a same-host reproducibility gate; a second independent builder and
signed provenance remain future release requirements.

## 7. Desktop integration

The Windows incident panel now renders `kilogram-offline inspect-request`,
`kilogram-offline authorize` and `kilogram-offline inspect-response` commands.
The live desktop/runtime still only creates the public request and applies the
public response. It never receives an Account Root path or secret.

The equivalent general-CLI commands remain temporarily available for backward
compatibility and development tests. They are not the recommended Root-host
surface.

## 8. Honest security boundary

A smaller binary does not make a general-purpose offline PC trustworthy.
Malware, a compromised OS, a malicious build, hostile removable media or a
deceptive display can still steal Root material or obtain the wrong signature.
KPC1 still detects exact substitution only when compared through an independent
trusted channel.

The package is not a bootable appliance, hardware wallet or sandbox. DPAPI ties
an existing Windows Root envelope to its user context, so copying only the Root
directory to another PC may not make it loadable. Hardware-backed Root keys,
read-only boot media, OS sandboxing and independently reproducible release
artifacts remain later hardening layers.
