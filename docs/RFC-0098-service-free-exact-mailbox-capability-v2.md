# RFC-0098: Service-free exact mailbox capability v2 (M0.9.75)

Status: implemented and locally verified.

## 1. Problem

M0.9.68--M0.9.74 proved delivery through an exact Device-signed set of
volunteer stores without starting or contacting an HTTPS mailbox. The original
v1 capability nevertheless still serialized an HTTPS base URL and one expected
store key. In the no-HTTPS field harness those values were deliberately inert,
but their presence made a central mailbox service look like part of the new
architecture.

The replacement wire format must express only the authority actually used by
volunteer delivery. It must not silently reinterpret one volunteer as a
central service, and it must preserve a bounded migration path for existing v1
state.

## 2. Versioned capability

The v2 exact-volunteer local binding and recipient offer retain the existing
mailbox address, read/write capability material, exact Account/Device binding,
scope, creation time, signatures and HPKE recipient binding. They use distinct
v2 signature, HPKE and content-ID domains.

The v2 content:

- does not contain an HTTPS URL;
- does not contain a central store key or `MailboxServiceDescriptor`;
- is activated only by `ActivateExactVolunteer`, whose Device-signed update
  carries the canonical exact volunteer replica-set commitment;
- requires at least two active transport-distinct volunteer providers before
  local persistence.

The deterministic first key in the signed set can be used as a local ledger
anchor. That does not grant it special network authority, does not change the
two-receipt threshold and does not permit fallback to that provider as a
singleton service.

## 3. Compatibility and migration

Decoders retain bounded v1 local bindings, offers and capability actions.
Existing v1 chains can therefore be opened, delivered and revoked while an old
installation is migrated.

An acknowledged, active v1 head automatically rotates v1 -> v2 only when the
runtime knows at least two eligible providers. The new generation keeps the
usual predecessor link and overlap until the recipient-signed acknowledgement.
Migration queues no chat message and is restart-idempotent.

Chain validation permits v1 -> v2 and v2 -> v2, but rejects v2 -> v1. A v2
revocation retains the v2 signing/version domain. Old IPC create/rotate variants
remain available for explicit compatibility tooling; authenticated IPC v26 and
the Windows desktop default use the new exact variants with no URL or store-key
arguments.

## 4. Delivery semantics

For v2, both ordinary outbound messages and reverse acknowledgements resolve
the exact signed provider set. Success still requires the configured threshold
of store-signed, transport-distinct receipts. Recipient polling uses only that
set and preserves application-commit-before-delete.

If exact replication is incomplete, v2 does not attempt HTTPS fallback. The
item remains durably pending for a later provider retry. Likewise, v2 polling
never creates an HTTP client. Legacy v1 state may continue to use its retained
descriptor during the compatibility window.

## 5. User-visible and diagnostic boundary

Mailbox status reports `v1-legacy-https` or `v2-exact-volunteer` through the
secret-free IPC projection. The desktop no longer asks for a mailbox URL or
store key; activation and rotation request the exact volunteer format and fail
closed when the provider threshold is unavailable.

The offline CLI keeps explicit legacy commands and adds exact create/rotate
commands without service arguments. This is migration/debug surface, not a new
daemon.

## 6. Security boundary and non-goals

This change removes an inert central-service tuple from newly created exact
capabilities. It does not prove Sybil-resistant provider independence,
availability of volunteer nodes, anonymity from peers/relays, access-pattern
privacy or mobile background delivery. Provider offers still expire and must
be rediscovered through the existing bounded authenticated gossip path.

The stage adds no server and no new executable. It changes authenticated local
IPC to v26 so an old GUI fails closed instead of accidentally invoking the
legacy creation surface.
