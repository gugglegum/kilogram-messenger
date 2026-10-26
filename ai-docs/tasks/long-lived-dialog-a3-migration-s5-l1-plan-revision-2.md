# A3-S5.2 plan revision 2 — account scope, public Consent1 renewal and exact IPC3

2026-10-26. **Corrected proposal for root review; document work only.**
Baseline `73c499818ba9b699c331aa842e64e8432e90c59a`.

The [submitted plan](long-lived-dialog-a3-migration-s5-l1-plan.md) remains the
immutable review input: 38467 bytes, SHA256
`155387113ed699d4e1fa85e2314770dfccf1ff8f9271a748d7adf3ff027d23be`.
[Root review 1](long-lived-dialog-a3-migration-s5-l1-plan-review-1.md), 10004 bytes,
SHA256 `36379adada4d7d4ad0c2d2c239a8f7b19cc5f8d0280b1bbf8b482e9732cf21dc`,
approves the core mapping and requires the focused corrections below. The
[task](long-lived-dialog-a3-migration-s5-l1-task.md) and
[accepted contract](long-lived-dialog-a3-migration-s5-contract.md) are unchanged.

This revision replaces the submitted plan's authority row in section4, extends
section5 with the missing offline producer, replaces its IPC3 request table and
section9 preparation classification, and records root's origin/budget decisions.
Its other approved state/owner, Native phase/constructor, atomic write-set,
history/cursor, credit, scheduling and verification mappings continue to apply.
No contract, submitted plan, product/script, test evidence or ledger is rewritten.

## 1. R1 replacement — account membership determines the affected set

Replace the combined `PinAuthority1` / `RenewConsent1` row with:

| Closed purpose | Atomic policy effect and invalidation boundary |
|---|---|
| `PinAuthority1(account,raw)` | Verify the Root-signed account snapshot, canonical bytes and revision against that account's held pin. A strict advance commits the pin and relevant policy/high-water changes for **every registered C referencing that account**, whether own or peer, before another dispatch/final Native commit. Invalidate all those C's old leases, policy results and credential/admission cache entries. The set is computed from immutable account membership across occupied C, not only currently connected/enabled C. An unrelated C continues independently. |
| `RenewConsent1(C,expected_ref,raw)` | Verify the same C and exact original cert tuple, both Root signatures, current authority and interval; CAS only current Consent1. Commit that C's policy/high-water change and invalidate that C's leases/cache. No immutable Spec/initial pair, Native, history proof or block is replaced. |
| `PublishLocator1` / peer Locator1 import | Retain exact Descriptor1 scope/signature/revision/current-policy checks. Locator changes invalidate their C's affected admission state, not an unrelated C. |

An account shared by C1 and C2 is one account pin. A peer-account advance can
therefore invalidate both C just as an own-account advance can. For authority
import: equal revision with identical canonical bytes is an exact no-op; equal
revision with different bytes is Conflict; lower revision is rollback refusal.
Neither refusal may publish a partial pin/cache change. On uncertain IO use the
already-planned poison/reopen discipline before further work. The serialized owner
does not process another job between the durable advance and cache/lease removal;
final commit gates reject trials carrying an old policy generation.
Lease1 binds the C's policy token and its exact relevant account pins; a single
global generation change must not invalidate a genuinely unrelated C2 lease.

This matches [contract account scope](long-lived-dialog-a3-migration-s5-contract.md#L303)
and the existing affected-account filter in
[Engine::pin_authority](../../apps/kilogram-cli/src/ordered_dialog/engine.rs#L245).
Reuse the rule, not the V4 writer.

Inside **C-L1-admission/resources / F-L1-policy**, add two complementary assertions:

- C1/C2 share the same peer account. Advance its authority, then exercise revocation
  in the appropriate stopped branch. Both old leases/cache results become unusable;
  neither C dispatches or commits Native using the old account policy. Same-revision
  exact retry stays a no-op; conflict/rollback leave the current pin intact.
- C1/C2 have different peer accounts. A C1 peer advance/revocation does not disable
  independently permitted C2. Consent1 or Locator1 changes for C1 similarly do not
  invalidate unrelated C2. No extra whole suite is added.

## 2. R2 addition — public same-C offline renewal producer

### Existing primitives and the exact missing surface

[create_context](../../apps/kilogram-cli/src/ordered_dialog/admin.rs#L208)
chooses a new random C at line236; it is not the renewal producer. Consent2Create
uses another signature domain/protocol. The existing
[import_context existing-C branch](../../apps/kilogram-cli/src/ordered_dialog/admin.rs#L329)
preserves Spec when cert refs and membership match, bumps only metadata Head,
updates `consent-current` and the peer authority, and can import a fully signed
invitation without loading either Root. This is the frozen d9b receiver to use.

Add one small offline module `ordered_dialog/consent1_admin.rs` and two explicit
CLI commands in `ordered2_cli.rs`: `ordered consent1-renew-propose` and
`ordered consent1-renew-cosign`. They consume public files and exactly one local
`--account-dir` per invocation; they open no messenger DB and contact no runtime.
Their CLI `state_directory()` is None; dispatch must not prepare a messenger vault
or instantiate a legacy Engine for either offline command.
They reuse `Credentials::{decode,parse}`, `Spec::{validate,verify_consent,valid_now}`,
`ConversationMembershipSnapshot` verification and
[AccountRootState::sign_ordered_context_consent](../../crates/kilogram-identity/src/account.rs#L915).
No wire1/IPC2 codec or frozen d9b executable is changed, and no generic Root signer
is exposed over IPC. A is the original elected role0; B is the original role1.

The saved **completed** original wire1 invitation is the public scope input. It
contains original C/certs/membership and both old Root signatures. It may have
expired as permission; its signatures/scope must still verify. The operator pins
`--expect-context` and the expected account identities to the existing dialog.
An absent scope artifact is a missing input, not permission to use context-create
or generate a replacement C. No new onboarding/export ceremony is proposed here.

Both steps also take current public authority snapshots for A and B and signed
minimum-authority snapshots representing their known current pins. Existing
`account-snapshot --account-dir ... --snapshot-file ...` exports each Root's
current signed snapshot. The producer builds each canonical current credential
with `Credentials::new(original_certificate,current_authority)`: it needs no new
runtime credential-export API or manual bundle factory. Existing exported public
credentials may supply the identical certificate/snapshot bytes, but are not a
required extra ceremony. Original invitation credentials are historical scope
witnesses, not substitutes for the current authority inputs.

### Commands, inputs and outputs

The commands below describe the **new implementation interface**, not commands
executed during this correction. Variables designate already-selected public
artifacts/identities, fresh output paths and each party's own Root directory.

```powershell
# A's offline machine; only Root A is supplied.
& $currentCli ordered consent1-renew-propose `
    --existing-invitation-file $originalCompleteInvitation --expect-context $C `
    --expect-account $accountA --expect-peer-account $accountB `
    --authority-a-file $currentAuthorityA --authority-b-file $currentAuthorityB `
    --minimum-authority-a-file $knownPinA --minimum-authority-b-file $knownPinB `
    --account-dir $rootADir --not-before $notBefore --valid-for-seconds $validFor `
    --output-file $renewalProposal

# B's offline machine; only Root B is supplied, never Root A.
& $currentCli ordered consent1-renew-cosign `
    --existing-invitation-file $originalCompleteInvitation --expect-context $C `
    --expect-account $accountB --expect-peer-account $accountA `
    --authority-a-file $currentAuthorityA --authority-b-file $currentAuthorityB `
    --minimum-authority-a-file $knownPinA --minimum-authority-b-file $knownPinB `
    --proposal-file $renewalProposal --account-dir $rootBDir `
    --output-invitation-file $renewalCompleteInvitation --output-consent-file $renewalConsent
```

Propose emits the existing eight-field invitation shape, with the original C,
exact original membership bytes, current credentials with **unchanged certificate
bytes**, a newly timed body and A's real Root signature; B signature is None.
Cosign verifies that exact proposal and scope, adds B's real Root signature, and
emits a completed legacy-compatible invitation plus the raw Consent1 for tag11.
Cosign does not edit A's signed body, silently replace a credential/authority or
extend its interval. If a bound current artifact is stale, obtain fresh inputs
and produce a new proposal rather than patching signed material.

Using `b(x)` for byte strings and `a(...)` for arrays, the files are exactly:

```text
body1 = [1, b(C), b(certRefA), b(certRefB), 1, 1, not_before, expires]
proposal = [1, b(C), b(currentCredentialsA), b(currentCredentialsB),
            b(originalMembership), body1, b(sigA), []]
completed = [1, b(C), b(currentCredentialsA), b(currentCredentialsB),
             b(originalMembership), body1, b(sigA), [b(sigB)]]
consent1 = [body1, b(sigA), b(sigB)]
```

The body is canonical wire1 (encode cap4096), signed with the existing
`kg:od1:consent\0` Root domain. Consent<=4096 bytes, each credential<=16KiB,
membership<=32KiB, invitation/proposal<=96KiB; signatures/IDs keep existing fixed
sizes. Outputs are newly created public files, never overwritten inputs, Root
files or protected runtime state. An interrupted multi-output publication is
reported as incomplete; both independently verified artifacts must exist and
match before import. No runtime state changed, so remaining output can be recovered
without constructing Account/Session/Native or treating a partial file as success.

### Validation at both signing steps

1. Validate canonical invitation/version/arity, original joint consent signatures,
   original C, exact cert bytes/refs and role election, the Root-signed original
   membership bytes, membership C/owner A and exactly those two accounts. Compare
   the proposal's scope/certs/membership byte-for-byte with this original scope.
   The historical scope parser may use `Credentials::decode` and static verification;
   it must not turn old authority/expired consent into current admission.
2. Verify the current authority signatures, canonical bytes and account bindings,
   then construct/verify the two canonical current credential bundles from the
   original certificates and these snapshots. The certificates equal the original
   tuple byte-for-byte, not merely a reused Device ID or key; cosign compares the
   proposal's credential bytes with these exact derived bundles. Verify both
   certificates' current messaging authorization. No revoked Device, changed
   member, changed certificate or new C is accepted as renewal.
3. Each signed minimum-authority snapshot names the corresponding original account.
   Current credential authority revision must be at least this known pin; equal
   revision requires identical canonical bytes. Reject lower revisions or same-rev
   conflicts. Each signer also obtains its **own** current snapshot from its local
   AccountRootState and requires its bound own credential authority to match it
   exactly. Its Root account must be exactly its elected original role's account;
   A cannot sign B's slot, and B cannot substitute a different Root/peer.
4. Propose requires a checked positive interval (`expires>not_before`, u64 arithmetic
   without overflow; `valid_for_seconds`1..2592000, at most30 days). Cosign rechecks
   that interval and current admissibility, including strict `now<expires` and the
   existing not-before skew rule; it cannot make an expired proposal current by editing it.
   Both signatures cover the same exact canonical body. A body/signature mismatch,
   wrong Root, wrong role, tampered scope or completed proposal passed as unsigned
   fails before output. Completion verifies the same full Spec/Consent1 shape the
   legacy importer expects.

Offline files establish signed floors; they cannot discover an unseen peer Root
advance. Both importers and the subsequent handshake must recheck their **held
current pins/time**. If either pin advanced after signing, stale inputs are refused
or refreshed through normal monotone authority administration. No signed renewal
can roll back revocation, erase fork/quarantine or confer permission by historical
signature alone.

### Actual frozen d9b and indexed import paths

For the stopped d9b side, the following **existing** commands use the frozen d9b
EXE and its existing exclusive state lock. The two authority files are the exact
ones bound in the completed invitation's current credential bundles. Verify that
correspondence before passing the artifacts to the old commands.

```powershell
# $d9bCli is the accepted fixed frozen EXE; its role may be A or B.
# Stop its runtime and wait for the exact process to exit before these commands.
& $d9bCli ordered authority-update --state-dir $d9bState --snapshot-file $authorityA
& $d9bCli ordered authority-update --state-dir $d9bState --snapshot-file $authorityB
& $d9bCli ordered context-import --state-dir $d9bState `
    --invitation-file $renewalCompleteInvitation --expect-account $d9bPeerAccount
```

No `--account-dir` is supplied to this import: the completed invitation already
has both signatures. The public producer has run on the new CLI separately, not
by changing d9b. The existing authority-update checks lower revisions and
same-revision/different-byte conflict
([ordered_cli.rs](../../apps/kilogram-cli/src/ordered_cli.rs#L251)).
The legacy context importer itself checks peer revision>=current but does not
independently compare equal-revision bytes at admin.rs:363–373; do not attribute
that stronger check to it. The stopped public flow pins/checks the exact current
snapshots first, then imports the matching unchanged completed artifact under
exclusive, serial administration. If any pin/input differs or is stale, stop;
do not call context-import to replace the known pin.

For the running indexed side:

```powershell
& $currentCli ordered authority3-import --ipc-file $indexedIpc --snapshot-file $authorityA
& $currentCli ordered authority3-import --ipc-file $indexedIpc --snapshot-file $authorityB
& $currentCli ordered consent1-import --ipc-file $indexedIpc --slot $slot `
    --expected-current $currentConsentRef --consent-file $renewalConsent
```

`expected-current` is the held current Consent1 reference (or explicit `absent`
only for real absence), encoded as specified in section3. Its hash uses the
existing `wire::hash(handshake::CONSENT, raw_consent)` domain. Tag11 checks purpose,
original cert tuple, both Root signatures, current account pins/time and current-ref
CAS under Owner7, then changes only current consent/policy metadata. Bad CAS,
scope/signature/current permission fails without Native/history/credit changes.
Authority advances use the full R1 affected-account set. Renewing consent invalidates
only that C's lease; actual ALPN1 work requires a newly admitted current lease.

The frozen existing-C import may advance metadata Head and current consent/peer
pin. Neither side replaces original Spec/initial pair, selected handshake controls,
Account/Session/OTK/keys/D/E/SID, Native generations/counters or historical capture
tags. An updated current credential can later be captured as an actual admission/
finalization bundle; it does not rewrite any earlier O/A/F/R. No fork/quarantine
block is cleared. A revocation assertion ends in AuthBlocked, not renewed access.

## 3. IPC3 replacement — existing Value algebra, exact arity and bounded config

The existing
[wire Value codec](../../crates/kilogram-protocol/src/ordered_dialog_v1.rs#L30)
supports only unsigned integers, byte strings and arrays. `opt(None)=[]`;
`opt(Some(b(raw)))=[b(raw)]`. In the table, `optBytes` means **exactly**
`[] | [bytes]`; `bytes` is a CBOR byte string, not a text/JSON value. There is no
literal null, map, bool or text-string extension. CLI text is UTF8 encoded in a
byte string, as with existing `Value::text()`. The nested request remains carried
inside authenticated `RuntimeIpcCommand::OrderedDialog`.

| Operation / total tuple arity | Exact IPC3 request and restrictions |
|---|---|
| Status /2 | `[3,1]` |
| Queue /5 | `[3,2,slot,request16,text_utf8_bytes]`; exact16-byte request, valid UTF8 body<=4096 bytes, existing L1 queue/lifetime bounds |
| Request status /4 | `[3,3,slot,request16]` |
| Indexed history /5 | `[3,4,slot,optCursorBytes,limit]`; None=`[]`, Some=`[cursorBytes]`; actual existing authenticated cursor decode/cap, limit1..32 |
| Locator1 /6 | `[3,5,slot,1,action,optLocatorBytes]`; action0 import requires `[rawDescriptor1Bytes]` (<=8192), action1 export requires `[]`; other family/action/shape fails |
| Authority import /3 | `[3,6,rawAuthorityBytes]`; required byte string, existing signed snapshot cap/verification and R1 monotone account rules |
| Consent1 CAS /5 | `[3,11,slot,optExpectedRef32,rawConsent1Bytes]`; expected None=`[]`, Some=`[exact32Bytes]`; required consent bytes<=4096; no historical flag |
| Capacity CAS /6 | `[3,35,expectedRootGeneration,expectedConfigRevision,expectedConfigHash32,candidateConfigJsonBytes]`; required hash exactly32 bytes; required bounded config bytes below |

IPC3 request byte cap stays32KiB before tree allocation; field caps and exact
arity/option/cardinality checks apply before operations. None expected consent
means real absence, never wildcard CAS. Raw authority/consent/config are required
byte strings, not Some-wrapped fields. Wrong version/unknown tags, extra fields,
`[bytes,bytes]`, raw bytes in place of an option, non32-byte Some ref, literal null
or malformed/cross-C cursor fail closed. History's actual escaped JSON<=48KiB
check still occurs before candidate cursor consumption.

Tag35's `candidateConfigJsonBytes` is **UTF8 JSON bytes<=16KiB**, parsed once into
the existing deny-unknown-fields
[MigrationConfig](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/records.rs#L23),
using the same representation/cap as
[load_config](../../apps/kilogram-cli/src/ordered_dialog/storage_migration.rs#L49).
Its required fields are exactly `logical_quota:[u64,u64]`,
`physical_capacity:u64`, `output_profile`, `control_ipc_file` and
`locator_directory` (the latter three are path strings). Reject invalid UTF8/JSON,
duplicate/unknown/missing fields, extra JSON values, wrong types or overflow.
This is data inside a required byte string, not an extension of the wire Value
algebra and not a pathname to open as a config on behalf of IPC.

Owner/state perform the existing normalized absolute-path/length/owned-publication
checks, require the same stored public paths, no quota/capacity decrease and real
backend headroom, and use expected root generation/revision/hash CAS through the
held core. Derive canonical Effective/config hashing in state; do not treat raw
JSON whitespace/order as the old effective-config hash or accept caller totals.
No second mutable repository, implicit endowment/refill or source import is added.

Tag11 allocation is accepted by root review1. Tags7–10/Locator family2/handoff
remain UnsupportedInCut2. Tags32–34 remain MigrationAdmin-only. The explicit CLI3
surface in the submitted plan is otherwise retained, with the two offline producer
commands added. No public set-purpose/set-native/set-mode/set-Q endpoint appears.
Frozen wire1 and IPC2 bytes/parsers/dispatch remain unchanged.

## 4. Focused matrix disposition and root budget decisions

The submitted five C-groups and seven F-groups remain; add these assertions inside
the existing policy/capture groups, rather than a new broad suite:

| Group | Revision2 observable assertions |
|---|---|
| C-L1-admission/resources and F-L1-policy | Shared-peer two-C authority advance/revocation invalidates both old leases/cache results, including final commit gates. Different-peer C2 continues. Equal-rev exact/conflict/lower-rev behavior stays closed. |
| C-L1-admission and F-L1-policy | Execute the actual public propose -> cosign -> completed invitation/raw Consent1 flow, with each process given only its own Root. Import current snapshots, stopped frozen d9b existing-C invitation and indexed tag11 CAS; expiry refusal becomes current permission only after legitimate renewal and new lease. No raw test-only renewal factory substitutes for these commands. |
| Same policy group, focused negatives | Wrong C/cert tuple/membership/peer/Root/role/signature, malformed/expired proposal, revoked Device, stale/conflicting authority and failed current-ref CAS produce no unauthorized runtime transition. No new C, constructors or Native work during issuance/import; original Spec/initial pair/history/native keys and fork/quarantine remain exact. |
| Same C policy/IPC group | Canonical []/[bytes] option positives/negatives, unchanged IPC2, fixed arities/caps, Locator import Some/export None, and bounded tag35 config decode/CAS. These are typed boundary assertions, not a codec rewrite/corpus repeat. |

No new empirical feasibility run was made. The existing group estimates remain
planning envelopes, with the focused additional operations consuming the same
reserve. If implementation cost or measured remaining budget no longer fits,
callback root before more execution; no silent repeat/limit expansion.

Root review1 selects **one real A511/B16 Text exhausted origin**, with actual
opposite cut, one generation and stopped Native-domain preservation for later
cutA512/first Event2 n512. The duplex511/511/F2200 alternative in the submitted
plan is not selected. Retain small gates first, the before-generation remaining
budget check for seed upper850 + gate140 + contingency, and the900-second whole
seed-attempt stop including cleanup. Every failed/overrun attempt is retained and
charged before a root retry decision. No free seed setup or fabricated rows.

Root accepts these **planning envelopes, not execution permission yet**:

| Set | Prior immutable S5.1 actual | New S5.2 tranche after separate dispatch | Resulting cumulative maximum |
|---|---:|---:|---:|
| C |279.3257298 s|500 s|779.3257298 s|
| F |1236.2886628 s|1800 s|3036.2886628 s|
| P |0 s|0 s|0 spent|

F named ranges945–1575 plus225 contingency=1800 remain unchanged. Later
C350–550/F650–1100 are estimates only, with no allocation now. Whole final P
requires a separate root decision and one continuous run<=2400 seconds; S5.2
branches/prefix/suffix cannot be stitched into it. Old S5.1 evidence/ledger and
R4-only +120 amendment remain immutable, with no reset/borrowing old balances.

## 5. Replacement of section9 preparation classification and stop

Match [contract section11](long-lived-dialog-a3-migration-s5-contract.md#L1097):

| Work placement | Classification / accounting |
|---|---|
| Separately executed compile-only, field build/stage, Clippy or fmt | **Static-only, outside C/F.** Retain exact argv, elapsed, source/lock/features hashes and resulting stable image hashes separately. It does not earn runtime PASS or permit fixture creation. |
| Cached compile/build/stage/inventory within a contributing C/F runner | Charged once as part of that runner's complete elapsed, alongside setup, fixture generation, copies, waits, kill/recovery, logging and cleanup. Do not subtract it after execution. |
| Real seed/fixture generation or preparation, wherever requested | Must belong to the named contributing case with setup/overhead; never reclassify fixture work as static-only/free. Frozen-original preservation and one active branch/domain remain mandatory. |
| Reuse of the same already-recorded preparation | Bind exact source/image hashes; do not count the same elapsed twice. New work/attempts are recorded under their actual placement, never hidden by reuse. |

The submitted standalone compile-only example therefore belongs in a static-only
record, not the C/F ledger. Its illustrative new C/F runner calls remain proposed
interfaces; all compilation they perform internally is charged there. Preserve
locked/offline/verification/Jobs12/BelowNormal, compile --no-run, stable Windows
harnesses/field EXEs, protected frozen d9b/S3/S4/relay images and source/hash binding.
No direct network `cargo test`, live-image recopy, parallel Native-domain copies,
full source scan, full-history cache or repeated100k outside an affected branch.

R1/R2 and IPC/static accounting are resolved by this focused corrected proposal.
Only this new revision document is the executor's change. Submitted plan,
root task/navigation/review, accepted contract and evidence are preserved.
No product/scripts/builds/tests/DB opens/EXE staging/commit/push occurred.
Callback root with exact path/hash and disposition, then stop. Product execution
requires root's acceptance of this revision and separate implementation dispatch;
later reserves and whole P remain separately gated.
