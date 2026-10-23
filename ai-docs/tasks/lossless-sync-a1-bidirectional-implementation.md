# OD1 bidirectional: implementation repair 1

2026-10-23. Вторая сдача кода по [review R1–R4](lossless-sync-a1-bidirectional-implementation-review-1.md).
**R1–R4 проверены; требуется повторное независимое review. Код не принят, не закоммичен.**
Основание: [задание](lossless-sync-a1-bidirectional-task.md) и
[принятый контракт](lossless-sync-a1-bidirectional-contract.md). Wire/schema не менялись.
Продолжение прямого поручения человека `01a1661b-1561-7ec0-9380-b2835fd35738`.

## Зафиксированные вход и candidate

Documentation baseline/HEAD/проверенный remote master:
`47d19c41be449deea9c1af003f7a27cb6992710c`; product baseline `dcc1300b759d45b0dfbb77e08b93d2cac0957b80`.
Новых commit/push нет, index пуст. [Повторный read-only ls-remote](lossless-sync-a1-bidirectional-evidence/repair1-remote-verification.log) подтвердил тот же master.

До repair сохранены точные неизменяемые [iteration1 inputs](lossless-sync-a1-bidirectional-evidence/iteration1-review-input/implementation.md):
report `f7c8f6d5cee91f23eafaf3e2deef72f339190d47196fc1460ef394aeb39699db`,
patch `a392d15d9102774799d22793845fb224105e7b931058ab1b034cb0f5ea4ce697`,
source manifest `eb15e80cc799fb112777d91d309302141ac634685e2195a99750cb8349df099d`,
final checks `ab72312d5cb8ab60c83b26af137d056371f0c74cb43438060943a8ab20ec637a`.
Архивные bytes не обновляются при новой сдаче. Также сохранены старые performance/index.

Review1 EXE сохранён в `target/compatibility/ordered-dialog-review1/kilogram-cli.exe`:
`532a5a76e836faacca63a71306ba10f1129718e7b0aa157e363f58bf7642ce93`.
Настоящий pre-implementation baseline остался в `target/compatibility/ordered-dialog-baseline/kilogram-cli.exe`:
`df4ae0d4760c85eb41ab833655061875472e4054295844d452137859f7d4c58c`.
[Old source/build provenance](lossless-sync-a1-bidirectional-evidence/old-baseline-manifest.json) сохранён.

Финальный candidate `target/field-tests/ordered-dialog/kilogram-cli.exe`:
`3f42245b899e678c423b3137b5d99dc225cb0c2bc36d99b5bfa99daf506a6626`.
[Build](lossless-sync-a1-bidirectional-evidence/repair1-build.log) и
[повторная сборка с совпавшим field EXE](lossless-sync-a1-bidirectional-evidence/repair1-build-confirmation.log).

| Артефакт | SHA-256 |
|---|---|
| [Адресный repair diff:7 файлов](lossless-sync-a1-bidirectional-evidence/repair1-code.patch) | `2bdf56c2b3858ad3c9dfbf376dce2635e0d0f981224616bea233dc5b5d4b997c` |
| [Полный code diff:30 файлов, включая untracked](lossless-sync-a1-bidirectional-evidence/review-code.patch) | `c6c7d6b42e421bb7ca86860274defa914d611d8c705032c4a7e82209e76fa13a` |
| [545 current source hashes](lossless-sync-a1-bidirectional-evidence/frozen-source-manifest.json) | `602b4982ef739ba68315a0e17ef480611f12d4218e4743efe075987d6aa8d4af` |

Оба patch прошли `git apply --check --reverse` против текущего worktree.
[Финальная сверка artifacts/source/архивов](lossless-sync-a1-bidirectional-evidence/final-review-checks.json).
Пять продуктовых модулей: handshake/engine/outbox/runtime и state/vault/ordered;
ещё два файла — cfg(test) регистрация и отдельные review regressions.
Нет новых CLI verbs, records, wire fields или общего refactor.

## R1–R4: изменение и доказательство

[Baseline regression log](lossless-sync-a1-bidirectional-evidence/repair1-regression-baseline.log):
**0 PASS/4 expected failures**,816.71s. Product source соответствует сохранённой iteration1;
перед compilation добавлены только regression tests.
[Точный тестовый patch](lossless-sync-a1-bidirectional-evidence/repair1-regression-baseline-tests.patch) и
[provenance/harness hash](lossless-sync-a1-bidirectional-evidence/repair1-regression-baseline-provenance.json)
позволяют воспроизвести это в отдельной копии baseline47d19c4 с archived full patch.
Правки продукта начаты уже после compilation baseline harness; его загруженный EXE не заменялся.

| Замечание | Baseline | Repair и адресная проверка |
|---|---|---|
| R1 | Valid renewal до Offer: `initial Hello scope/role` | Только первый выбор A требует exact current ConsentRef. Historical pair требует одинаковых refs в обоих Hello, C/devices/roles/membership/credentials, обе Hello signatures, T и обе Device Proofs, включая собственный proof прошлого проверенного admission. Каждый вызов требует fresh current lease/Root policy; peer signature отдельно недостаточна. Traces renewal до Offer, после Offer/Grant/Active, по два renewal и reopen, включая старый Offer до первого Grant. Wrong current consent, несовпадающие Hello с привязанным к ним Offer, испорченный proof и неверная Root signature отвергаются |
| R2 | Fence-only root усыновлён: `[false,true]` вместо двух отказов | `create()` fail closed при любом существующем fence без manifest/claim; ничего не стирает. `open()`/enrollment после durable manifest проверяют прежний owner и используют saved keys. Same/different owner до manifest с/без DB получают CreationConflict; после manifest чужой owner отказан, свой завершает enrollment с теми же публичными signing/encryption keys и exact claim. Normal enrollment/reopen и реальные old/live fences повторены. Это component snapshots выбранных interrupted windows, не новый process kill/power-loss acceptance |
| R3 |511 настоящих Text; own.next512; новый enqueue успешен (`new_request_upgrade=false`), IPC/reopen не сообщают UpgradeRequired | Остаток lifetime учитывает genesis и уже принятую queue. Новые requests за capacity получают UpgradeRequired; idempotent queued/committed retries проверяются раньше лимита. IPC и maintained actor показывают UpgradeRequired; materialization останавливается при512, connection/receive/repair не закрываются. Fixed:511 Text через настоящие native/vault/queue/receive/receipt операции; статус UpgradeRequired до и после reopen. Отказ при двух зарезервированных последних slots, queued/committed retry, сохранность старой принятой sealed queue, opposite lane receive, history512, exact ciphertext retry, prefix serving и ACK прошли |
| R4 | Self-consistent native Init с неверным request16 принят | Init/Confirm проверяют request16==OfferId/GrantId prefix и пустые parents до native decrypt. Четыре negative vectors: real native encryption с согласованными Core/Inner, правильной Device signature, но неправильным request либо parents. Native blob не меняется; исходные Init/Confirm после отказов принимаются |

Historical verification R1 реализует §5 контракта: собственный сохранённый Device proof
подтверждает прошлый admission. Он не переопределяет current consent/authority и не
создаёт неограниченный consent archive. Selected pair/Offer/Grant/T/E/SID не заменяются.
Нормативная wire/schema развилка не потребовалась.

[Fixed component log](lossless-sync-a1-bidirectional-evidence/repair1-regressions-fixed-attempt1.log)
и [его provenance](lossless-sync-a1-bidirectional-evidence/repair1-fixed-component-provenance.json): **4/4 PASS**,828.80s.
Нативные секреты/guard/pickle contents не печатаются; assertions сравнивают public keys,
opaque equality и реальные последующие операции.
Fixture lease в component проверке обозначен явно и **не является TLS acceptance**.
Полная R3 проверка выполняет настоящие queue/materialization/receive/receipt commits,
а IPC boundary вызывается через production `runtime::ipc_request`.

После compilation этого harness строгий Clippy нашёл два unwrap в тестовой подготовке.
Исправлено только propagation ошибок/sort key acquisition, без product changes;
[точный test-only diff](lossless-sync-a1-bidirectional-evidence/repair1-test-lint-only.patch) сохранён.
Весь product EXE после этого совпал byte-for-byte.
[Финальные быстрые components на текущем source](lossless-sync-a1-bidirectional-evidence/repair1-final-fast-components.log): **6/6 PASS**,40.52s — существующие guard/phase/native3 и R1/R2/R4. R3 не повторён после двух test-helper lint edits; его полный run уже прошёл на том же product source/EXE, точный test-only diff указан выше.

## Реальные процессы и применимость прежнего evidence

[Duplex64](lossless-sync-a1-bidirectional-evidence/repair1-duplex64.log) на candidate3f42245b:
два CLI/runtime, настоящие owner/device credentials/двойные Root consents,
Iroh QUIC/TLS exporter и native Olm;32 Text в каждую сторону, по8 receiving DH transitions,
одинаковый SID, exact IDs/request IDs/UTF8, pending0/unacked0.
[Reopen тех же профилей](lossless-sync-a1-bidirectional-evidence/repair1-reopen64.log):64 exact
records и272 immutable/native hashes совпали, graceful authenticated Shutdown.
[Old/current CLI и live old owner fences](lossless-sync-a1-bidirectional-evidence/repair1-fences.log):
protected records не изменились, fresh-only refusal без migration.
Это компактный regression gate; повтор всех256 сообщений не заявляется.

[Storage3 PASS](lossless-sync-a1-bidirectional-evidence/repair1-storage.log),
[locked/offline strict Clippy](lossless-sync-a1-bidirectional-evidence/repair1-clippy.log),
rustfmt и diff whitespace. Ошибка PowerShell argument forwarding и первые test lints
сохранены отдельно; они исправлены, не объявляются продуктовыми failures/PASS.

Новая сдача содержит [50 process archives с fingerprints](lossless-sync-a1-bidirectional-evidence/process-run-index.json).
EXE/profiles/IPC bearer/native pickles в архив не копируются; только allowlisted public
results/traces/synthetic histories/stdout/stderr. Input1 archive остаётся неизменным.

| Старое evidence | Что оно по-прежнему доказывает |
|---|---|
|532a5: [Duplex256](lossless-sync-a1-bidirectional-evidence/accepted-candidate-duplex.log), [reopen1040 hashes](lossless-sync-a1-bidirectional-evidence/accepted-candidate-reopen-attempt2.log), [C3Partial](lossless-sync-a1-bidirectional-evidence/accepted-candidate-partial.log) | Полный prior acceptance; native guard/ingress/drain/repair/receipt/history не изменены. Это не новый256 acceptance на3f42245b |
|532a5: [root independent Gap](lossless-sync-a1-bidirectional-evidence/root-review1-gap.log), missing-auth/cross-context | Full64+late DH+kill/reopen и typed input admission на точном review input. Семантика этих модулей unchanged; новый handshake отдельно покрыт выше |
|f32c6650:14 crash points,2 contexts,B-only,lease/locator/revocation/expiry,control fork,unsupported suite | Исторические process gates, fingerprints сохранены. Новые interrupted-enrollment и send-capacity seams закрываются адресными repair checks, не приписываются старым прогонам |
|Protocol/native/transport/guard/storage bounds, I1–I4, ordinary IPC/history | Непереписанные ветки сохраняют свои прежние проверки; storage/old fences и существующие actor components повторяются адресно. Полный corpus не повторялся |

[Performance observations](lossless-sync-a1-bidirectional-evidence/performance.json): compact64
setup5.64s/exchange59.54s/history3.93s,total69.11s;317 commits,85 native,commit p50/p95=47/99ms.
Reopen0.88s,history/reauth3.88s. Прогон шёл параллельно R3; это не controlled benchmark.
Baseline R3 engine-boundary511 занимает816.71s: многократные native/control signature checks
и durable transactions в unoptimized verification; не per-message network latency.
Full iteration1 performance/ограничения сохранены; daily-MVP throughput не заявляется.

## Воспроизведение и остановка

```powershell
$env:CARGO_NET_OFFLINE='true'
& scripts/invoke-cargo-friendly.ps1 build -CargoJobs 2 --locked --offline --profile verification --package kilogram-cli --bin kilogram-cli
Copy-Item target/verification/kilogram-cli.exe target/field-tests/ordered-dialog/kilogram-cli.exe
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter 'ordered_dialog::repair_tests::' -CargoJobs 2 -Run -NoCapture
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -Filter 'vault::ordered::tests::' -CargoJobs 2 -Run -NoCapture
& scripts/test-ordered-dialog-process.ps1 -Scenario Duplex -MessagesPerLane 32 -Burst 4
# Подставить RunDirectory из последней команды:
& scripts/test-ordered-dialog-reopen.ps1 -RunDirectory <RunDirectory>
& scripts/test-ordered-dialog-fences.ps1 -RunDirectory <RunDirectory>
```

Windows network harnesses запускаются только stable runner; CLI fixed-path, hash по содержимому.
Native token нужен для ACL/DPAPI/loopback. Все builds/checks явно CargoJobs2,
BelowNormal/locked/offline. Housekeeping runner печатает default12, но не компилирует/запускает.
Runtime tests выполнялись с native token. Финальный git ls-remote сначала отказал локально в sandbox (MSYS named object0xC0000022); [диагностика](lossless-sync-a1-bidirectional-evidence/repair1-remote-sandbox-failure.log), повтор с native token подтвердил47d19c4. Это не продуктовый/network failure.

L1 остаётся2 C/profile,1 epoch/C,512 encrypted/lane включая genesis→511 Text.
Нет silent eviction/reset, миграции/default rollout, mailbox/relay/GUI/M1.2.63/A3.
Physical two-host/external networks и все power-loss windows не проверены.
**A2 остаётся gate до внешнего использования.**

Контракт `587cf786…` и iteration1 archive `77c0c69b…` не менялись.
Чужой `ai-docs/development-environment.md` не затронут, SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Адресные проверки завершены. Далее один callback с hashes/evidence и остановка до review.
Продуктовый commit/push только после явной приёмки и поручения оркестратора.
