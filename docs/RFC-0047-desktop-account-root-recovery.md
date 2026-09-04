# RFC-0047: Desktop Account Root recovery ceremony

Status: implemented in M0.9.20 (2026-09-04).

## 1. Goal and boundary

M0.9.20 exposes the M0.9.19 Account Root authority checkpoint in the Windows
desktop client. The GUI orchestrates the existing one-shot `kilogram-bootstrap`
helper; it does not implement a second Root writer and the messaging runtime
must be stopped for export, inspect, and restore.

This ceremony restores only Account Root authority. It does not enroll a local
device, copy device or vault keys, restore messages, revoke a lost device, or
prove that the chosen checkpoint is globally newest. After a successful restore
the user continues with the existing device-link ceremony and then one or more
independent history-recovery plans.

## 2. Export

The panel takes the current Account Root directory plus distinct new package
and witness paths. Both outputs must be outside the Root and must not already
exist. The helper holds the Account Root authority lock and publishes the
bounded Root-signed package and exact Root-signed witness with the atomicity
rules in RFC-0046.

The UI copies the resulting canonical paths into the inspect fields and shows
the Account ID, authority revision, package ID, and captured device/membership
counts. It explicitly tells the user to retain the latest witness independently
from the package. Every authority or membership mutation requires another
export and independent witness update.

M0.9.21 also exposes **Check recovery export status**. It compares a package
rebuilt from current Root state with the exact local receipt from the last
successful export and reports `current` or `update-required`. The status is
cleared when the Root path changes or desktop device enrollment mutates Root
authority. It is deliberately labelled as local lifecycle state, not global
freshness proof; RFC-0048 defines the stronger live-device protocol.

## 3. Inspect and exact-artifact gate

Package and witness can be selected through text fields or dedicated file-drop
targets. Direct symlinks, non-files, identical paths, relative helper output,
unknown JSON fields, oversized helper output, and dishonest freshness labels
are rejected. Inspect verifies the nested and outer signatures and exact witness
binding without reading the recovery phrase.

Editing or dropping either artifact invalidates the previous inspection,
clears the freshness confirmation, and wipes any phrase already entered. A new
inspect request also invalidates the old result before the helper starts, so a
failed request cannot leave a stale restore gate active.

Canonical paths alone do not close a same-path replacement race. Restore
therefore sends the inspected package ID and authority revision back to the
helper as explicit expectations. The helper recalculates and compares both
after reading the package and witness, before it creates the staging Root. A
replacement package at the same path is rejected even if it is otherwise valid.

## 4. Phrase and restore handling

Restore is enabled only after:

1. a successful inspection of the exact current artifact paths;
2. explicit confirmation that the independently retained witness is the user's
   newest known checkpoint;
3. entry of a syntactically valid 24-word phrase for the inspected Account ID;
4. selection of a new, non-existing destination Root directory.

The phrase is masked in the UI and held in zeroizing containers. It is redacted
from worker debug output, removed from the editable UI immediately after the
request is accepted, bounded to 4 KiB at the helper adapter, and written only to
the helper's standard input. It is never placed in process arguments or parsed
from helper output. Normal helper invocations receive a closed/null stdin.

This is memory hygiene, not isolation from a compromised desktop process. The
GUI toolkit, OS input method, crash dump, screen capture, or malware executing
as the user may still observe an entered phrase. Production hardening can add a
separate secure-entry process or hardware recovery factor, but must preserve the
one-shot Root writer boundary.

The helper performs the RFC-0046 staged reconstruction and publishes only a new
Root directory. On Windows its Root key is wrapped in a new DPAPI CurrentUser
envelope. The desktop then fills the restored Account ID and Root path into the
device-link panel; no enrollment or history action runs automatically.

## 5. Freshness claim

The accepted helper value is deliberately exact:

```text
exact-independent-witness-not-global-monotonic-service
```

The UI presents cryptographic validity separately from freshness. An old
package paired with the newest witness fails, but an attacker who replaces both
artifacts with an older matching pair can still cause rollback. The confirmation
checkbox records a human decision only; it is not a cryptographic proof. A
privacy-preserving monotonic witness or a quorum of current devices remains the
next protocol stage.

## 6. Verification

Regression coverage includes strict Account Root helper JSON validation,
redaction of the recovery phrase from worker diagnostics, exact inspected
package/revision binding, and rejection of a valid same-path package replacement
before a destination is created. A configured real-process test runs account
creation, export, inspect, stdin-only restore, and Windows DPAPI provider checks
through the same desktop adapters used by the GUI.
