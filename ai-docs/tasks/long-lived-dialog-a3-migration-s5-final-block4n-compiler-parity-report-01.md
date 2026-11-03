# Block4N — compiler filename representation parity, ready for review

2026-11-03. Baseline `9013f9cfb2e1803d279eaa0377b2865c835618ec`. Task `long-lived-dialog-a3-migration-s5-final-block4n-compiler-parity-task-01.md`. Original shared native compiler validation воспроизвёл отказ на pinned public F04 inputs; после узкой нормализации filenames тот же compiler-only вызов прошёл. Production Run/Prepare/full admission не выполнялись. F04 остаётся consumed/failed; исправление не является runtime или whole S5.4 acceptance. F03 причина по-прежнему UNKNOWN.

## Before/after evidence and exact scope of the finding

Использован неизменённый original F04 inventory116874B SHA256 `b548cf47be85fdae660bf7ed183438b5810f376bc2c3275b78f38e7ac824b85d`. Normal capture1765914B SHA256 `e1bea8b4fc7c4ce98e00da98cd303f0295cbd9a72aa4f1f01ce2a6c2bb3f530c`; test capture2730375B SHA256 `40b014b4c8ac56e16e8729f1b52fea7bee3d38a89f4ab872f8a2b0125f7ee4d8`. Исходные captures/inventory/hashes не переписывались и не перепривязывались.

До исправления compiler implementation сохранён отдельно:18873B SHA256 `3f9d92b7deac877acba3db36f015128981251f2d8b3023f963f84300ae1373da`. Перед baseline probe raw bytes живого compiler source совпали с этой копией; Git diff compiler file был пуст. Изменены были только добавление test module и диагностический test code, а не реализация валидатора.

Exact nonproduction test `live_l1::handoff::final_p5::admission::tests::compiler_parity::s54_compiler_public_f04_replay_without_runtime_admission` вызывает **только** `compiler::validate(&Policy::production(), &pinned_inventory)` для настоящих данных. Не вызываются inspect_state/load_for/AdmittedRun/full admission/claim/clock/functional factories. Ordinary test без opt-in дополнительно проверяет fresh tiny public fixture; explicit `KILOGRAM_S54_F04_COMPILER_PROBE_PHASE=before/after` включает фактический pinned F04 replay и отдельные наблюдаемые markers. Успех обычной fixture не заменяет результаты этого explicit replay.

Baseline actual output:

```text
S54_PUBLIC_COMPILER_ORDER normal_nonordinal=44;tests_nonordinal=45
S54_PUBLIC_COMPILER_CHECK_LABEL normalized actual compiler units, no duplicate or invented profile
S54_PUBLIC_COMPILER_REPLAY phase=before;result=refused;check=normalized_actual_compiler_units;runtime_capability=false
```

Это первый наблюдённый отказ **original shared compiler validator** на исходных public F04 inputs. Его fixed label сопоставлен с error chain в памяти; arbitrary error Display/Debug/path/private values не печатались. Native test exit0 означает успешную проверку ожидаемого Err; сам compiler validation вернул Err. Baseline native test15.82s, весь bounded command27.0826496s.

В normal capture447 units, из них44 filename vectors имеют иной порядок, чем Rust ordinal; в test capture501 units, из них45. Конкретный механизм: raw Cargo filenames нормализуются/сортируются при построении actual Rust Unit, а прежний `encoded` сортировал лишь features supplied Unit. PowerShell producer сортирует filenames своим locale comparator; например `[kilogram_cli.pdb, kilogram-cli.exe]` не совпадает с Rust `[kilogram-cli.exe, kilogram_cli.pdb]`. Vec сериализовался с этим отличающимся порядком, поэтому равные filename collections давали разные unit identities.

После исправления actual output на тех же pinned captures:

```text
S54_PUBLIC_COMPILER_ORDER normal_nonordinal=44;tests_nonordinal=45
S54_PUBLIC_COMPILER_REPLAY phase=after;result=passed;runtime_capability=false
```

Полный shared native compiler validation теперь вернул Ok; native test52.91s, весь focused Rust command96.7934682s. Он прошёл normal/test capture boundaries, inputs, units, artifacts, embedded/dependency checks и frozen normalized unit identities. Это подтверждает defect и устранение воспроизведённого compiler-only отказа. Сохранённый original F04 runtime diagnostic остаётся `compiler/unknown`; более точный label получен этим отдельно разрешённым replay, а не извлечён из старого transcript. Не утверждается новое прохождение complete admission/активации/clock/runtime и не выводится причина F03.

## Minimal implementation and representation audit

- Rust: в уже существующем `encoded` clone Unit добавлен один `u.filenames.sort()`. Он применяется к actual и supplied vectors; все поля остаются в identity, sort не удаляет элементы.
- PowerShell: общий `ConvertTo-S54CompilerUnitIdentity` вынесен из прежнего inline canonical block и используется самим `Assert-S54Compiler` и focused representation audit. Существующее path normalization и feature handling сохранены; filenames сортируются ordinal на обеих сторонах сравнения без Unique/dedup.
- Existing duplicate raw-path checks, canonical-path policy, complete count checks и duplicate unit refusal сохранены. Missing/extra/changed filenames, profile, features, executable, freshness, class и provenance не исключались из сравнений. Expected hashes, raw records, captures, frozen manifests и source guard не менялись.
- Перестановка filenames не меняет membership/multiplicity. Порядок kind и остальных identity fields не расширялся; existing feature normalization и unordered unit comparison сохранены. Сериализованные raw unit identities для frozen provenance остаются точным отношением с исходными captured units; ephemeral comparison clone их не переписывает.

На исходных public F04 captures actual shared PS unit identity helper подтвердил обе unit relations. Отдельно прошли `Assert-S54Embedded`, equality before/after/cross-build inputs, frozen build_inputs/artifacts и `Get-KilogramFrozenUnitIdentity` relation. Frozen manifest817612B SHA256 `648030215990528feddfef693357c027e2473f025f8e80b19b1549e20a483022` не изменён. Материальных других representation discrepancies в этих shared compiler/frozen/embedded relations не осталось.

**Ограничение PS audit:** полный `Assert-S54Compiler` на original F04 inventory правильно отвергает live source freshness, поскольку N исправляет исходники, hashes которых зафиксированы F04. Для normal и tests зафиксированы два отказа именно из `Assert-S54CompilerSourcesFresh`. Guard не подменялся/не пропускался, current hashes не restamp-ились в старый inventory. Поэтому report не объявляет полный PS compiler pass на изменённом checkout: прошли actual unit/frozen/embedded representation relations, а current freshness refusal обязателен. Compiler-only native probe по исходному контракту не выполняет source/admission guards; его Ok тоже не full admission.

## Focused regressions and actual bounded commands

Fresh administration `.tmp/s54-block4n-compiler-parity-01`. Existing fixed **mutable nonproduction** target `target/s54-diagnostics-source-checks`; old K image hashes — historical observations. Production F04 images/Cargo origins/dependency files/captures и F01–F04 artifacts отделены от этого cache и сохранены. Использованы installed pinned pwsh/Rust1.98, Jobs2/BelowNormal/offline/locked, process-local environment. Compile-only — через `scripts/run-cargo-tests-stable.ps1`; exact native execution — тем же stable runner. Direct network cargo test не запускался.

Common bounded entry:

```text
pinned pwsh -NoProfile -NonInteractive -File .tmp/s54-block4n-compiler-parity-01/owner.ps1 -Step <step>
```

| Step | Actual final tool return | Exit / elapsed |
| --- | --- | --- |
| format-01 | fa074d | 0 /2.1009802s |
| compile-01 original validator | 992b3b (launchc8117e/session9747) | 0 /64.2231657s |
| rust-01 explicit before replay | e01a25 (launchdf2cc2/session20973) | 0 /27.0826496s |
| format-02 | dae530 | 0 /1.4216461s |
| compile-02 repaired validator/tests | 8a8b72 (launchbbd43e/session88725) | 0 /41.3358882s |
| rust-02 five exact tests | e15bff (launch98199f/session67006) | 0 /96.7934682s |
| ps-01 failed fixture export observation | fc8d0b | 1 /1.3226498s |
| format-03 fixture-only correction | 827bb0 | 0 /1.307628s |
| compile-03 | 48bc2f (launche35ef6/session35015) | 0 /36.4077378s |
| rust-03 one affected export/permutation test | 81190c (launch783e96/session52436) | 0 /11.1439545s |
| ps-02 first real-source audit | afa7c5 (launchc54f24/session45391) | 1 /19.6540086s |
| format-04 shared PS helper/audit | 675e5c | 0 /1.2255891s |
| ps-03 final PS regressions/audit | 476cef (launch92b8ac/session64927) | 0 /38.3914918s |
| seal-01 | 2185b4 (launch0f0ad9/session37945) | 0 /10.7661853s |

Compile commands1170/1200/30, остальные570/600/30. All native children waited, actual handle/Job verified, stdout/stderr complete bounded EOF, reader_failed=false/work_expired=false. Compile-only cleanup forced для owned residual PID67920/61936/29316 соответственно, identities не установлены; verified=true/after=[]/failed=false. Native before/after/regression runs, PS checks и seal — clean/nonforced. Это source-check cleanup, не runtime verdict.

Repaired Rust five exact tests: actual F04 compiler-only replay; три accepted EXE/PDB cases (исходный порядок, supplied permutation, raw permutation);11 finite missing/extra/duplicate/changed filenames/profile/guard/fresh/features/executable/raw-duplicate/unit-duplicate refusals; existing compiler parity18 negatives; existing embedded5 negatives. Все прошли, 5 named one-pass summaries. После test export correction повторён только один affected permutation/export test. Четыре empty aggregate invocations каждого stable check command не исполняли другие selected tests.

Final PS branch `test-migration-s54-admission.ps1 -CompilerFilenameOnly` прошёл14 cases:3 filename-order acceptances + те же11 identity/provenance refusals на fresh real-shaped public fixture. Тесты используют actual shared comparator. Не повторялись transcript/purpose/HEAD/full path matrices и production preparation.

Сохранён failed ps-01: export index был записан до дальнейших изменений той же synthetic fixture, поэтому index pin закономерно перестал соответствовать ей. Коррекция создаёт отдельную fresh fixture для export; старые index/data/source/work snapshots не переписывались и не rebinding-ились. Failed ps-02:14 fixture cases прошли, затем full real-data PS compiler audit встретил обязательный live-source freshness refusal. Final audit проверяет actual representation helper отдельно и явно требует оба freshness refusals; product guard не изменён. Routine failures относятся к test administration/audit expectations, не к новой production попытке. Исторический compile cargo.json phase tag не является proof результата; authoritative before/after markers и raw original compiler snapshot перечислены выше.

## Preservation and submission boundary

Final seal:265 deduplicated immutable production/history pins verified (F04 nine images/seven origins/seven dep-info/public/admin/captures, L/M reports/manifests, F01–F03 immutable history/H slot); разрешённый mutable K source-check cache исключён из immutable set. Source closure520; вне пяти source allowlist paths исходники не менялись. HEAD тот же/index пуст, root navigation сохранена, successful native relevant process inventory0. F05/P allocations отсутствуют; F04 original eight runtime failure artifacts неизменны. Private profiles/keys не читались/не создавались. Deadline/workload/crypto/storage/messenger/admission/guard rules не менялись.

Exact candidate:5 source/test paths + неизменённые L preparation report/pins и M failure report/pins + этот report/pins, всего11 regular100644 paths. Manifest, before/after outputs, minimal source diff, original artifact pins и normalized Git blobs — в fresh N administration. Большие binaries/captures не копировались для отчёта. Root task/review/navigation вне executor allowlist. Commit/push/production Prepare/F05/activation/Run/P не выполнялись. После одного callback этап остановлен для архитектурной приёмки; следующая production попытка этой задачей не разрешена.
