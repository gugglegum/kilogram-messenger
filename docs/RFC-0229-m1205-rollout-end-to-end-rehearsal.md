# RFC-0229: M1.2.5 rollout end-to-end rehearsal

Status: locally verified; exact two-host Windows evidence pending.

## Complete dormant control-plane chain

M1.2.5 closes the first executable rehearsal of the rollout control plane from
an explicitly supplied offline release trust root through operational-key
rotation, atomic trusted-policy import, installation-local health evaluation,
controller proposal, explicit Device approval, emergency containment and
explicit Device recovery. The rehearsal uses disposable test-only state and
does not register canonical sync v2 as a public transport.

The trusted health store no longer treats one operational release key as a
permanent local database authority. It is anchored by the offline root and is
synchronized from the complete verified trusted-policy bundle chain. An
already installed prefix is checked byte-for-byte and only missing successors
are appended. A foreign root, fork, gap, truncation or a current health head
that is absent from the supplied chain fails closed. Operational signing-key
rotation therefore cannot strand otherwise valid installation-local health.

## Restart-safe Device apply

Approval and recovery remain two-step Device ceremonies. Before applying an
artifact, the CLI decodes its exact bytes and verifies the current trusted
bundle, health, controller result, Device signature, activation predecessor
and monotonic high-water expectation. If a crash happened after the signed
activation record was durably appended but before its high-water was
published, the same exact artifact may repair only that missing high-water.
No altered artifact, Device, predecessor, policy, health record or candidate
bytes can use the repair path. A completed replay is idempotent.

## Rehearsal and negative matrix

The canonical in-process rehearsal proves one five-bundle chain covering
genesis, advertise, accept, containment and recovery. It checks operational
key rotation, three explicit Device-approved transitions, containment,
explicit recovery and replay idempotence. Negative cases cover a foreign root,
stale input, truncated chain and fork; a symlink input is rejected on hosts
that expose the required Windows filesystem primitive.

The two-host harness transfers the exact same stable-name debug/verification
binaries to a disposable Windows laptop, compares SHA-256, runs the complete
rollout suite and the named end-to-end test on both hosts, and proves that both
read-only observers hold without creating missing state. Durable working state
and raw output stay under ignored `.tmp`; the tracked evidence directory may
contain only bounded JSON with revision, hashes, counts and anonymous host
labels.

## Security boundary and non-claims

All rollout behavior remains dormant. There is no background downloader,
remote administrator, network-triggered apply, automatic enrollment or one
click desktop activation. The public default remains `legacy-only`; release
and root keys remain proposal/verification authority and cannot replace a
Device signature.

This rehearsal is not an independent security review and does not authorize
accept-canary. First-contact capability stripping remains unresolved. A
coordinated rollback of every local rollout database is not detectable without
an external witness. No production trust root, release archive, tag or release
build is created by this milestone.
