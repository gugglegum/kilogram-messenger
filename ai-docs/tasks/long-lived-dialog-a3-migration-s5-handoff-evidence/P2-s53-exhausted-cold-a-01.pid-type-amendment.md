# C2-T Await-Start PID type correction authorized

2026-10-31. Root7fc049 independently confirmed that pinned-shell ConvertFrom-Json -DateKind String decodes public JSON supervisor_pid1234 as System.Int64, while current Await-Start requires Int32. This would reject a valid decoded start record.

Authorize only the production predicate to accept Int32 or Int64 with 0<PID<=Int32.MaxValue and PID!=self.Id. No strings/floating-point/coercion, no relaxed identity/time/pin/claim checks. Extracted test validator must remain identical to revised production source; do not coerce the fixture to hide the mismatch.

Update launcher pin, extracted definition hash and affected inert examples/review; Core/output/ParentGuards unchanged. Final revised source remains pending root recheck and one allocated public control run. No private/runtime grant, allocation, ledger change or publication yet. Existing source acceptance applies to the reviewed design; its old launcher byte pin is superseded only after the corrected source is rechecked.
