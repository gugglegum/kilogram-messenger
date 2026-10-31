# C2-O proposal: restrictive ACL failure at output binding

2026-10-31. **Contract only; no implementation or execution.** Baseline `1d9946fcb1ed9e918de8015e6353fcf41591410c`. One remaining [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md)/[r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) negative, using accepted [C2-M ownership/output evidence](C-s53-cold-core-output-boundaries-01-attempt-01.accepted.md), [C2-N baseline](C-s53-cold-core-native-read-errors-01-attempt-01.accepted.md) and the current verification workflow.

**Recommended scenario:** create one fresh empty fixture E normally, then remove only its explicit Administrators allow ACE. Keep current-user and SYSTEM FullControl, owner/group, protected DACL and remaining ACE flags unchanged. The existing real verifier must reject the missing third ACE inside Core.BindOwnedOutput. No fake verifier or preselected exception.

## Preconditions and the sole mutation

Proposed family `C-s53-cold-core-output-acl-01-attempt-01`, case `output-acl`; E is that new case's fixed `proof` directory. All paths remain closed literals. Reuse healthy five-row/52B mirrors, six empty locks and safe-empty-S. Require successful CaptureAndCompare/four passes208B/H and released inputs **before E creation**. Existing accepted evidence covers unchanged primitives, not the unexecuted new ACL transition.

1. A fixture-only binding of the accepted normal output owner creates E with its usual protected three-ACE DACL and completes PrepareDirectory: checked identity/path/type, current-user owner, exact current-user/SYSTEM/Administrators FullControl and ancestry. Record immediate E ownership; reject collisions. No receipts are created.
2. Require the same newly owned E to be empty by a bounded check whose iterator/current owner is closed before the edit. Reject any entry. Open **one extra handle to this E only**, READ_CONTROL|WRITE_DAC|FILE_READ_ATTRIBUTES, no-follow/directory flags and compatible sharing denying delete. Match it to the owner's retained E identity before mutation. Retain it across binding/refusal for post-checks; it never transfers to Core.
3. Through that handle, set only the protected DACL to the original explicit SYSTEM and current-user ACEs (same FullControl/inheritance flags), omitting Administrators. No owner/group/SACL/label change. A non-null, present, valid two-ACE DACL is mandatory; no null/empty DACL fallback. This removes an allow ACE from an allow-only protected DACL; it adds no principal or permission and never edits an ancestor, input or preexisting E. Current-user FullControl remains available even if the user also belongs to Administrators.
4. Read back exact two-ACE shape, protection, unchanged owner/group and same E identity. Independently validate this intended restricted shape; **do not call/replace the original rejecting verifier here**. No children exist to receive propagated inheritable ACEs. Call the normal TakeDirectory, then BindOwnedOutput with the owner's actual AclVerifier delegate.

[SetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setsecurityinfo) identifies the edited object by handle, changes only selected components and returns its error code directly. The proposed write selects DACL plus protected-DACL only; success must be0, not inferred from LastError. [GetSecurityDescriptorDacl](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getsecuritydescriptordacl) distinguishes present from non-null DACL: both must be checked. Descriptor lifetime stays owned until the native call finishes and then is freed. No privilege enabling or system setting change.

This uses narrowing after normal PrepareDirectory intentionally: creating E initially with two ACEs would be rejected by PrepareDirectory and would **not** exercise binding. TakeDirectory has no ACL recheck. The actual VerifyExactAcl/AssertPrivate bodies remain unchanged: the two remaining ACEs pass their individual tests, then seen.Count!=3/missing Administrators triggers Refusal(private_acl).

## Exact refusal and ownership

Source anchors: [Core RememberFailure/Normalize](../../../scripts/migration-s53-cold-preservation.ps1), [accepted owner/verifier and reflection binding](../../../scripts/migration-s53-cold-output.ps1), and the fixture-bound BoundaryOwner in the accepted [C2-M worker](C-s53-cold-core-output-boundaries-01-attempt-01.preflight.ps1). Current raw pins were reread: Core79451B/`9b4a7d6165c11d82ff122ca6c13f8876d616b711424e7355e5508e73c7a2cf06`; original owner15195B/`81f625dd3334c5ce6ba5d7efb36d54404ccfefd1bbe03849312121a241da4dea`; M worker110948B/`98e58c85b3fce63f8e604a6b1d340249b5ea2954295da7b007840394cbe87438`.

Bind sets role=output/phase=output_binding, reserves one Core handle, validates Info, then invokes the real ACL verifier. Original InvalidOperationException has HRESULT -2146233079, cold_output_code=private_acl and rootcopies_code=private_acl, no colda_code or inner exception. RememberFailure maps the rootcopies mark while output to:
**output_acl / output_binding / output; native_error0; error_hresult -2146233079; cause_kind=unexpected.**
“Unexpected” is the current classification string, not permission to accept an arbitrary exception. It is neither bounded_refusal nor native access-denied5. Normalize adds the exact colda code/phase/role around the original exception without native marks.

Core's semantic chain is normalized InvalidOperationException → original private_acl InvalidOperationException. With the fixed shell's recorded PowerShell7.6.5 and the approved reflection invocation, expected observed chain is **MethodInvocationException(-2146233087) → normalized InvalidOperationException(-2146233079) → original InvalidOperationException(-2146233079)**. [.NET Invoke](https://learn.microsoft.com/en-us/dotnet/api/system.reflection.methodbase.invoke?view=net-10.0) introduces TargetInvocationException, but PowerShell's [binding catch](https://raw.githubusercontent.com/PowerShell/PowerShell/v7.6.5/src/System.Management.Automation/engine/runtime/Binding/Binders.cs) and [ConvertToMethodInvocationException](https://raw.githubusercontent.com/PowerShell/PowerShell/v7.6.5/src/System.Management.Automation/engine/runtime/Operations/MiscOps.cs) unwrap that layer. Retain full raw actual chain later; another shape is not silently accepted.

**Binding detail for subsequent source review:** the M wrapper detects method entry by finding TargetInvocationException. That alone is insufficient on the unwrapped failing path. Its best-effort Dispose on an already invalidated/closed SafeHandle makes no second native close, but must not be reported as proof of pre-entry ownership. A new case binding must establish consumption from the exact Core refusal, entered real-verifier call, closed original wrapper and Core known-zero ledger; preserve the original exception. Never retry native close or claim receipt ownership. Frozen helpers are not edited at this contract phase.

Core never assigns outputDirectory/createRestricted/assertPrivateAcl or success transfer state after the rejecting callback. Its finally consumes/closes the incoming directory even on refusal, invalidates/disposes the wrapper and releases its transient slot on observed close success. GetObservation then reports owned0/cleanup_unknown=false. outputBoundOnce being set is not output binding success. Do not call WriteReceipts.

| Boundary | Fixture-owned handles | Core output handles |
| --- | ---: | ---: |
| Prepared E |6 ancestors + E =7|0|
| Edit/post-check handle retained|8|0|
| TakeDirectory / Core binding|7|transient1|
| Core refusal and returned Dispose|7|0|
| Post-check, editor/ancestry cleanup|0|0|

After Core.Dispose, verify E through the retained edit handle and read back the intended restricted DACL, then close it and the six ancestors once; release descriptor/current iterator ownership on every path. Do not restore the removed ACE: that would broaden the retained fixture again. Keep partial E and reports. A setter/readback/prepare/identity error or failed close is failed_or_unknown, never this expected refusal.

## Minimal evidence, later budget and stop

Reuse full38-field Core observation schema at capture return, binding refusal and Core.Dispose, rather than inventing new Core APIs. Required deltas: completed=true/four hashes208/H and released inputs remain; receipts_completed=false, receipt bytes/knownwritten0/write_unknown=false/digests empty; only output_binding phase added; precise error/cause above; final owned0/cleanup_unknown=false. No receipts phase or creator call.

Keep only necessary new facts: fresh E identity/creation and empty precheck; one DACL narrowing/native result0 with exact before3/after2 shapes and owner/group/protection checks; one real verifier invocation **during Bind**; directory transfer and closed-wrapper/known Core cleanup; one extra edit handle opened/closed, descriptor freed, ancestry/current ownership0; ordered capture/create/edit/bind/refusal/CoreDispose/post-check/cleanup clocks. Reuse M's bounded guardian/job/EOF/reap/report checks.

**Truthful partial E:** E_created/owner.DirectoryCreated=true and DirectoryTransferred=true even though Bind fails; creator calls/mask/transferred receipts all0, no receipt/proof artifacts and no positive preservation proof. Existing M field proof_created means the directory was created: if reused it must stay **true**. Calling the empty directory “no proof directory” or setting that field false would be incorrect. whole_attempt_verdict=not_established.

Later proposal only: one work20/whole30-second batch, lane1/retry0, existing source131072/report65536each/196608total caps. No new general ACL/collector mechanism. Native mutation/cleanup/exception-shape behavior still requires reviewed source then separate allocation/runtime. Current inactive C26.5825091 cannot reserve C30 without at least C3.4174909 additional ceiling; root decides funding/ceilings at activation. No reservation or charge here.

Only this contract is added. Public source/API reads and document integrity/link checks only; no Core/helper edits, harness rebuild, dot-source/Add-Type/compile/runtime, fixture creation/ACL change, private grant, commit/push or S5.4. **Callback and STOP for contract review.**
