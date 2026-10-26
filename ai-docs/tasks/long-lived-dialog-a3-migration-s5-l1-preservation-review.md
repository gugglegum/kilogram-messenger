# S5.2 preservation completion accepted; prepare final submission

2026-10-26. Root accepts the narrow logger repair and the two actual contributing
checks below. This completes the missing checkpoint preservation, not the final
implementation/publication review. The earlier
[boundary03 completion decision](long-lived-dialog-a3-migration-s5-l1-boundary03-completion-review.md)
continues to define the scope and limitations of its functional evidence.

## Independently verified outcome

Readiness binding afdd0ad89fb55ae3fee6d412b55e8c58ec905d585b4f90dd75b38b6fa301bae3
had199 current references, all verified by root. Root and an independent read-only
reviewer found no blocker in the actual Phases/helper C test or preservation-only
F path. Only controller and runner changed; the genuine boundary assertions/body,
product sources/images and raw preservation implementation stayed unchanged.

- C-boundary-phase-preservation-01: whole PASS24.1463993s, actual matching test
  1passed/0failed in0.88s, normal wrapper/cache finalization. Work30 unexpired,
  whole40 owner completed, owned children0. The helper copied the actual closed
  phase prefix98B and256 opaque bytes while two real empty locks were held;
  subsequent source phase logging continued to345B.
- F-exhausted-preservation-01: whole PASS67.5439277s, actual matching test
  1passed/0failed in46.26s, normal wrapper/cache finalization. Work120 unexpired,
  whole140 owner completed, owned children0. This used the original stopped NCjBHb
  absolute paths and unchanged raw helper under both OS locks, without typed DB
  opens, runtime, migration, recovery, restore or frozen-origin traversal.

Root verified both matching logs and results,54 post-case source/image references,
the unchanged harness12bb6adda2a96c518785b654369805f6502b6e69998e28b30b1cde052688156e,
all39 ledger rows against their actual case files and the sums. Process snapshot
found zero live Kilogram processes. Root read the existing preservation manifest
and verified its hash and all three inventory arrays; no new fixture traversal
or DB/state open was performed for this review.

The F manifest records6956 files/45597399 bytes with identical source-before,
source-after and destination path/size/SHA256 tuples. Only the two exact held empty
OS-lock source digests are derived from checked zero length; all destination
digests and every other source digest are actual byte reads. The22 completed-case
public artifacts also remain byte-identical before and after preservation.

Private backup, never activated:
`.tmp/migration-s5-preserved/F-exhausted-preservation-01`.
Its `preservation-inventory.json` SHA256 is
`c2a56327fd1462b471163956fbfb61c6b89a07e142f4828c76011c29cadf28b3`.
The source remains stopped at
`.tmp/migration-s5-storage/l1-exhausted-observed-NCjBHb`.
Later cut3 may restore bytes only to these original pinned absolute paths, with
one active branch per Native domain; this review does not activate cut3.

Public evidence in `long-lived-dialog-a3-migration-s5-l1-evidence/`:

| Artifact | SHA256 |
| --- | --- |
| C-boundary-phase-preservation-01.json | 2e640f6112d056ed7337dfc3dcad3b4d862d19d8110c2dee098ef90cde38002d |
| C-boundary-phase-preservation-01-result.json | 5620bd3725e711f355e71fcd6c03d5a5e15c616a6c7464a5ee39f8430c463554 |
| F-exhausted-preservation-01.json | 2180aadd201d76b164302088d25706a4a46561be76ddc1fbddf54267bbaeb320 |
| F-exhausted-preservation-01.log | 2a2eb80ea7c8f5cde988da54f4fbb0aed0351a9eb9fa50c2ac3f48c9c0ee0d24 |
| F-exhausted-preservation-01-result.json | bcd69f3c44940c269f38c3fc5fc463e30803d7f5c5e2887a9875305d62926ca8 |
| contributing-cases.json | 8acf1e8a11994e839f032e45e28aefcae7ce713f9eac47791fd7599af320b2fa |

Boundary03 stays whole FAIL649.592232s. Its missing original parent-result is not
reconstructed. The completed functional assertions are accepted solely under the
earlier source-bound review; this later PASS covers preservation only. Historical
Ready02/capacity01 split outcomes and all failed attempts remain recorded.

## Finish the assigned S5.2 submission

Complete the original implementation report and exact candidate allowlist against
HEAD73c499818ba9b699c331aa842e64e8432e90c59a. Include raw size/SHA256 and normalized
Git blobs, final source/image/static bindings, required coverage and justified
reuse, every actual failure/repair/split result, private checkpoint ownership and
explicit missing/deferred cut3/whole-P/MVP coverage. Keep private state local.

Current actual C172.0310185/F3331.2897354/P0 under500/3650/0; F remaining318.7102646.
No further C/F, source changes, product rebuild, budget change or refund is needed
for documentation and manifest finalization. Do not overwrite accepted evidence.
Keep unrelated work and root-owned navigation/task/review material intact; identify
all excluded paths. The queued S5.3 handoff task remains outside this candidate.

Callback with final report/allowlist paths and hashes, then stop for root acceptance.
No commit/push yet. After explicit acceptance root will assign exact publication,
verify committed/pushed blobs and then dispatch the already agreed next stage.
