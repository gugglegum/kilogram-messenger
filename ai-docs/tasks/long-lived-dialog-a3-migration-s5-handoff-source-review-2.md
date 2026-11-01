# S5.3 controls/Native source clearance and initial execution

2026-10-27. Root closes R1 from
[source review1](long-lived-dialog-a3-migration-s5-handoff-source-review-1.md)
after independent focused protocol re-review. This clears the first component
slice for the exact executions below; it is not final S5.3 acceptance or a
runtime PASS.

## Accepted source snapshot

- Baseline: `64c2359788ead360314793e47741adacb83b8d4f`.
- [Readiness revision1](long-lived-dialog-a3-migration-s5-handoff-readiness-revision-1.md):
  10335 bytes, SHA256
  `4688e8db3346506ca98365e49ef254d3c56b6987d20e8fa27148c86eee6c19c6`.
- [Binding r4](long-lived-dialog-a3-migration-s5-handoff-evidence/S5.3-controls-native-binding-r4.json):
  183223 bytes, SHA256
  `fd472368efc7839055d03df2fc75f328161720810912e587c3ff52d6d1685a72`.
- Case adapter:16757 bytes, SHA256
  `7ccff375e5bbcb8ff662f29393ebe0b612c53f26da97b58c365dc8ada81b5928`.
- Library harness:44699136 bytes, SHA256
  `e095470e5f473139a78de1968a9c5300a12c433e5d73fe4c60fd51c423692d1f`.

Root independently verified786 input rows,7 images and67 static evidence rows,
all with zero hash/size mismatches. Only the four reported existing inputs
changed from r3. The four final revision static records have identical766
before/after source rows matching r4, exit0 and no finalization errors. Root
also verified all10 archived r3 source/image copies against their manifest.
Index empty, no live Kilogram process, tracked whitespace check passed.

The verifier now requires exact signed author_auth/credential-reference equality.
Conversion resolves and Root-verifies each row's historical O from the catalog
before scope verification, fold or conversion; initial/current credentials are
not substitutes. The genuine renewal extension retains the original Device and
session, creates/delivers one actual Text1 under the updated authority, covers
both historical O positives and same-certificate cross-substitution negatives,
and preserves old/new raw history through conversion/reopen. Root and the
independent reviewer found no remaining R1 blocker. Scoped source hashes were
stable through review. Other first-slice review findings/limits remain unchanged.

## Activate only the initial C180 tranche

The separate root activation JSON and initial ledger authorize these two cases
once each, sequentially, in this order:

1. C-s53-native-01: selector `s53_native_core`; exact expected name
   `ordered_dialog::runtime7::handoff_tests::s53_native_core`; count1;
   work75 seconds, whole90 seconds.
2. C-s53-controls-01: selector `s53_controls_core`; exact expected name
   `ordered_dialog::runtime7::handoff_tests::s53_controls_core`; count1;
   work75 seconds, whole90 seconds.

Native runs first to exercise the corrected historical-author path and the
single-domain conversion before the larger controls exercise. This does not
add a timing probe or another case. Execute the second case only after the
first whole runner succeeds with its complete verified result/ledger and
cleanup. Timing feasibility remains unmeasured; failure is not permission to
extend a deadline, retry, run the other case or continue integration.

Use only `scripts/invoke-migration-s5-handoff-case.ps1 -Case <exact case>
-ExecutionMode NativeToken`, through the standing human authorization for the
necessary local native-token execution. The adapter invokes the stable Windows
runner, preserves fixed EXE paths and Jobs12/BelowNormal/offline policy, and
charges fixture creation, cached compile/staging, waits, finalization and cleanup
inside the case. No extra product invocation or harness discovery is authorized.

Retain actual durable pre-run inputs, original logs, result-before-tail snapshot,
complete whole exit/deadline/owned-child evidence and numeric timing limits.
Record the actual outer tool/command exit as well as the adapter result. The
snapshot alone cannot establish whole PASS. On any failure, timeout, hash drift,
incomplete cleanup or tool rejection, preserve evidence and callback root without
retry or source/image mutation. Ordinary automatic approval review still applies;
there is no authorization to bypass it or change system settings.

The active allowance is now C180/F0/P0, inside the unchanged allocation
C360/F1200/P0. The other C180, every F case and S5.4/P remain inactive. Historical
charges remain C587.4486109/F4716.3201489/P0; new actual remains zero until these
cases run. The private exhausted checkpoint/backup remains out of scope.

Source/images stay frozen. After both whole successes, callback with exact
records/logs/snapshots/ledger identities and outer exits, then stop for root
evidence review and release of the next integration work. No stage, commit or
push; final full S5.3 acceptance and exact publication assignment remain later.
