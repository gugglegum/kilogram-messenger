# RFC-0115: Windows extended-length runtime journal (M0.9.92)

Status: implemented locally; hosted confirmation and fresh field evidence pending.

## 1. Incident

The second hosted `Windows CI` run (`38513111049`) proved that the aggregate
CLI harness and the first isolated endpoint-lifecycle test pass. The remaining
pairwise own-device discovery test failed in a fresh process.

Its first actionable error was not the later HTTP 404. Windows rejected the
local append-only publication journal path with `os error 3` before the first
device could upload its locator. The opposite device then correctly observed a
missing opaque publication and returned HTTP 404.

The file name deliberately contains a 64-hex channel, a fixed-width generation,
a 64-hex publication identity and a descriptive suffix. Combined with the
state root and runtime namespace, this can exceed the legacy 260-character
Windows path limit even though every component is valid.

## 2. Storage fix

`persist_runtime_record` now:

1. creates the requested parent directory;
2. canonicalizes that existing parent;
3. appends the not-yet-existing final file name to the canonical parent; and
4. performs existence checks, temporary-file creation and the no-clobber
   persist operation through that resolved path.

On Windows, canonicalization supplies the extended-length path form required
by filesystem APIs. The fix does not shorten content-addressed names, weaken
append-only collision checks, depend on the machine-wide long-path policy, or
permit overwriting an existing different record.

A Windows-only regression creates a runtime record whose requested path is
longer than 260 UTF-16 code units, verifies the initial insert, verifies an
identical idempotent write, and reads the exact bytes back.

## 3. CI behavior

The pairwise locator test returns to the aggregate CLI harness because its
failure had a deterministic storage cause. Only the genuinely process-history
sensitive revocation/restart test remains in the exact fresh-process manifest.

Rust tests are also allowed to run after a failed readiness step when and only
when the locked dependency fetch succeeded. This is not `continue-on-error`:
the readiness failure still fails the job. It permits a protected-product
change to receive hosted test feedback while the intentionally stale candidate
evidence is being refreshed.

## 4. Evidence consequence

This change touches `apps/kilogram-cli`, which belongs to the field-tested
runtime surface. The existing M0.9.89 candidate must therefore fail closed and
must not be tagged as M1. A green hosted Rust run establishes regression
coverage, but fresh physical field evidence and a refreshed candidate record
are still required. The independently reproduced offline-artifact surface is
unchanged because it does not include `apps/kilogram-cli`.

No release build, archive, attestation or Git tag is created by this stage.
