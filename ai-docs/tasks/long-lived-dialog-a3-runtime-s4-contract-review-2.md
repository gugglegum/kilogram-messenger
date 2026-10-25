# A3-S4 contract review 2: контракт принят

2026-10-25, Asia/Yekaterinburg. Baseline
`7f82036ee5803edd64be98b73572604250f5961c`.
Проверен исправленный [контракт](long-lived-dialog-a3-runtime-s4-contract.md):
97748 bytes /952 lines, SHA256
`b1c4f5ee185ce0b719a47a624b00ea45649a341cc3044ba3c62e34c887f6f906` до root status edit.
[Review1](long-lived-dialog-a3-runtime-s4-contract-review-1.md) остаётся неизменным:
13626 bytes, SHA256 `f2631bb7a48de92a06a205f2c0d016c719f37acc978c944ce9aeccc9e298d9c1`.

**R1–R3 закрыты на уровне контракта. S4 integration contract принят.**
Это приёмка спецификации и будущей matrix, не ещё не написанной реализации.
Сейчас разрешена публикация exact документации; product implementation поручается
отдельно после root проверки опубликованного diff.

## Повторная проверка замечаний

**R1 — закрыто (§4.1.1/§10/F10).** Выбран runtime-only authenticated plaintext
prefix `06 || "KGOD2RT\0" || u16BE(1) || Body5`; ComponentRoot5 сохраняет `05||Body5`.
Root сверил исходный `Root::decode`: S3 действительно отвергает06 до разбора Body5
и до возврата writable vault. Outer schema5 sentinel, envelope/key/AAD неизменны,
поэтому это осознанное local-root различие, а не ненамеренная смена wire/schema.
Первый committed runtime root уже имеет новый prefix и marker, bootstrap не оставляет
временный writable component-compatible root. В новом коде typed loaders и guards
разделяют profile types до retry; reserved slots и unknown revisions fail closed.

F10 проверяет actual predecessor7f82036 с correct owner/config и positive component
controls, а также same-version public/internal bypass. Это точнее одного d9b V4
EXE. Будущий helper использует exact старые library sources/lock/features и отдельный
fixed path. Его не собрали/не запускали во время contract review. Discriminator
закрывает accidental old-version open, не обещает защиту от local code с секретами.

**R2 — закрыто (§8.3/P08/F11).** Выбран ephemeral transport endpoint на каждый
restart/rebind. Durable старый locator служит revision high-water; новый endpoint
получает checked newer revision и atomic commit до export. Exact export допустим
лишь для живого того же current instance. Повторные kills до export не рекламируют
умерший endpoint и не повторяют native constructor. В F11 одновременно проверяются
два kills, actual ID/контролируемо другой port, monotonic revisions, отказ stale
locator и доставка уже materialized B-only debt после импорта актуального адреса.
Account/Device/D/E/SID/Q и ciphertext сохраняются; operator exchange остаётся
ограничением этого этапа, автоматическое глобальное обнаружение не заявляется.

**R3 — закрыто (§8.1/§8.2/F12).** Runtime использует Minimal и explicit transport/
lookup configuration; N0/public fallback не наследуется из общего helper. Таблица
None/Some для трёх route policies конкретна; DirectOnly+Some сознательно отвергается
в локальном S4 scope. RelayOnly не возвращает IP transport через bind_addr. Runtime
locator допускает URL-only и отвергает empty/empty; canonical arity10 строго отделена
от неизменных S3 nine-field vectors. Проверка actual route/exporter сохраняется.
Root сверил pinned Iroh1.1.0 clear/config APIs и default secret generation; будущий
P/F использует этот production builder, не отдельную test-only конфигурацию.

## Остальные решения и граница

Приняты fresh production S4 и обязательный successor S5 для existing L1 migration/
handoff, namespaces21/22, closed atomic runtime-purpose operations, strict current
clock/high-water, before-SID NeedHandshake для новых requests и historical consent
support-artifact import без смены current pointer. Deferred gates A3-1/7/8 остаются
видимыми; whole A3 и MVP не закрыты.

Уточнения review1 включены: profile-total connection permits захватываются до TLS/
application spawn; admission/streams/mailbox имеют deadlines/bounds. При двух C
scheduling slots разделены64/64 без borrowing и освобождаются независимо от Delivered;
P07 наблюдает C2 progress при задержанном C1, durable debt не удаляется.

Post-staging gate rejection отделён от IO ambiguity: explicit successful tx.abort
сохраняет accepted logical state и usable repository. Root сверил `redb4.2.0`
`WriteTransaction::abort`: API возвращает Result и проверяет IO errors. Ошибка abort/
backend или неопределённый начатый commit требует poison/reopen. Physical staging
bytes/growth и уже выполненные trial calls не объявляются нулевыми.

Нормативны все P/F/bounded regression gates, включая новые F10–F12 и14 F01 cases.
P ожидает1640 Text+4 genesis,3288 успешных новых native packet calls; retries/aborted
attempts отдельно. Profile verification (без S2 scale/release override), fixed
Windows runners/EXEs, Jobs12/BelowNormal. Ceilings: P2400s, F1200s, components600s;
это суммы final runner elapsed, static/compile-only отдельно. Они не являются
пользовательской latency. Изменения scope/profile/budgets требуют отдельного root
review, а не сокращения counts после FAIL. Heavy S1–S3 evidence reuse ограничен
действительно неизменными source/driver/seams.

## Выполненные проверки и публикация

Root прочитал исправленный текст и сверил ключевые решения с actual source: Root
encoding/open/strict decoder, component mutation guards, pinned Iroh builder и
redb abort. Baseline/review hashes и отсутствие product diff проверены. При одной
документации tests/benchmarks/сетевая matrix не запускались; runtime PASS не заявлены.
Root меняет в контракте только статус/ссылку на это review; accepted final hash и
normalized Git blob будут в publication manifest. Старые review/evidence frozen.

Следующее поручение исполнителю — один documentation commit с exact parent
7f82036… и обычный non-force push origin/master. Exact allowlist: README,
chat-coordination, S4 task/contract/review1/review2 и accepted-files manifest.
Перед stage сверить raw SHA256/bytes и normalized Git blobs; после commit — exact
changed paths/blobs/parent. Не менять evidence и не добавлять post-publication status
files в эту публикацию. Root независимо проверяет remote/tree, затем поручает
implementation по принятому контракту. Обычный commit/push уже разрешён пользователем.
