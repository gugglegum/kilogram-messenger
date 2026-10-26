# F-obligations-resume-02 — partial diagnostic result, FAIL

Actual runner40.0101169s; test31.68s. Original25s condition expired at pending56.
The150s whole/125s work guard did not expire; owned children after cleanup0.
This is not a full obligations PASS or a reclassification of previous failures.

Before typed repository open, both exact OS handles were locked and checked empty
before/after preservation. The private stopped copy contains203 files with matching
source-before/source-after/destination byte identities. Only the two exact source
OS-lock digests were derived from checked length0; both destination digests were
actual byte reads. Private inventory SHA256:
`172bc6c6a876fca0bfb813b190a422cc0ad4913be84138f245accb7c99ffaf21`.
Its path is recorded in the exported starting JSON; private bytes stay local.

Actual cold input: A.next[70,5], debts5..69; B.next[5,5], pending6..61 (56), same
actual C/D/E/SID, no quarantined row. Runtime startup retained session/key/Guard
correspondence and all constructor/packet counters0. Old stderr042b10...d4958
is unchanged. Fresh locators changed policy generations through the public path.

The sender resumed its volatile scheduling cursor fromNone. It replayed actual
unacknowledged source records, which were already durably pending at B. Four
successful exact Complete checks establish progress, not receipt/debt retirement:

| after cursor | actual selected ordinals | owner outgoing ms | Deliver RPC ms | exact Complete monotonic tick ms |
|---|---|---:|---:|---:|
|None|6..13|203|1850|7811|
|13|14..21|258|1619|12986|
|21|22..29|153|1797|18058|
|29|30..37|165|1624|22457|

Complete-to-Complete intervals were5175,5072,4399ms. No observed RPC exceeded
its15s product limit; no stuck worker, cache divergence or ref leak is proven.
The final observed B state still had56 pending, unchanged Native, zero packet
calls, and two admitted C. The new tail62..69 was not selected in this attempt.
Repeating fromNone with eight-source batches requires eight delivery cycles before
that tail; the observed first four cycles consumed most of this condition window.
This explains why this resumed observation cannot establish full64 within25s;
it does not prove the full cause of either earlier fresh-attempt timeout.

Not reached: full64+reserved head, current-versus-captured O/A/F/R, delayed debt
release, quarantine/renewal and C2 progression at those later boundaries. Different
peer isolation, affected S5.1 capacity and exhausted-origin gates remain unrun.

For root review only, not applied: the existing test scheduler supports legal
wire1 direct_batch32, independently of the Native four-packet turn limit. With
head5 held, its expected source selection is31 records6..36,32 records37..68,
then69. This could test the same64/head/ref requirements in three cycles while
retaining25s and all product timeouts. It needs a root decision before another F;
this report does not authorize one or assert its result.

Current charged ledger: C105.3761137/F763.8892556/P0; F remaining1336.1107444
under2100. Other F and511 remain closed pending the required root callback.
The measurement/publication distinction is documented in timing-boundary.md.
