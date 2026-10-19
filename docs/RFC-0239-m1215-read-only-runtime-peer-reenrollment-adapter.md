# RFC-0239: M1.2.15 read-only runtime peer re-enrollment adapter

Status: verified implementation boundary. This RFC does not authorize public
rollout or a peer re-enrollment mutation through IPC.

## Purpose

M1.2.14 defined a five-stage canonical peer re-enrollment language but kept it
dormant. M1.2.15 connects only `inspect` and `status` to the foreground runtime.
The adapter lets CLI and desktop explain why a quarantined contact is not yet
safe to re-enroll without handing either observer the invitation ticket,
endpoint material, secrets, raw peer identity, filesystem paths, or mutation
authority.

The active Postcard IPC makes an exact co-installed-binary hard cut from version
31 / descriptor signature domain v12 to version 32 / domain v13. There is no
dual reader or permissive fallback. A stale descriptor or request is rejected
before actor dispatch.

## Runtime request and response

`PeerReenrollmentRead` carries the already bounded M1.2.14 nested request. The
nested request is canonical Postcard, at most 4096 bytes, binds the exact signed
runtime descriptor and has a fresh nonzero request ID. The active adapter accepts
only `inspect` and `status`; nested `plan`, `approve`, and `accept` fail before
any store can be changed.

The result carries at most 16384 bytes of current-Device-signed response. The
client independently verifies the exact request digest, descriptor binding,
current descriptor Device, canonical encoding, state invariants, and signature.
Descriptor replacement therefore invalidates both request and response rather
than silently retargeting the ceremony.

## Invitation staging

Invitation staging is a separate explicit offline command. It verifies the
current Device and Account, the exact local authority list, the enrolled
contact, the replacement invitation and embedded ticket, and the already
durable predecessor quarantine. A successful import writes one bounded,
content-addressed, current-Device-signed record owned by the local Account and
Device. Exact replay is idempotent; divergent reuse, unexpected files,
symlinks, oversized data, corruption, or foreign ownership fail closed.

Staging is not acceptance. It cannot clear quarantine, create a tombstone,
commit a replacement, hot-apply a transition, start recovery, or contact the
peer. Desktop never reads an invitation file and never receives ticket or
endpoint bytes. The runtime observer receives only an opaque staging identifier,
not a filesystem path.

## Read-only projection

Under the runtime state lock, the adapter opens the capability quarantine,
acceptance receipt, runtime transition, and invitation staging stores through
their read-only APIs. An absent store is an honest empty or missing state and is
never created or repaired by observation. Corruption, an unexpected entry,
foreign Account or Device ownership, conflicting artifacts, replay, expiry,
authority regression, or inconsistent acceptance/transition state fails closed.

The response distinguishes:

- durable predecessor quarantine from this-process scheduler suppression;
- missing, stale, unsafe, or ready invitation state;
- durable acceptance requiring a fresh runtime restart from a consumed
  transition after restart;
- future communication recovery from old readable-history recovery.

Old readable history is not implicitly granted to a replacement Device. It
remains a separate same-Account rewrap ceremony with separate approval.

## Privacy and desktop authority

The public projection contains only opaque contact and artifact identifiers,
bounded invitation metadata, a bounded quarantine reason, durable outcome,
process-local suppression counts, restart requirement, and the separate history
recovery state. It contains no raw Account ID, raw Device ID, queue ID, message
body, ticket, endpoint, IP address, relay URL, path, private key, reusable
capability, or decrypted history.

The desktop loads and verifies the exact authenticated v32 descriptor, creates
one nested status request, verifies the signed response, and shows a clearly
read-only panel. There are no plan, approve, accept, clear, retry, downgrade,
hot-apply, or automatic-recovery controls. The CLI observer has the same
boundary.

## Verification and non-claims

The implementation checks absent-store/no-create behavior, foreign owner,
corrupt, oversized and unexpected artifacts, stale invitation, descriptor
substitution, restart classification, durable-versus-process-local state, and
authenticated desktop IPC. Windows Rust harnesses use stable executable paths.
This stage performs no network operation and needs no second host.

The public sync default remains `legacy-only`; connection-ticket and sync wire
formats are unchanged. No release build, ZIP, tag, automatic GitHub build,
quarantine clearing, automatic peer re-enrollment, implicit history recovery,
or independent security-review claim is made. Aggregate readiness contains 141
closed gates.
