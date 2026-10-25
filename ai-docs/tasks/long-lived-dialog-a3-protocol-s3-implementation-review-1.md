# A3-S3: независимое review реализации и приёмка

2026-10-25, Asia/Yekaterinburg. **S3 component scope принят.** Блокирующих замечаний
по проверенному diff не найдено. Разрешена публикация только exact accepted manifest;
production activation и следующий этап этим документом не разрешаются.

Baseline: `e6f9b8ff6c7e9305d7216a94b142f0220b0c4a25`.
Основания: [implementation task](long-lived-dialog-a3-protocol-s3-implementation-task.md),
[принятый plan](long-lived-dialog-a3-protocol-s3-plan.md),
[contract review2](long-lived-dialog-a3-protocol-s3-plan-review-2.md),
[read-budget amendment](long-lived-dialog-a3-protocol-s3-read-budget-amendment-1.md),
[provenance clarification](long-lived-dialog-a3-protocol-s3-provenance-test-clarification-1.md)
и [time-budget amendment](long-lived-dialog-a3-protocol-s3-time-budget-amendment-1.md).

## Что проверено независимо

Прочитаны новые protocol2/authority/frame/mode, Guard2, restricted state reader,
closed writer и изменения existing managed/read paths; проверены test-only owner,
native continuation/import и основные negative/fault/differential tests. Проверка
проводилась по исходному коду и evidence, не только по итоговому сообщению исполнителя.

- Canonical layouts, domain separation, caps, actual native header, selected Q/certificate
  и genesis request связаны до native trial. Parsed и Verified wrappers имеют private
  fields; Root/static verification не выдаёт current-policy permit. Внутренний plaintext
  проверяется после MAC. Mixed dispatch выбирает один decoder по explicit format.
- Guard2 хранит две bounded lanes, проверяет successors и phase до native operations,
  использует authenticated Seen observations и связывает PreparedStep с exact before
  checkpoint. Apply изменяет копию; ошибка не частично изменяет принятый guard.
- State выводит fields/span/ID из полного Event2. Только HPB1 genesis request
  нормализуется в zero sentinel; raw source и подписанный Core не изменяются.
  Сохранённые source/projection, прямые parents, Seen first, first/tail и checkpoint
  проверяются через bound2 correspondence; отсутствующие/повреждённые обязательные
  значения не подменяются hints или проверкой одного hash fragment.
- O/A/F/R разрешаются по exact bundle hash и static certificate role. Peer Pending
  атомарно сохраняет original/admission captures, readable — F и immutable receipt,
  ACK не заменяет исторический R текущей revision. Exact last-commit retry также
  повторяет persisted evidence checks. Generic и старые trusted writers остаются
  закрыты для управляемого bound2 состояния, кроме согласованных Queue/Repair операций.
- Native/Seen/source/projection/Auth/credits/F/receipt effects проходят общий atomic
  writer с bounded rows/plan, immutable records, H1/H2 и shared Author ownership.
  Role→Author slot устанавливается trusted owner при init; этот mapping не является
  доказательством certificate authenticity внутри structural state crate.
- Bound2 page ceiling `40*(limit+2)+3`, legacy `8*(limit+2)+2`; выбор по authenticated
  view не добавляет скрытые reads после снятия statistics. Parent traversal остаётся
  ограниченным прямыми parents. Root cardinality дополняет exact evidence проверки.

Проверенные границы доверия: state не проверяет Root/Device signatures или native MAC;
pure L1 facts/import не удостоверяют происхождение произвольного pickle. Эти обязанности
отделены и явно описаны. Тест unsupported config2 проверяет согласованный owner-origin
seam до open/import, а не недоступный в pinned build actual config2 SessionPickle.

## Source/evidence correspondence

[Отчёт исполнителя](long-lived-dialog-a3-protocol-s3-implementation.md): 27561 bytes,
SHA256 `fa364c666b4e5a94a07381afbfc82d3e28acde7317b445b4da9f8e8f65cd4831`.
[Artifact manifest](long-lived-dialog-a3-protocol-s3-evidence/artifact-hashes.json):
SHA256 `99c97032c7d2f1ead5407c30acba279d0afab4b36a675c3bd8180bd790408408`.
Root независимо пересчитал **все 182 listed files**, mismatches=0; сам manifest проверен
отдельно. Его файлы сохранены без изменений и после независимого теста root.

Исходная сдача содержит 25 source/data paths. Между N01-final и окончательным source
snapshot изменены ровно два test-only файла: `protocol2_failure_tests.rs` (funded G02)
и `ordered_dialog_v2/tests/golden.rs` (actual getter assertions). Production dependencies,
shared helpers и N01 driver byte-identical. N01 CLI lib имеет старый hash; отсутствие
его перезапуска не маскируется hash более нового EXE. В N01 protocol unit harness
исполнял 0 selected tests. N02/bounded/targeted regressions относятся к final source.

Frozen plan/reviews/plan accepted manifest, clean development-environment и baseline
compatibility EXE сверены отдельно: 6 protected artifacts, mismatches=0. Cargo files,
lockfile, native byte adapter, Codec1, Guard1/PrivateState и HPB1 predicate/encoding
не изменены. Прочие runtime activation paths не добавлены. Frozen S1/S2 DB root
не открывал и не запускал. До публикации index пуст, HEAD остаётся baseline.

Root добавил один самостоятельный regression test и отдельные evidence ниже.
Это 26-й source/data path в итоговом accepted change; он не меняет выводы о source
provenance прежних 55 tests. Старые manifest/report исполнителя не переписаны.

## Проверки и самостоятельный native probe

Изучены outcomes и содержание matrix: **25 S3 + 30 selected existing regressions =
55 top-level PASS**, 12 final runner commands exit 0. Вложенный cursor child не
считается вторым top-level тестом. Strict Clippy all-targets `-D warnings`, fmt,
helper-rustfmt, whitespace и script syntax в сдаче PASS; промежуточные FAIL сохранены.
Root не повторял весь набор или heavy 100k только ради приёмки.

Добавлен [independent native/Guard2 probe](../../crates/kilogram-ratchet/tests/a3_s3_root_review.rs):
32 воспроизводимых schedules с OS-random native keys, разными задержками доставки
в обе стороны, несколькими queued packets и public snapshot/restore между ними.
Внутри каждой lane доставка FIFO, как требует ordered ingress; это не тест network
reordering/recovery. Проверяются фактические native key/index transitions, plaintext,
конечные own↔peer lanes/Seen, неизменность SID/initial keys после restore.

Результат: **1 PASS**, sent=1073, received=1073, successful message calls=2146,
node restores=704, sending DH rotations=203, max pending per lane=16.
Harness 1.60s (scenario 1.599970s), runner 25.1146726s. No native private fields,
новые session/OTK при продолжении, listeners или реальные пользовательские данные.
Это component evidence; оно не заявляет проверку application protocol/current policy,
storage durability, Live Active2 или two-process communication.

Evidence: [лог](long-lived-dialog-a3-protocol-s3-root-review-evidence/native-schedules-1.log),
[result/source/EXE hashes](long-lived-dialog-a3-protocol-s3-root-review-evidence/native-schedules-1-result.json),
[reproduction script](long-lived-dialog-a3-protocol-s3-root-review-evidence/run-native-probe.ps1).
Windows harness только через stable runner, который компилирует `--no-run` и запускает
fixed EXE paths; verification/unoptimized, locked/offline, Jobs12/BelowNormal.
Дополнительный strict Clippy all-targets kilogram-ratchet, rustfmt нового файла,
diff whitespace и PowerShell parser также PASS:
[static result](long-lived-dialog-a3-protocol-s3-root-review-evidence/static-1-result.json).
Test SHA256 `d680bee8cda588c44613e332bf4a61fa81d4b965c8629fe8ecb524d97afb9aa4`.

Итого в accepted evidence **56 top-level PASS**. Root пересчитал сумму исходных
12 runner commands: 1117.3353278s. С самостоятельным runner: **1142.4500004s**,
то есть около 19 минут 2 секунд, ниже явного нового aggregate ceiling 1200s.
Это сумма отдельных замеров; static/Clippy отдельно. Исходный 600s aggregate
не выполнен и не объявляется выполненным. Каждый N01/N02 отдельно ниже 600s.

## Приёмка и оставшаяся граница

Принята именно библиотечная/component реализация S3. Findings, требующих repair,
на этом review нет. Это не независимый внешний криптографический аудит и не
утверждение, что потенциальных ошибок больше нет.

Не завершены whole A3 и пользовательский MVP: production current-policy owner,
Active2 ceremony, runtime actor/ALPN2, offline scheduling/recovery, реальная L1 file
migration/live handoff и whole-A3 two-process/2C acceptance остаются впереди.
Read-bound поправка не доказывает приемлемую пользовательскую latency; внутренний
CPU/crypto/IO bottleneck не профилировался. Старые S2 performance numbers не переносятся
автоматически на новый bound2 reader. Эти ограничения сохраняются после публикации.

Следующее разрешённое действие исполнителя — один commit принятого manifest с exact
baseline parent и обычный non-force push `origin/master`. До этого сверить raw SHA256
и normalized Git blobs, stage только manifest paths плюс сам manifest; после commit
проверить exact changed paths/blobs. Root отдельно проверяет actual remote и published
tree. Не менять evidence, не добавлять post-publication status files, не начинать
следующий implementation этап. Постоянное разрешение пользователя на публикацию
принятых логических изменений действует; повторное human permission не требуется.
