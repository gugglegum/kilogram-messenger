# RFC-0233: Read-only runtime quarantine observability

Status: accepted for M1.2.9.

## Purpose

M1.2.8 enforces the durable peer capability floor before background message
materialization and network use. A user must also be able to understand why a
contact is not progressing without granting the desktop or the observer any
ability to erase the evidence, retry work, or weaken the floor.

M1.2.9 therefore adds one bounded read-only runtime IPC page and a matching
Windows desktop panel. It deliberately distinguishes three states that would
otherwise look like one generic delivery failure:

- durable capability quarantine, reconstructed from the Device-bound store;
- process-local scheduler suppression, which prevents a busy loop only until
  restart;
- ordinary network retry, which is not a capability-policy refusal.

The connection-ticket wire and sync wire are unchanged. The public default
remains `legacy-only`.

## Read-only store boundary

`PeerCapabilityReadStore` can open only an already existing database. It uses
the database read-only API, validates metadata, owner binding, retained chain
and current records, and never creates a directory or database. A missing store
is represented as an initialized=false empty observation rather than being
materialized as state. Corruption or an owner mismatch fails closed.

Tests compare the database bytes before and after observation and verify that
an absent path remains absent. The existing writer remains the sole path that
can append a capability record.

## Bounded and private projection

The IPC command uses an exclusive opaque-contact cursor and a caller-selected
limit from 1 through 64. The underlying runtime collections remain bounded to
256 contacts and 4096 queued items. Rows are strictly ordered, cursor progress
is validated, and aggregate counts describe the complete bounded snapshot.

A row contains an opaque contact identifier, bounded quarantine reasons and
counts/booleans for durable quarantine, scheduler suppression and network
retry. It does not contain Account or Device IDs, queue IDs, ticket or endpoint
material, addresses, relay URLs, message/ciphertext data, filesystem paths, or
secret/capability bytes.

The IPC hard-cuts co-installed binaries to version 31 and descriptor signature
domain v12. There is no permissive v30 reader.

## Restart semantics

After restart, durable quarantine is loaded again from the verified store.
Process-local scheduler suppression is intentionally absent until the runtime
re-evaluates work; it is not durable authority. Ordinary retry remains a
separate queue property. This distinction prevents the UI from implying that
restarting repaired a capability violation.

## Desktop projection

The advanced Windows panel is explicitly labelled read only. It displays the
three classes separately and refreshes through authenticated runtime IPC, both
manually and after the existing runtime-status refresh. It offers no clear,
retry, downgrade, re-enrollment, or recovery control.

## Verification

The stable-path Windows harness covers missing-store/no-create, read-only byte
preservation and owner rejection, restart classification, multiple rows,
pagination, bounds, privacy-safe serialization, IPC version rejection and the
authenticated desktop request/response path. A fail-closed boundary verifier
also mutates the manifest and source expectations to prove that unbounded
pages, recovery authority, raw identifiers, a creating store open, or a public
default change are rejected.

Aggregate readiness passes 135/135 together with rustfmt and workspace-wide
locked/offline strict Clippy. No release build, ZIP, tag, automatic GitHub
build, second-host run or network test is part of this local IPC milestone.

## Scope and non-claims

This observer cannot clear quarantine, initiate retry, lower a capability
floor, import a new invitation, or authorize recovery. Designing an explicit
Device-authorized peer re-enrollment ceremony is a separate later milestone.
Generic first-contact extension stripping and coordinated whole-store rollback
without an external witness remain non-claims. This is not an independent
security review and does not authorize public canonical rollout.
