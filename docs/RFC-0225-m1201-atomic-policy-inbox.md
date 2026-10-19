# RFC-0225: Atomic trusted-policy inbox and fail-closed quarantine

Status: M1.2.1, verified locally, dormant.

## Explicit local input only

M1.2.1 adds the first durable consumer of the rotation-aware policy bundle. It
accepts only a caller-supplied local file together with an explicit
`UserOrTestOperatorRequested` authorization. There is no URL, downloader,
background polling, remote administration or network dependency.

Before decoding, input must be a real regular file rather than a directory or
symlink, must be 1–8192 bytes, and must retain the same type and length across
the bounded read. The bundle is then verified from the explicitly pinned
offline trust root, including its full keyring, signer purpose, policy and
canonical encoding. Any failure is a quarantined diagnostic outcome: the
durable head is not changed and invalid input bytes are not copied into the
store.

## Atomic append and full bounded restore

The separate redb database pins the offline trust-root public key and retains
at most 32 immutable bundle records. A new record and its replacement head are
committed in one immediate-durability transaction. Exact replay is idempotent;
same-ID/different-bytes is rejected.

On every open and current-head read, all retained bundles are verified again,
ordered by policy epoch and checked as one exact chain from genesis to the
named tip. Missing records or head, gaps, forks, keyring skips, signature
failure, corruption and a non-tip head fail closed. The write transaction also
compares the head observed during planning, so two local writers cannot
silently overwrite one another.

A coordinated rollback of the complete database, including its pinned root,
history and matching head, still requires an external monotonic witness and is
not claimed detectable here.

## Verification and non-claims

Four store tests cover explicit authorization, atomic install, exact replay,
operational-key rotation, reopen, malformed input, non-file/oversized input,
foreign root and durable corruption. The full rollout crate passes 40/40 tests,
rustfmt and locked/offline strict Clippy.

The inbox still does not translate a policy into Device activation. It has no
runtime, desktop or CLI consumer and no production root. Peer wire, sync wire,
runtime IPC and public `legacy-only` behavior remain unchanged.
