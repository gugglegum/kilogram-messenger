# RFC-0228: Rollout operator UX without hidden authority

Status: M1.2.4, verified locally, public rollout still held.

## One read-only view

`rollout-operator-status` joins four independently verified local views:

1. the existing Device-signed activation chain and high-water;
2. the root-verified trusted policy/keyring inbox;
3. the matching offline-root-anchored, rotation-aware rollout-health snapshot;
   and
4. the dormant controller decision.

The command opens rollout and trusted-policy databases through their read-only
APIs. A missing store remains absent and reports a fail-closed hold; observation
does not create a directory, database, identity or rollout seed. Output contains
only bounded state names, epochs, public artifact identifiers, health presence
and aggregate zero-tolerance count. It never prints the installation cohort
seed or bucket, private signing material, paths, contacts, message data or
network metadata.

The Windows desktop exposes the same result as a read-only advanced panel. It
is disabled unless all of `--rollout-dir`, `--trusted-policy-dir` and the public
`--release-trust-root` are supplied together. The panel deliberately has no
rollout toggle, enrollment action, approval button or recovery button. Missing
configuration is displayed as `legacy-only`, not silently synthesized.

## Explicit CLI ceremonies

Policy installation, approval and recovery are separate operations:

- `rollout-trusted-policy-import` requires `--confirm-import` and an explicit
  `--rollout-dir`, verifies the complete bounded file before an atomic inbox
  update, then restart-safely synchronizes health across the complete verified
  bundle chain; it grants no activation;
- `rollout-approval-plan` requires `--confirm-device-approval`, creates a new
  no-clobber Device-signed artifact and does not change activation state;
- `rollout-approval-apply` requires the exact printed approval ID, rechecks the
  current bundle, health, controller decision and activation predecessor, then
  persists only the independently Device-signed activation candidate and its
  high-water;
- containment recovery uses separate `rollout-recovery-plan` and
  `rollout-recovery-apply` commands, an independent exact recovery artifact and
  exact recovery ID.

The release root and operational release keys remain verification/proposal
authority only. Neither the desktop nor a release policy can manufacture the
Device signatures. Plan and apply cannot be collapsed into a single toggle or
network-triggered operation. Exact already-applied artifacts are the only
idempotent replay; altered IDs, bytes, Device, policy, health or heads fail
closed.

## Compatibility and non-claims

Ordinary startup has none of the three rollout-observer arguments and behaves
exactly as before. There is no downloader, background poller, remote admin,
automatic cohort enrollment, runtime IPC extension or peer/sync wire change.
The public default remains `legacy-only`, and accept-canary remains forbidden
until a separate decision and independent security review.

M1.2.4 verifies the local operator surface and read-only desktop parser. It
does not provide a production trust root, distribute a policy, activate any
real user, or claim that a coordinated rollback of every local database is
detectable without an external witness.
