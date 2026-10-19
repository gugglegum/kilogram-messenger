# RFC-0220: Read-only rollout-controller observer

Status: accepted locally for M1.1.96.

## Purpose

The dormant rollout controller can now be inspected through the existing
`kilogram-cli` binary. The command is deliberately an offline observer rather
than a rollout control plane. It combines a verified existing rollout store
with the verified Device activation chain and prints either a hold reason or
an advisory proposal.

The caller must supply the exact pinned Ed25519 release-policy public key.
Kilogram still embeds no production release key and has no policy downloader.
The command does not accept a private release key, sign a release policy,
record health, approve a proposal or apply Device activation.

## No-create, read-only store inspection

`inspect_existing_rollout_store` first examines the requested directory and
database without creating either. A missing directory or database becomes the
normal `missing-signed-policy` hold. A symlink, partial schema, foreign pinned
authority, invalid signed policy, missing current health record, bad MAC,
oversized history or policy/health mismatch fails closed.

The database is opened through redb's read-only API. Every retained bounded
health record is decoded and MAC-verified before the current policy and health
snapshot are returned. Observation therefore cannot accidentally repair,
initialize or advance rollout state.

## Device-side join

The command independently verifies the existing Device-signed canonical sync
activation authority. It maps the four bounded dark-launch states into the
dormant controller and refuses `canonical-required`, which remains outside the
public rollout model. A proposal is still only a proposal: output always says
`rollout_runtime_activation_authorized=false`, and the existing explicit
Device plan/apply ceremony remains mandatory.

No runtime IPC variant is added. A running runtime does not consume this
decision, hot apply remains impossible, and a normal client does not invoke
the command automatically.

## Privacy and output

Output may show the common signed policy ID and epoch, phase, local selection
boolean, aggregate zero-tolerance count, current activation state and proposed
next state. It never shows the installation seed or cohort bucket, messages,
history, contacts, conversations, addresses, relay URLs, secrets or local
paths. There is no uploader and no automatic health export.

## Acceptance and non-claims

Twenty crate tests cover the controller, local authority and read-only path,
including absent-store no-create behavior, foreign authority and corrupted
health. A stable-path CLI smoke confirms that an absent rollout directory
produces a hold and remains absent. Strict Clippy and formatting pass.

M1.1.96 does not provide production policy delivery, remote administration,
anonymous attestation, automatic enrollment or public canonical rollout. Wire,
runtime IPC and the public `legacy-only` default are unchanged.
