# Долговечная личная переписка: продолжение OD1 и большая история

Дата: 2026-10-24, Asia/Yekaterinburg. Статус: разрешён этап; первое поручение —
только контракт на review. Продуктовая реализация начинается после приёмки контракта.

## Разрешение, исполнитель и baseline

Прямое human userMessage `01a16bf9-89cd-7cb3-8661-36eef244e0fc` в чате root
`01a164ec-589c-7b41-bc34-3ba158d1f22b`, host local:
«Передай этот этап прежнему исполнителю OD1 на GPT-6.1 Sol Max, без Fast.
Сначала проверь контракт, затем реализацию. При необходимости запускай циклы
исправлений, после приёмки — commit и push».
Это разрешает весь описанный этап, последовательные review/repair cycles и обычные
commit/push принятых частей. Исходные прямые разрешения на substantive callbacks:
`01a1657e-b03f-7d01-aaf0-e5af979125fd`, `01a16568-4515-7420-b3af-32d6fc0bfeb4` того же root.
При необходимости подтвердить human источник через read_thread, а не считать
межчатовое сообщение самостоятельным разрешением человека.

Исполнитель — прежний чат **«Подготовить baseline lossless sync»**,
`01a1653c-0154-7611-8e8c-044fbe1e4d7a`, local. GPT-6.1 Sol / reasoning Max,
service tier default; Fast/priority не включать. Новые чаты/субагенты не нужны.

Baseline HEAD `860b351fb747562e1fe12d469b1ad053217a7c0f` уже отправлен в origin/master.
После твоей последней OD1 работы другой исполнитель обновил Iroh и совместимые
зависимости, добавил обычный local relay test, advisory gate и mixed-version process
harness. Root независимо проверил этот commit и все82 accepted blobs. Не возвращаться
к dependency snapshot bc86cd2. Исходная OD1 реализация остаётся принятой после R1–R4.

Минимальный dossier: AGENTS.md, этот task, [OD1 contract](lossless-sync-a1-bidirectional-contract.md),
[его implementation review2](lossless-sync-a1-bidirectional-implementation-review-2.md),
[A2 review](transport-security-a2-implementation-review-1.md), раздел P1
[handoff](../project-handoff-2026-10-23.md). Остальные документы читать адресно.

## Пользовательский результат и границы

Один и тот же личный диалог продолжает работу за нынешним пределом511 Text/направление;
давние сообщения остаются читаемыми, история доступна постранично, после offline и
restart возвращаются пропущенные сообщения без потери accepted данных и дублей.
Размер истории сам по себе не требует создания пользователем новой беседы.

Работать на принятом OD1. Сейчас L1 ограничивает512 encrypted frames/lane с genesis,
одну epoch/context и резервирует полный lifetime budget; native guard, source ledger,
manifest/index и vault также имеют связанные ограничения. Простое увеличение LIFETIME
не является решением. Новый контракт должен заменить связанные lifetime assumptions,
сохранив проверяемые bounds операции, очереди, памяти, commit/staging и места на диске.
Конечный диск не объявлять бесконечным: исчерпание ресурсов имеет явный безопасный исход.

В scope остаются два независимых личных contexts, два существующих single-device
участника, operator-assisted locators, локальные runtime процессы и штатные direct/
relay пути. Самостоятельный onboarding, multi-contact listener, GUI, группы, каналы,
multi-device, mailbox lifecycle, device replacement, dormant M1.2.63 и public rollout
не входят. Старый legacy inventory cap4096 не объявлять устранённым, если legacy path
не менялся. Это продолжение нового message/history backbone, не весь A1/A3/MVP сразу.

## Первая сдача: один реализуемый контракт

Создать `long-lived-dialog-a3-contract.md` в этой папке. Нужен компактный основной
документ с одной рекомендованной схемой и её точными правилами; таблицы/приложение
использовать для необходимых layouts/traces. Не запускать новую бесконечную RFC-серию.
Сравнить существенные альтернативы кратко; не выбирать заранее epoch rollover только
потому, что это позволяет сохранить старую константу. Решение должно показывать
пользовательскую непрерывность, ограниченность затрат и сохранность старых данных.

1. **Связанные ограничения.** По коду перечислить lifetime/in-memory/durable/wire
   зависимости: ratchet guard, replay/dedup, source ledger, inventory/range, history
   index, manifest/AEAD metadata, whole-vault rewrite/staging. Для каждого — новое
   правило и реальная асимптотика append/receive/retry/page/reopen. Начать с измеримого
   baseline, избегая повторения уже принятого дорогого corpus без необходимости.
2. **Непрерывность протокола.** Канонические версии/идентификаторы, ordinal/epoch
   semantics, continuation и negotiation. Если вводятся segment/epoch boundaries,
   определить авторизацию и atomic переход, concurrent proposals, обе стороны
   offline, late old messages, old gaps/ACK/retry и restart во всех новых состояниях.
   Новый epoch/reset не может скрыть незавершённую старую доставку или переписать
   identity/history. Native crypto primitives не менять и pickles не клонировать.
3. **Индексируемое хранилище и история.** Append/range/page API, стабильный cursor
   при параллельной записи, replay/fork проверка и readable projection. Durable
   ciphertext и ключи/история остаются пользовательскими. Каждый append/receive/page
   не должен требовать полной загрузки/сериализации всей переписки. Указать отдельно
   стоимость cold reopen, валидации целостности, восстановления и maintenance.
   Garbage collection только для доказанно производных/ненужных transport данных;
   не удалять user-owned history или единственный источник недоставленных bytes.
4. **Bounded catch-up и ресурсы.** Paged/range inventory, общий ordered ingress,
   владение source bytes и старые prefixes. Показать прогресс при полной pending queue
   и большом gap без lifetime-wide prepaid reservation. Учесть полные encoded frames,
   retained metadata, transaction delta, atomic storage staging, head/repair reserve,
   disk-full и отмену. Accepted queue/pending не теряются; ACK только после readable
   commit. Контексты изолированы, кроме явно описанной общей нехватки ресурсов.
5. **Переход существующих OD1 L1 профилей.** Конкретный путь для уже созданных данных,
   в том числе исчерпанной send lane, старой очереди и incomplete ingress. Определить
   explicit consent/upgrade, version fences, live old writer, rollback/restart миграции
   и доступность старой истории. Различать безопасный отказ старого binary и успешную
   совместимость. Нельзя решить задачу только fresh profiles и оставить старые диалоги
   без описанного продолжения. Произвольная legacy migration в scope не входит.
6. **Реализация и приёмка.** Точные затрагиваемые modules/entrypoints, bounded порядок
   логических изменений и матрица проверок ниже. Для каждого свойства — наблюдаемое
   evidence и предел утверждения. Сохранить I1–I4 и R1–R4 либо явно обосновать новую
   эквивалентную гарантию для сменившегося формата. Короткий список решений для root.

До review контракта продуктовый код, dependencies, schema и тестовые fixtures не менять.
Допустимы чтение и небольшие локальные измерения, необходимые для конкретного решения.
Если проверка требует существенного прототипа — сперва описать его вопрос и стоимость.

## Проверяемое завершение будущей реализации

- Реальный зашифрованный двусторонний диалог: минимум640 Text **от каждой стороны**,
  тем самым обе стороны проходят прежнюю границу511; регулярные receiving DH changes,
  точные EventId/requestId/plaintext и отсутствие дублей после restart. Если выбран
  механизм сегментов/epochs, отдельно проверить несколько последовательных переходов
  production state machine; сниженный test budget явно отделить от реального bound.
- Offline catch-up не меньше256 сообщений в одном направлении, несколько страниц,
  reorder и смешение включённых штатных transport paths. Restart между страницами,
  late old-chain messages, lost ACK и exact retry; readable convergence без silent
  fallback/reset и без удаления принятой очереди. Одновременно второй context работает.
- 100000 событий в одной истории: реальный persistent storage/index API, page reads,
  append, reopen и resume. Масштабный storage test можно отделить от полного crypto
  runtime прогона; не выдавать fixture generation за100000 end-to-end сообщений.
  Измерить память, bytes read/written, latency и vault commits на нескольких размерах
  истории; показать отсутствие O(total history) в каждой обычной page/append операции.
- Исполнение с малыми очередями/квотами, заполненным pending и честной нехваткой диска:
  нет ложного Delivered, silent eviction, ordinal holes или потери уже принятого.
  Targeted fault/crash cases покрывают новые durable переходы; graceful reopen не
  заменяет kill/IO-failure evidence. Старые crash windows повторять по затронутости.
- Сохранённый baseline EXE `d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`
  позволяет проверить исходные OD1 profiles и поведение до/после upgrade. Immutable
  identities/history/event bytes должны сохраниться. Старый/живой writer соблюдает
  fences; mixed wire — либо доказанная совместимость, либо явный безопасный отказ.
- Relevant legacy smoke, ordinary transport, strict Clippy/fmt/locked checks. Полный
  corpus не повторять автоматически. Контракт задаёт обоснованный бюджет времени
  проверок; критичные acceptance нельзя заменить только увеличением test counters.

## Ресурсы, ограничения и обратная связь

Все Windows network Rust harnesses — compile no-run и
`scripts/run-cargo-tests-stable.ps1`, fixed EXE names, CargoJobs2/BelowNormal.
Не запускать прямой cargo test и не размножать названия EXE по hash/revision.
Модель GPT-6.1 Sol / Max / default проверяется при старте; Fast запрещён.

A2 supporting update принят и сохранён; весь A2 открыт. Семь warnings остаются
видимыми, strict advisory gate FAIL. Отложенный security reproducer и его материалы
не относятся к этому этапу: не запускать/не отлаживать и не переносить заблокированную
операцию в этот чат. Здесь обычные функциональные проверки собственных локальных
профилей; external/public network readiness не объявлять.

Сохранить чужой `ai-docs/development-environment.md`, исключить из commit.
SHA-256 `330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Исторические accepted manifests/reports/evidence не переписывать. Новые task,
README и coordination изменения root относятся к текущему этапу.

После контракта один содержательный callback в root: path/hash, baseline,
рекомендованное решение, важные tradeoffs, фактические проверки и открытые вопросы.
Остановиться до review. Root ведёт исправления без нового согласования пользователя.
После явной приёмки root поручит commit/push принятого контракта и реализацию.
Далее `long-lived-dialog-a3-implementation.md` и отдельный каталог public evidence,
review/repair, точный accepted-files manifest, commit/push и независимая сверка root.
Не отправлять пустые ACK, не создавать heartbeat и не начинать следующий этап.
