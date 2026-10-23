# A1: lossless sync — воспроизведение и проект исправления

Дата: 2026-10-23. Статус: baseline и проект подготовлены, независимое review
выполнено. До production необходим ограниченный offline feasibility spike;
[замечания и следующая задача](lossless-sync-a1-review.md).
Реализация продуктового исправления не начата.
Результат: [проект решения и regression matrix](lossless-sync-a1-proposal.md),
[baseline evidence](lossless-sync-a1-evidence/baseline-run.log).
Исполнитель: отдельный чат GPT-6.1 Sol Max.
Результат предназначен для независимого архитектурного review перед реализацией.
Это не автоматический запуск M1.2.63 и не изменение принятого wire default.

## Цель и границы

Спроектировать ограниченное исправление A1: доступный корректный ciphertext
не должен становиться недешифруемым из-за порядка получения событий через
paged sync, разные conversations, direct/relay и mailbox. Отдельно обозначить
условия доступности исходных данных и ключей: никакой протокол не возвращает
уже утраченные ключи или отсутствующие у всех участников данные автоматически.

Задача заканчивается воспроизводимым baseline, одним рекомендуемым решением,
анализом совместимости и конкретным планом регрессионных проверок. Продуктовое
исправление, wire/storage migration и новая криптографическая схема здесь не
реализуются. Допустим только небольшой test-only диагностический repro, если
имеющегося недостаточно для установления причины; новых framework не создавать.

## Точка входа

1. Прочитать корневой `AGENTS.md`, выполнить `git status --short --branch` и
   определить фактический HEAD. Исходная ревизия аудита и передачи:
   `3e82e6380c30d0f9443b6692ea34faccaacddef7`. Если HEAD изменился, сопоставить
   релевантные изменения, а не откатывать их и не предполагать старый код.
2. Прочитать [передачу](../project-handoff-2026-10-23.md),
   [A1 и границы аудита](../reviews/2026-10-22-architecture-mvp-audit.md),
   [инструкцию компонентного repro](../reviews/repro-2026-10-22/README.md),
   [принятые требования identity/history](../../docs/RFC-0193-user-sovereignty-and-contextual-identity.md).
   Другие RFC читать только по относящимся к задаче ссылкам.
3. Сохранить существующие незакоммиченные документы и чужие изменения.
   Не начинать recovery retirement или другой ранее запланированный milestone.

## Что установить

1. Повторить сохранённый offline component repro по его README. В нём реальные
   `SyncServer`, `EventStore`, подписанные события и persisted ratchets; успешный
   exit означает воспроизведение старого дефекта, а не исправность продукта.
   Сохранить фактический вывод и ревизию. EventId случайны, точное failing sequence
   может отличаться; не выдавать случайный прогон за детерминированный regression.
2. Проследить production-путь от выбора страницы до decrypt и transaction commit.
   Ключевые места: `crates/kilogram-session`, `crates/kilogram-ratchet`,
   `apps/kilogram-cli/src/main.rs`, mailbox store/client. Различить ratchet message
   index, author sequence, event hash order и causal order: они не тождественны.
   Отделить воспроизведённое компонентно от подтверждённого только чтением кода
   и от ещё не проверенного на реальном runtime.
3. Сравнить как минимум сохранение общей device-pair сессии с согласованным
   приёмом/durable buffering и разделение ratchet domains по conversation/context.
   Если оба недостаточны, объяснить конкретную причину и предложить иной вариант.
   Не выбирать решение только по минимальному diff. Показать случай, в котором
   история одного чата или mailbox приходит после более новых direct-сообщений.
4. Рекомендовать один вариант и объяснить его security/availability trade-offs.
   Определить domain identity, аутентификацию порядка, работу при пропусках,
   повторной доставке, permanently missing predecessor и hostile peer. Указать
   пределы памяти/диска/CPU, backpressure и restart-safe поведение. Бесконечное
   хранение skipped keys или безграничный буфер не являются готовым решением.

## Инварианты

- Сохранить E2EE, проверку membership/certificates/authority и replay/revocation
  boundaries. Root остаётся административным ключом, не online message signer.
- Сохранить immutable events/EventId, causal links и уже committed readable
  history. Не удалять неудобные события, не переподписывать историю молча и не
  считать нечитаемый ciphertext успешно доставленным пользователю.
- Сохранить атомарность ratchet/event/projection/outbox/ACK transitions; объяснить
  crash windows между получением, расшифровкой, commit и подтверждением.
- Не обходить explicit consent через auto-reset, history rewrap, recovery import
  или downgrade. Dormant recovery facade остаётся отключённой.
- Не добавлять новый публичный глобальный ID/счётчик, связывающий независимые
  контексты. Текущий `AccountId` не считать окончательной social identity.
  Полный переход к contextual identity не включать в реализацию A1, но явно
  описать совместимость выбранных domains с будущим переходом.
- Сохранять opaque E2EE ciphertext и bounded volunteer storage. Для анализа A1
  нельзя просто выключить mailbox или исключить его из гарантии.
- Public default сейчас legacy Postcard v6. Включение canonical v2, сортировка
  только одного batch или увеличение константы skipped keys сами по себе не
  считаются доказательством устранения A1.

## Обязательное решение о совместимости

Для рекомендуемого варианта составить небольшую таблицу:

- persistent ratchet state и его загрузка;
- signed wire events, authentication/KDF domains и protocol negotiation;
- существующая readable history и пока не прочитанный ciphertext;
- pending outbox, volunteer mailbox items и sync checkpoints;
- mixed-version peers, restart/crash, rollback и необходимость user consent.

Для каждого указать: меняется ли формат/семантика, как обрабатываются прежние
данные, какое evidence подтверждает безопасность, что ещё неизвестно.
Нельзя требовать удаления профиля или молчаливой потери старых данных. Если без
protocol/state migration нельзя обойтись, описать её явно и вынести на review.
Не представлять forward secrecy как восстановление уже удалённых прошлых ключей.

## Критерии сдачи этой задачи

- Есть результат baseline repro и проверяемая причинная цепочка с местами кода.
- Есть один рекомендуемый проект исправления и сравнение альтернатив без
  разработки нескольких реализаций.
- Есть таблица совместимости и узкий список production-модулей будущего change-set.
- Есть проверяемая regression matrix для реализации: не менее 256 сообщений,
  несколько страниц, persisted state/restart между страницами, смешанные
  conversations и delivery paths, duplicate/retry, crash/ACK boundaries,
  авторизационно неверные входы и bounded поведение при недостающем predecessor.
  Матрица различает component, runtime/process и physical two-host evidence;
  проверяет decrypt/readable projection, а не только совпадение EventId.
- Будущий regression должен воспроизводимо падать на исходной реализации и
  проходить после исправления; текущая задача определяет, как это обеспечить.
- Результат — один короткий design-документ в `ai-docs/tasks/` (ориентир 2–4
  страницы со ссылками), фактические логи нужных проверок и краткий ответ в чате.
  Указать нерешённые вопросы. После этого остановиться до review решения.

## Что не входит

100 000 events/lifetime-scalable paging (A3), Iroh upgrade (A2), onboarding,
multi-contact, actor redesign, полноценная contextual identity, группы, media
и recovery UI — отдельные change-set'ы. Зависимости с A1 нужно назвать, но не
реализовывать попутно. A2 остаётся приоритетной security-задачей до внешнего
использования; его выделение не означает принятие известного риска.

Не запускать полный workspace/mutation corpus для документационного решения.
Использовать существующий ограниченный repro; дополнительные проверки должны
закрывать конкретную неопределённость. Windows network test harnesses запускать
только через `scripts/run-cargo-tests-stable.ps1` после компиляции без запуска;
фиксированные имена EXE сохранять. Не создавать дополнительные чаты или
массовую параллельную команду агентов для этой ограниченной задачи.
