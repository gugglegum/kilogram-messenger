# External crash-action common loader correction and conditional field03 preparation

2026-11-20. Source review candidate on parent `f81eb45513a8005b2baae4f3319c007b4c6bb1c7`. No commit/push or installed Apply/Wake/field/Restore on this preparation step. Field02's accepted pre-fault verdict and complete restoration are retained in [its result note](terminal-t1-3e-sender-crash-field-attempt-02.md).

## Narrow source correction

The external `lab-crash-descriptor`/`lab-runtime-crash` branch now asserts the common path is plain, reads its bytes once, requires exact accepted common SHA256 `02b2a53e5b9d13f52312059ab1fc8a859de43eb694be72ce0eca00b06fe6346a`, and dot-invokes a ScriptBlock decoded from **that same verified byte array**. It never rereads the file between verification and execution. The common source has no PSScriptRoot dependency. Its candidate payload hash is unchanged.

The existing Host.command transport and its policy remain unchanged. No process Bypass is added to the checked invocation, and no User/Machine/global policy or system setting is changed. Identity/attempt/active-first-request, one-use publication/replay and expected-crash predicates are unchanged. Only the external branch changes; the existing native-owner launch path is untouched.

The policy test exposes its existing Fixture factory through a test-only FixtureLibrary switch, without starting its default classification campaign or creating a guard in library mode. Its default behavior retains the same guard and cases. This reuses the existing isolated fixture protocol, rather than introducing a fault framework. Sender-crash entry changes only fixed attempt/evidence02→03.

## Focused checks

`.tmp/terminal-t1-3e/sender-crash-loader-prep-01/transport-03` is the final native transport evidence, **9 cases PASS**, existing guard100/115 with empty Job. It runs actual inherited `Host.command` and the full encoded request with a complete six-input pin manifest. Checked PS5 invocations use process-only `PSExecutionPolicyPreference=Restricted` and contain no Bypass. Bootstrapping the disposable fixtures occurs separately; the original common file dot-source actually raises PSSecurityException in the checked invocation, proving effective file refusal.

- Unmodified published host descriptor fails at the common file; corrected descriptor returns the exact expected descriptor/profile hashes.
- Actual runtime-crash first-request branch publishes the request, waits for a synthetic policy-fixture reply, and returns it. Cached identical replay still works after fixture state becomes inactive. Changed request is refused and leaves original request bytes unchanged; only one synthetic reply publication occurs.
- Wrong session refuses. Wrong common pin in each external branch refuses before unverified marker code or fault request publication.
- A test-only after-hash hook replaces the on-disk common file before the unchanged production load expression. Descriptor still succeeds from the verified bytes, proving there is no post-check disk reread. This instrumentation exists only in that disposable fixture; the production host has no hook.

Synthetic reply publication is explicitly not a runtime crash or product durability test. No installed actor, native runtime termination or unrelated process kill occurs. Short-lived PS5 processes are owned by the existing guard. Earlier transport01/02 are preserved; transport03 adds the full six-pin manifest to the same focused checks. All passed.

The existing policy harness default run, `policy-01`, passed **64 cases**, empty owned Job; this verifies the library-mode refactor leaves ordinary/expected classification and mixed-policy refusal intact. No unchanged Rust/native death suite or general campaign was repeated.

Packet derivation has **8 synthetic mapping/refusal cases PASS**, clearly marked mock-only: current→future mapping, stale/wrong identities, forced/not-success outcome, loaded-policy, raw attributes and task drift. They do not claim actual Apply/Task.Run/DPAPI. Future A identity remains NULL in the saved templates.

## Concrete packet and actual read-only prerequisites

`packet-01/apply-plan.json` SHA256 `df9562e28810840872fc3157203a9c6301eba4705ccb39cb6a57ef64dc7569a8` is phase Apply/fresh refresh-a-02, bound to actual original A PID35024/start134396673583752763/session135d44c8ba8342cca13dc5bd349e4577. Existing runnerdb3cd102/helper190eb4c4, exact old/new manifestsfc3bf5b0/5a2784ca and canonical15 IDs/owner/group/DACL are unchanged. Current A has original old15 and no crash policy. Actual default read-only Validate passed for that plan with native/raw groups, task/readiness, canonical15 and idle predicates. No Apply/stop/Wake/DPAPI was invoked here.

Fresh B read-only inspection also passed with original6, same agent11884/start134395656927981748/sessiona6e8faadcf4d4979b422f85578ae7c64, idle/unblocked/readiness. Proposed refresh-a-02/restore-a-03 and field03 evidence paths are unused.

All30 runtime source pins and both asset cohorts are verified. Product EXEs are reused unchanged; only the candidate host asset hash changes to `5a5a92876592ea543b5ac089c3eba16e59dccd24f94ad6d6fb357a99cc9a650e`. Static generation of real A commands with the actual six-pin manifest and maximal numeric PID/start fields stays within the unchanged7500 cap: descriptor3752, runtime-crash6832. This generation does not execute installed actions.

`field03-plan.template.json` SHA256 `33a4b34a77d98f42d318c818cde0648c5a2fa155514a67cbb23f7642e5c7da98` changes only required source hashes, host asset/package binding and attempt/evidence03; all functional contract/timing/quotas stay unchanged. Its seven future A identity/token/observation fields remain NULL. `restore-plan.template.json` SHA256 `a9d038e648567a4aa6a9b8e18c85fd9e17f6093753bbc06364344e0c977e3024` is fresh restore-a-03 with only future expected_actor NULL. Acceptance templates are false/unbound. Full executable field static validation is intentionally deferred until actual Apply identity exists; current preparation verifies the entire source/assets/static closure without fabricating that identity.

`materialize-success.py` derives the two concrete plans only from the actual successful fixed Apply result, unchanged token/task/canonical predicates and actual new-session DPAPI response. It checks verified new source publication, derives a reverse plan first, default-validates current A against it, checks fresh A/B original6/idle/readiness, and runs the full unchanged sender-crash static validator. It saves allowed-only structural proofs and leaves acceptances false. Its pure mapping is tested; its future installed-success branch has not run because Apply is not authorized at preparation time.

`commands.txt` and `run-field03.py` give the bounded next sequence for a single conditional root review: exact source publication; accepted Apply180/200; actual-success derivation; separately authorized field03 at1800/1860/BelowNormal/max2/native1000/1015reserve60; conclusive original6 cleanup on both sides; success-derived metadata-safe Restore03 at180/200 with ordinary readiness and independent preservation verification. Existing known-state rollback/abort paths stay narrow; unknown state blocks without force or policy changes. No field03 physical crash or installed activation was performed.

The scoped publication candidate includes the field02 result note and this fix note, host correction, sender03 binding, new transport regression and policy fixture-library change. Foreign worktree changes and prior root notes are excluded. Return one substantive ready callback, then idle without polling; publication/activation requires root acceptance.
