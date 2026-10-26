# A3-S5.2 obligations — empty synchronization-file preservation and attempt02

2026-10-26. Root accepts the narrow snapshot-helper correction below and authorizes
one further diagnostic continuation after its focused mechanism probe passes.
The [previous decision](long-lived-dialog-a3-migration-s5-l1-obligations-continuation-review-1.md)
remains the governing scope, input, assertions and timing rules except for the new
case ID and the explicit empty-file handling described here. No product contract
or runtime timeout changes are authorized.

## Verified failure and budget

F-obligations-resume-01 remains FAIL9.6329237s. Its log shows Get-FileHash failing
on the held Windows synchronization file during the first inventory. No preserved
output directory exists, and the runtime/typed repository phase was not reached.
The deadline record has work_expired=false and owned_children_after_cleanup=0.
Root verified both actual lock files have Length0 and the original a.stderr hash
is still `042b10b35f54c868493e4e69056322afee076297c8c63b1ae29dfa15624d4958`.

Current charges are C102.4639586/F723.8791387/P0, including the subsequent
C-preservation-01 PASS1.514107s for the initial empty/nonempty-file mechanism.
Limits remain C500/F2100/P0,
without refunds or an additional allocation. F remaining is1376.1208613. Another
whole150s attempt would leave1226.1208613, still above the1215 long-origin reserve,
but not enough to pre-authorize all other remaining F checks. Long511 remains closed.

## Narrow accepted correction

StateDirectoryLock::acquire creates/opens `.kilogram-state.lock` without truncation,
locks it and then runs recovery. This helper must continue to acquire only the raw
OS handles without repository recovery before preservation. The two observed files
are empty; emptiness is a checked fact of these inputs, not a new global format rule.

Permit special handling only for the **two exact normalized lock paths associated
with the retained OS handles for the two Profile3 state directories**:

1. Using the already held handles, require regular non-reparse lock files with
   length0 before preservation and again before releasing the handles. The helper
   also checks the path metadata remains length0 during before/after inventories.
   A nonempty file, unknown lock path, missing ownership/lock evidence or changed
   metadata must fail closed; never truncate or edit the source.
2. The only byte sequence of length0 is empty. Record its exact empty SHA256
   `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
   with explicit provenance `held-lock length checked; empty-byte digest derived`.
   Do not claim that Get-FileHash read the locked source bytes.
3. Create a new zero-byte destination synchronization artifact without BOM/newline,
   using create-new semantics. Verify its actual length and byte hash normally.
   This preserves the source byte sequence; it does not synthesize Native, Head,
   Event, database or private-key state.
4. Every other file retains the existing actual read/hash/byte-copy comparison:
   source-before, source-after and destination byte/size/SHA agreement. Do not skip
   all `.lock` names, suppress arbitrary access errors, omit private databases or
   broaden the special case to arbitrary empty files. Keep path/reparse checks.

The initial C-preservation-01 confirms the empty/nonempty-file principle, but its
helper still selects exceptions by basename and derives the destination digest.
After narrowing those checks, run one small C-class mechanism probe with a
genuinely held empty lock and an ordinary non-lock payload. It must demonstrate a matching preserved result while
the original locks remain held, plus refusal of a nonempty held lock and of an
unlisted/unsupported lock exception. No Native fixture or broad suite is needed.
Charge the probe as actual C and preserve any failure. If it fails, report the
specific cause before another F attempt. If it passes, no additional root/human
confirmation is needed to execute the already authorized next paragraph.

## One new F case, then callback

Authorize exactly **F-obligations-resume-02**, the same filter and original stopped
active branch `.tmp/migration-s5-storage/l1-obligations-Iw3nQI`, bound to this
decision's hash. Preserve its input in the new private immutable path
`.tmp/migration-s5-preserved/F-obligations-resume-02`. Never reuse the prior case ID,
overwrite its final/started record or rewrite old evidence. An unfinished started
marker requires reconciliation before another invocation, not marker replacement.

Retain the150s entire-job and125s work cutoff, dedicated owned job and cleanup,
exact starting-state checks, one active Native-domain branch, case-specific logs/
schedules, monotonic owner/RPC/Complete trace, original25s condition and all
full64/head/capture/debt/quarantine/C2 assertions from review1. The original failed
DB snapshot is preserved before typed open; the frozen d9b seed is never opened.

Static preparation and this one F attempt are authorized after the focused probe.
No automatic third F attempt, new65 generation, budget reset,511/S5.3 or publication.
Callback root with the actual result, input preservation/probe/source/image hashes,
last completed phase, assertions/remaining coverage, cleanup and ledger. Root will
decide the remaining checks and reserve from that evidence.
