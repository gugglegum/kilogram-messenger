# C2-O contract acceptance

2026-10-31. Root accepts the single-case [proposal](C-s53-cold-core-output-acl-01-attempt-01.contract.md) for dormant source preparation, not execution. Baseline 1d9946fcb1ed9e918de8015e6353fcf41591410c; root271b38 verified contract/Core/original output owner/C2-M worker raw pins and empty index.

Root inspected real AssertPrivate/VerifyExactAcl, PrepareDirectory/TakeDirectory, Core RememberFailure/Normalize/BindOwnedOutput and the PowerShell reflection wrapper. The proposed missing Administrators ACE reaches the actual verifier's final membership check after normal directory preparation. The expected mapping is output_acl/output_binding/output, native0, HResult -2146233079, cause_kind unexpected. No change to this classification or injected replacement exception is approved.

The change is limited to removing one allow ACE on newly created, empty, fixed-path fixture E, preserving current-user/SYSTEM rights and all other security components. [SetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setsecurityinfo) returns its error code directly; [GetSecurityDescriptorDacl](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getsecuritydescriptordacl) requires separate presence and non-null checks. Both checks and descriptor lifetime must be explicit in source.

Source-review requirements:
- Preserve the actual verifier body; instrumentation may count entry without replacing its decision or exception.
- Prove Core consumption from verifier entry, exact refusal, closed incoming wrapper and known-zero ownership at binding return, before later Dispose can obscure it. Dispose and fixture cleanup remain separately observed. An unexpected pre-entry failure is not PASS.
- No second native close on a consumed/uncertain handle; preserve primary failure and truthful cleanup_unknown on incomplete cleanup.
- Keep E-created/directory-transferred facts true, successful binding false, receipt/positive-proof facts false. Do not conflate proof-directory creation with proof success.
- Scope source to one new W/S pair and source-review note plus minimal closed fixture selectors in Core if needed; retain reviewed production behavior, original output helper, old evidence and unrelated worktree changes.
- Track the original reflection-wrapper consumption inference as a later launcher integration issue; this acceptance does not certify that failing path in the frozen helper.

Reuse existing accepted evidence for unchanged primitives. Static parse and source checks only; no dot-source, compilation, fixture/ACL mutation, native runtime, private access, commit/push or S5.4. Proposed work20/whole30 remains inactive; ceiling/reservation decision occurs only after source acceptance. No charge or reservation change.
