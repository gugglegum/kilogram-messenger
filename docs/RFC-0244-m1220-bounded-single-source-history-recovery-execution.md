# RFC-0244: Bounded single-source old-history recovery execution

Status: accepted locally for M1.2.20.

## Purpose and authority boundary

M1.2.20 performs the first real old-history recovery work after the separate
M1.2.19 consent commit. Execution can start only from the exact durable consent
record and its unique transition head. A re-enrollment approval, acceptance
receipt or consumed fresh-start transition is not history consent by itself.

Before both preparation and durable commit, the executor reloads the consent,
verifies its record/head and source signature, and re-verifies the complete
committed live context: consumed transition, current Account authority,
memberships, body-free inventory, selected range, source and replacement
Device. The original 15-minute expiry controls the first consent commit. An
already committed exact consent remains resumable after that expiry, provided
the immutable live context has not changed.

## Three deliberately separated operations

The source prepares at most 32 events by invoking the existing authenticated
same-Account `HistoryRewrapBundle` format. It does not invent a second wire or
encryption protocol. The bundle is signed by the source Device, binds the full
ordered inventory claim and encrypts every selected body for the replacement
Device.

The recipient separately opens and verifies the prepared bundle with its own
encryption key. Each entry becomes an existing history-rewrap
`LocalTextProjection`, which is still encrypted for that Device. The recipient
advances the existing signed recovery checkpoint only across the exact bundle
manifest. Source signing and recipient decryption therefore remain separate
capabilities even though the focused tests execute both local steps in one
process.

Finally, the source commits an exact execution record. It repeats the durable
consent and live-context checks and accepts only the expected checkpoint head.
This stage returns encrypted projections to a later import adapter; it does not
write them into the ordinary event or projection stores.

## Crash-safe append-only journal

`old-history-recovery-execution-v1.redb` is separate from consent,
re-enrollment and product history databases. Metadata binds it to the history
Account and source Device. One immediate-durability transaction inserts the
source-signed record, exact rewrap bundle, before/after recipient checkpoints,
encrypted projection artifact and the single head for one consent selection.

Each record binds the consent record and plan, transition, selection and
conversation, full inventory count/digest, approved range, generation,
predecessor record, chunk range, bundle ID/digest, both checkpoint IDs and the
projection digest. Records form one append-only linear chain. Startup verifies
every signature, canonical encoding, content ID, artifact digest, checkpoint
transition and head reachability. A fork, orphan, missing artifact, changed
byte or competing child fails closed; an existing database is never repaired
implicitly.

An injected failure before the atomic commit leaves no record, artifact or
head. Restart resumes from the last committed recipient-signed checkpoint.
Replaying an already committed checkpoint returns the exact durable chunk and
does not decrypt or advance it again. A differently randomized ciphertext
bundle or different checkpoint output for that already consumed input is
rejected as divergent.

The journal cannot detect restoration of its complete directory to an older
internally consistent snapshot. Detecting whole-database rollback requires an
external monotonic witness.

## Bounds and privacy-safe observation

A chunk contains at most 32 events. The database accepts at most 16,384 chunk
records; records are capped at 16 KiB, heads at 2 KiB, bundles and encrypted
projection artifacts at 4 MiB each, and checkpoints at 64 KiB.

The no-create read-only status contains only an opaque selection token, bounded
committed chunk count, next index and approved end. It exposes no body,
plaintext, raw Account/Device identity, path or network coordinate. Its
`network_authority` and `store_import_authority` are both false.

Plaintext exists transiently only inside the recipient verification operation
required to open the existing rewrap entry. It is neither returned by the
status API nor serialized into the execution journal. The journal stores only
source ciphertext, signed checkpoints and encrypted local projections.

## Deliberate exclusions

M1.2.20 is local/offline and single-source. It does not open a network endpoint,
send a bundle, choose among multiple sources, schedule retries, import events
into the production history store, add active IPC commands or add desktop
controls. Connection-ticket and sync wire formats, active IPC v33/domain v14
and the public `legacy-only` default are unchanged.

## Verification

Six stable-path tests cover atomic commit/reopen and privacy-safe status,
bounded three-chunk resume, exact replay and divergent-output rejection,
pre-commit rollback/retry, foreign identity/recipient/context drift, and
corruption/orphan rejection. On Windows Cargo only compiles the harness; the
project runner executes it under a stable EXE path.

The dedicated fail-closed boundary rejects ten negative mutations. Aggregate
accepted-tag readiness contains 146 closed gates with rustfmt and
workspace-wide locked/offline strict Clippy. No second host, product network,
release build, archive, Git tag or automatic GitHub build is used.
