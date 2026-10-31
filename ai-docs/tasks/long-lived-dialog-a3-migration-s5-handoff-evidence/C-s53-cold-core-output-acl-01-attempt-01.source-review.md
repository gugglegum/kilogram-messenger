# C2-O dormant source: actual output ACL refusal

2026-10-31. Baseline `1d9946fcb1ed9e918de8015e6353fcf41591410c`. Prepared for focused source review under the [accepted contract](C-s53-cold-core-output-acl-01-attempt-01.contract-accepted.md) and [proposal](C-s53-cold-core-output-acl-01-attempt-01.contract.md). **Source/static only; STOP before allocation or execution.**

## Scope and raw pins

Own delta: the new [worker](C-s53-cold-core-output-acl-01-attempt-01.preflight.ps1), [supervisor](C-s53-cold-core-output-acl-01-attempt-01.watchdog.ps1), this note, and three closed-selector edits in [Core](../../../scripts/migration-s53-cold-preservation.ps1). Core diff is 6 insertions/2 deletions: append the C#/PowerShell allowlist member and fix its family/case/attempt literals. Removing these exact edits reproduces HEAD's 79451-character source byte-for-byte. Limits, production selection, algorithms, native behavior and public observation schema are unchanged. Index remained empty; unrelated worktree changes were preserved.

| Source | Raw bytes | SHA-256 |
| --- | ---: | --- |
| Core | 79709 | ec3cd2190c4b67c201c02eeda6c68e5832e95ac114a2e570954e4a25a05f3bb9 |
| Worker | 106242 | 8bb6d0ec31984da3d18612906bc6cee5b16d45cfd3a862c6bcc390e005cedd43 |
| Supervisor | 33729 | 82b588f84e92691e75260bf06862b9e2ccadd563bcee0ef2c30b3a93fc1e4e13 |

The accepted contract remains 10390B/9a8e381f8a41ba0a94158bbf39ba5b707bdc3d0ddaca44c606d06bc89ab492c0. The [original output helper](../../../scripts/migration-s53-cold-output.ps1) remains 15195B/81f625dd3334c5ce6ba5d7efb36d54404ccfefd1bbe03849312121a241da4dea. The accepted C2-M W/S remain 110948B/98e58c85b3fce63f8e604a6b1d340249b5ea2954295da7b007840394cbe87438 and 37370B/70f9e79c51b9db6dcbaec218e2044370d334f684d22bdeeb1d9eb5917bc49664. Old evidence/navigation were not edited.

## New path and decisive barrier

One closed selector `boundary-output-acl` uses family C-s53-cold-core-output-acl-01-attempt-01/case output-acl. Successful healthy capture precedes ordinary E creation. OutputAclOwner is fixture-only, with no caller-supplied path, case or ACL. Its AssertPrivate and TakeDirectory are text-identical to the frozen owner. VerifyExactAcl adds only entry counting before its unchanged body. No substitute callback or injected ACL exception.

NarrowNewEmptyE requires newly prepared, untaken E, zero receipts/factory calls and one-use state. An identity-checked bounded FindFirstFileW/FindNextFileW emptiness check accepts at most the distinct dot entries and closes its owned search in finally. No child entry is allowed. It then opens only this E with access 0x60080 (READ_CONTROL|WRITE_DAC|READ_ATTRIBUTES), share3, OPEN_EXISTING3 and flags0x02200000. The original E handle and editor both allow read/write sharing and deny delete; Core receives only the original handle. The separate editor stays fixture-owned through refusal/Dispose and post-check.

Original current-user/SYSTEM ACE binary bytes are saved; only the Administrators allow ACE is removed from the protected three-ACE DACL. Owner/group/protection remain equal. The managed descriptor is bounded and pinned; GetSecurityDescriptorDacl must succeed with both presence and non-null pointer, then IsValidAcl must pass. SetSecurityInfo selects SE_FILE_OBJECT1 and DACL|PROTECTED_DACL0x80000004, with null owner/group/SACL arguments. Its direct DWORD return must equal0; LastError is not used as its result. The pin is released in finally; failure leaves cleanup_unknown. Readback requires exactly the original two ACE byte strings, current-user owner, unchanged group, protected/present DACL and identical E file identity. No restore, ancestor edit, permission addition or generic ACL API is exposed.

Invoke-AclBinding retains the original SafeFileHandle and original caught exception in a result box. It never infers entry from TargetInvocationException and never closes an uncertain incoming handle. Immediately at binding return, before any Core.Dispose, the driver requires all of:

- attempted invocation, no successful return, exact three-node shell exception chain and complete normalized/original marker sets;
- capture-completed Core observation with output_acl/output_binding/output, native0, HRESULT -2146233079, cause_kind unexpected;
- exactly one entry to the actual verifier, DirectoryTransferred=true and no receipt creator call;
- original wrapper already closed at return and Core ownership0/cleanup_unknown=false/native cleanup0.

Only then does it set consumption_proved_before_dispose and its QPC. Core.Dispose entry/return, readback through the retained editor and fixture Dispose have separate later clocks. The parent repeats these checks. A pre-entry, setter, identity, other exception or incomplete-close result cannot pass. Finally cleanup does not retry uncertain incoming closes: it retains an unclosed wrapper strongly, marks controller cleanup unknown and fails the case. The primary binding cause remains separate from assertion/cleanup failures. An unexpected successful Bind is reported truthfully as bound but fails the case.

Expected helper-owned handle stages are 7 prepared, 8 restricted, 7 after Take/refusal, then0. Core transiently owns/closes one incoming E handle during binding. Fixture Dispose closes editor+6 ancestors once; search and descriptor pin have distinct counters. The four helper snapshots check all37 fields; three Core snapshots check all38 (35 fixed fields, capture QPC, ordered phase durations, empty receipt digests). Proof-directory creation stays true, successful binding false, positive_proof false, receipt count/bytes/digests zero. WriteReceipts is never invoked.

## Static checks and independent public-data recount

PowerShell Parser.ParseFile: zero errors and duplicate functions for Core/worker/supervisor (3/51/28 functions). All25 shared W/S functions are identical, including assertions. Core inverse comparison equals actual HEAD source. Original AssertPrivate/TakeDirectory and verifier body comparison passed. C# string/comment-aware brace balance passed; this is not compilation. Sources are UTF-8 without BOM, LF terminated, without trailing whitespace; default git diff --check passed for Core.

The literal inventory is3562B/SHA-256 1daa4c64c070458b8b0c494131b7f31eaa8058947c85fd906ef671b2c3a3d579. Independent in-memory public-data calculations confirmed three equal five-row triplets with their required provenance, per-row byte digests, 52B payload and tuple H=54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5. Four passes would hash208B. Core model: 26 fixed strings plus11 relative strings =37 interned, path arena6800; 33 retained inputs plus simultaneous search/metadata yield peak35; records12073840+33*384+5*160=12087312. No cap changes. Expected family geometry is17 directories/13 files/3666 logical default-stream bytes/57 existing ADS checks; the new E emptiness iterator is counted separately. Controller guard ledger is20 (family5+case15); report/source bounds remain unchanged.

## Deferred boundary

Reuse [C2-M accepted primitives](C-s53-cold-core-output-boundaries-01-attempt-01.accepted.md) and [C2-N baseline](C-s53-cold-core-native-read-errors-01-attempt-01.accepted.md) for unchanged behavior. The frozen output helper's TargetInvocationException-based consumption inference remains a later launcher integration issue; this source does not repair or certify that failing path.

No harness was dot-sourced, compiled or executed in this stage. No fixture/.tmp existence/read/write/ACL operation, private/keys/database/network runtime, allocation/ledger edit or commit/push was performed. New native transition, exception shape and cleanup still require source acceptance followed by separately authorized runtime. Work20/whole30/lane1/retry0 is dormant; the accepted contract's funding/ceiling decision remains with root. No reservation/charge change, whole-attempt verdict or S5.4 advancement. **Callback and STOP for source review.**
