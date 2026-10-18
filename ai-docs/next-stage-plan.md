# Следующий большой этап: M1.1.77–M1.1.80

Статус: запланирован, выполнение ещё не начато.

Цель: за один длительный блок продолжительностью примерно 8–10 часов закрыть
последний технический gate `fresh-hosted-and-field-evidence` для dormant
`kilogram/m1/sync/cbor/2` и получить честное состояние
`ready-for-explicit-activation-decision`. Сам этап не меняет transport по
умолчанию, не удаляет Postcard v6 и не разрешает fallback после ошибки уже
выбранной canonical-сессии.

## Исходная точка

- точный runtime/protocol baseline: commit
  `cc724c908dc3774c87071a523991f7ef79a81335`; последующие roadmap-only commits
  не меняют его protected runtime surface, а field revision будет заново
  зафиксирован после реализации evidence lane;
- восьмичасовой decoder campaign M1.1.76 завершён: 469 853 771 iterations,
  0 artifacts, coverage profile проверен;
- все локальные activation requirements, кроме
  `fresh-hosted-and-field-evidence`, отмечены verified;
- active transport остаётся Postcard v6 / `kilogram/m0/sync/8`;
- `/cbor/2` выделен и полностью реализован как dormant candidate, но не
  зарегистрирован и не dial-ится;
- первый push runtime baseline выполнен 2026-10-18; Windows CI run
  `39087571586` был `in_progress` во время составления плана. Итоговым hosted
  evidence считается только новый green run exact implementation commit,
  созданный после завершения пакетов 1–3.

## Рабочие пакеты и ориентировочное время

### 1. Hosted baseline и точный evidence contract — 1–1.5 часа

1. Дождаться результата Windows CI для `cc724c9` и сохранить exact run ID,
   revision и conclusion.
2. При failure сначала локализовать и исправить только воспроизводимую причину;
   любой code change требует нового clean push и нового hosted run.
3. Определить отдельный versioned manifest свежего evidence: exact source tree,
   toolchain, бинарные SHA-256, роли, transport identifiers, route, run IDs,
   counts, restart/rollback results и отсутствие fallback после выбранного
   `/cbor/2`.
4. Добавить fail-closed verifier и negative self-tests. GitHub-hosted evidence
   и physical-device evidence должны оставаться двумя различными объектами.

Контрольная точка: hosted baseline зелёный, schema не допускает подмену
revision, бинарника, topology или transport path.

### 2. Явно ограниченная field-активация candidate — 2–2.5 часа

1. Добавить только debug/test evidence lane, который может зарегистрировать
   `/cbor/2` и dial-ить его после уже реализованной взаимной authenticated
   selection ceremony.
2. Не добавлять silent production flag и не менять default: отсутствие exact
   evidence option обязано оставлять процесс legacy-only.
3. Сохранить жёсткое правило: до взаимного выбора возможен downgrade к legacy;
   после выбора canonical любая decode/network/session ошибка завершает попытку
   без Postcard fallback.
4. Подключить durable client/responder adapters, typed store, resume и rollback
   snapshot к реальному Iroh stream, не создавая второй логики состояния.
5. Добавить focused Rust tests для v2/v2, v2/legacy, restart-resume,
   post-selection failure и one-step rollback.

Контрольная точка: network-capable candidate доступен только через явно
ограниченный evidence path; обычный CLI/runtime остаётся byte-for-byte по
поведению legacy-only.

### 3. Автоматизированный hosted experiment — 1–1.5 часа

1. Запустить два свежих процесса на GitHub-hosted Windows runner с отдельными
   state directories и тестовыми identities.
2. Проверить exact ALPN `/cbor/2`, двусторонний sync, одинаковую историю,
   restart-resume и отсутствие plaintext в evidence.
3. Отдельно выполнить mixed-version control и negative post-selection control.
4. Загружать только небольшой manifest/log summary без state, secrets, corpus,
   release EXE или ZIP.

Контрольная точка: fresh hosted regression принят verifier-ом, но явно не
считается physical-device evidence.

### 4. Двухмашинный Windows field run — 2–3 часа

Достаточно основного ПК и ноутбука. Используются свежие test-only state
directories и debug binaries с одинаковым SHA-256. Ноутбук должен быть включён,
не спать, быть доступен по OpenSSH; VPN лучше отключить на обоих устройствах.

Матрица:

1. direct LAN: v2/v2, bidirectional history convergence;
2. остановка одного процесса между durable commit и продолжением, затем resume
   без потери или дублирования события;
3. relay-only control через один pinned relay, если relay доступен;
4. v2/legacy: согласованный legacy path до canonical selection;
5. испорченный/оборванный selected-v2 attempt: fail closed, без legacy retry;
6. explicit one-step rollback с неизменной пользовательской историей.

Каждый процесс пишет в локальный закрытый log и после завершения закрывает
handle; controller собирает immutable copies, hashes и bounded summary. Не
использовать Yandex Disk как coordination transport и не переиспользовать
старые state directories.

Контрольная точка: два физических Windows host подтверждают один exact commit
и exact binaries; direct или relay path подтверждён реальной Iroh telemetry,
истории совпадают, restart и rollback доказаны.

### 5. Формализация, regression и commit — 1–1.5 часа

1. Проверить hosted и physical manifests новым verifier-ом.
2. Обновить canonical migration/transport/coverage ledgers: последний evidence
   gate становится verified, но `changes_active_wire=false`, candidate
   `registered=false` и `dialed=false` остаются production defaults.
3. Зафиксировать отдельный RFC с результатами и остаточными рисками.
4. Прогнать affected boundary self-tests, полный readiness, rustfmt, locked
   offline strict Clippy и bounded stable Rust regression.
5. Закоммитить только при чистом дереве и зелёных проверках. Push выполнять
   отдельно после проверки commit-а.

Итоговое состояние: `ready-for-explicit-activation-decision`, а не
`activated`. Следующий отдельный этап после человеческого решения сможет
перевести default в `dual-reader-prefer-legacy`; удаление legacy и переход в
`cbor-required` в этот блок не входят.

## Stop conditions

- hosted и physical evidence относятся к разным source revisions;
- бинарные SHA-256 на ПК и ноутбуке различаются;
- field run использовал старое или частично заполненное state directory;
- selected canonical attempt незаметно перешёл на legacy;
- evidence зависит от открытого/несинхронизированного файла;
- любое изменение runtime/protocol после field run инвалидирует evidence и
  требует пересборку и повтор именно затронутых прогонов;
- обнаружена потеря, дублирование или изменение уже сохранённой истории.

## Ресурсная политика

- debug builds, без release/ZIP;
- Cargo jobs 2 по умолчанию, normal или below-normal priority, чтобы ПК оставался
  отзывчивым;
- существующий target cache сохраняется;
- не повторять восьмичасовой fuzz campaign, если decoder/toolchain/seed surface
  не менялись;
- никаких фоновых OS services, scheduler tasks, tags или releases.
