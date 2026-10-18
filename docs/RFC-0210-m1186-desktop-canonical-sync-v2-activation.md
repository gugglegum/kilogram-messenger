# RFC-0210: M1.1.86 safe desktop canonical sync v2 activation

Status: accepted locally.

## Decision

The Windows desktop may inspect, plan and apply the existing Device-signed
canonical sync v2 activation authority, but only while its runtime is stopped
and disconnected. The GUI invokes the existing `kilogram-cli` authority
commands against the selected state directory. It does not copy a Device
signing key or Account Root secret into the GUI process and it does not create
a second activation authority.

The public default remains `legacy-only`. Opening the panel, refreshing status
or planning a transition cannot activate `kilogram/m1/sync/cbor/2`. An apply is
an explicit local, signed, append-only transition for one Device and takes
effect only after a fresh runtime start.

## Why activation is deliberately offline

A live runtime builds its Iroh endpoint and registered ALPN list at startup.
Persisting a different activation policy behind that endpoint would make the
durable state and in-memory transport disagree until restart. A new live IPC
mutation would therefore either pretend to hot-reload a capability that is not
registered or require a substantially larger endpoint-rebind protocol.

M1.1.86 avoids that split-brain boundary. Status, plan and apply are available
only when no desktop-owned runtime process exists and the GUI is disconnected.
The CLI then obtains the existing exclusive state lock, opens the existing
vault and performs the already verified authority ceremony. A concurrently
running external process causes the state lock to fail closed. Runtime IPC
stays at version 29 and receives no activation mutation.

## Exact ceremony

1. **Refresh verified policy** parses the complete helper output. Missing or
   malformed fields, non-canonical IDs, inconsistent implicit/signed state,
   an invalid high-water state or a rights matrix inconsistent with the
   selected policy is rejected.
2. **Plan exact transition** fixes actor `explicit-local`, target state,
   disposition, bounded reason and the currently verified evidence binding.
   The helper must report `activation_plan_writes_state=false`; the planned
   Device, predecessor and next epoch must match the displayed authority head.
3. **Apply exact signed successor** is disabled until the user types
   `APPLY ` followed by the first 12 lowercase hexadecimal characters of the
   candidate record ID. Any draft change clears both the plan and confirmation.
   Apply passes the complete candidate ID as `--expected-record-id`.
4. After apply, the desktop runs status again and accepts success only when the
   verified chain head equals the applied record ID. The cached plan is removed
   and the user is told to start a fresh runtime.

The reason is 1 through 256 UTF-8 bytes, has no leading/trailing whitespace and
contains no control characters. Emergency containment is available only for a
session-capable state with existing canonical evidence. Its displayed rights
must retain the registered reader and exact selected-session resume while
disabling publication and new accept/initiate.

## Fail-closed UI boundary

The desktop independently checks the six activation rights against the exact
state/disposition matrix instead of merely displaying helper booleans. It also
binds the plan to Device ID, current record, next epoch, target state, evidence
ID and reason. Stale status, changed draft, wrong confirmation, a different
applied record or a post-apply head mismatch cannot be acknowledged as success.

The panel is an advanced control rather than a convenience toggle. There is no
single-click canonical activation, no hidden environment override and no
automatic migration. The normal profile continues to start on Postcard v6 in
the absence of an explicit signed policy.

## Verification and non-claims

Desktop unit tests cover exact confirmation, draft invalidation, malformed
status, inconsistent rights, stale epoch and unbounded/control-character
reasons. A fresh CLI lifecycle smoke verified implicit `legacy-only`, no-write
plan, exact apply and a signed epoch-one `advertise-cbor-v2` head. The dedicated
boundary verifier includes negative source and manifest mutations. Readiness
contains 113 closed gates; rustfmt and locked/offline strict Clippy pass.

No network run, release build, archive, tag or automatic GitHub build is part
of this stage. This RFC does not enable a public dual-reader default, promise
hot policy reload, materialize canonical-native history, retire legacy sync or
replace an independent security audit.
