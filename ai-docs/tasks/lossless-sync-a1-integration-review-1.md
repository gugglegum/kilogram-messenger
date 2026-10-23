# A1 integration — review итерации 1 и задание на исправления

2026-10-23. HEAD `3e82e6380c30d0f9443b6692ea34faccaacddef7` + незакоммиченный
diff из [исторического отчёта исполнителя](lossless-sync-a1-integration-iteration1.md).
Вердикт: **нужны исправления I1–I4 до приёмки узкого slice**.
Следующая итерация остаётся в прежнем scope; нового общего design этапа нет.

Последующий статус: I1–I4 исправлены и закрыты
[приёмкой итерации 2](lossless-sync-a1-integration-review-2.md).
Позиции исходников ниже относятся к историческому diff итерации 1.

## Что подтверждено

Исходный код и сохранённые EXE совпадают с SHA-256 в environment.json;
patch reverse check и git diff --check прошли. Независимый повтор четырёх
negative/reopen тестов через stable runner, jobs=2/locked/offline: 4 passed,
0 failed, 1 subprocess helper ignored; 48.57 s.
[Лог review](lossless-sync-a1-integration-evidence/review-iteration1-negatives.log).

256-test проверяет содержимое plaintext через настоящий history API, vault и
SyncClient/SyncServer, с отдельными дочерними процессами после страниц. Его
финальный лог на совпадающих sources/binary принят как evidence; повторный
18-минутный прогон в этом review не делался. Это не два сетевых runtime actors.
Readiness 188/188 и Clippy — evidence исполнителя, отдельно не перезапускались.

S1 для подписанного bad MAC, S2 exact EventId/authorization и durable partial
batch реализованы содержательно. ACK intent фиксируется с readable commit.
Честная блокировка на unknown DH tail и недостатке общего budget сохранена.
Ниже — отдельные непокрытые ветки отказов/границ, установленные чтением кода;
их исполняемые regressions требуется добавить в repair iteration.

## I1 — P1: operational errors нельзя коммитить как quarantine

[ordered_ingress.rs:576](../../apps/kilogram-cli/src/ordered_ingress.rs#L576)
карантинит любой RatchetError, после чего closure возвращает Ok и коммитит
транзакцию. Однако
[decrypt_inner:1192](../../crates/kilogram-ratchet/src/lib.rs#L1192)
сначала сохраняет session, затем выполняет fallible сохранение account и prekeys.

Корректный genesis: session уже записана с cursor=1, следующее сохранение
account/prekeys возвращает временную ошибку. Ingress оставляет checkpoint.next=0,
пишет quarantine для корректного ciphertext и коммитит продвинутую session.
Доступность диска после retry этого не исправляет. Даже сбой без продвижения
session не должен навсегда карантинить валидное сообщение.

Аналогичная blanket-классификация находится в ветке authorization/encoding
[ordered_ingress.rs:541–555](../../apps/kilogram-cli/src/ordered_ingress.rs#L541):
ошибка чтения локального trust repository не доказывает невалидность автора.

Исправить типизированное разделение ошибок: quarantine только для доказанно
некорректного input. I/O, local-state load/corruption и ошибки обслуживания
ratchet должны прерывать transaction и запускать rollback. Не определять класс
по строке Display и не превращать любой неизвестный error в bad input.
Если корректный prefix был обработан в той же transaction до operational error,
его rollback допустим; не фиксировать частично изменённый native state.

Regression: настоящий fault после session save и до следующего account/prekey
save; после reopen pending bytes сохранены, quarantine не создан, native
digest/frontier/readable/ACK соответствуют pre-state. Убрать fault и успешно
расшифровать тот же ciphertext ровно один раз. Отдельно проверить transient
ошибку чтения trust при drain. Existing bad-MAC quarantine должен сохраниться.
По возможности сначала показать, что новый regression ловит исходный дефект.

## I2 — P1: чтение account не должно снимать fence при ошибке

[persist_account:1735](../../crates/kilogram-ratchet/src/lib.rs#L1735)
использует `read_to_string(...).is_ok_and(...)`. Любая ошибка чтения трактуется
как отсутствие prefix, после чего записывается обычный валидный native pickle.
Успешная запись снимает fence, и последующий CLI/legacy loader пропускает профиль.

Конкретный operational scenario на Windows: account уже загружен; временный
file handle запрещает share-read, но разрешает delete/replace. Проверка prefix
получает sharing violation, atomic replacement может пройти. Злонамеренная
правка файла не требуется. Этот OS trace в review не запускался.

Нужно явное fail-closed управление форматом account (например, сохранение
на основе известного `ordered_access`), а не best-effort чтение security tag.
Не терять защиту при обычном и при ошибочном сохранении. Минимум — не подавлять
ошибки чтения существующего account.

Regression: fault чтения при последующем разрешённом write либо соответствующая
проверка явно выбранного формата. После save/reopen prefix остаётся, legacy
loader/encrypt/decrypt отказывают. Обычные unfenced профили продолжают работать.

## I3 — P2: head не может использовать зарезервированные bytes

[Admission:460](../../apps/kilogram-cli/src/ordered_ingress.rs#L460) всегда требует
`pending_bytes + raw.len() + MAX_EVENT <= limit`, в том числе для index==next.
Если futures заняли limit−MAX_EVENT, любой непустой head получает Backpressure.
Зарезервированное место остаётся неиспользуемым. Slot check выше верно делает
исключение для головы. Это не S3 transient/full-commit exhaustion: остальные
budgets в контрпримере достаточны.

Разрешить head занимать его byte reserve, не выходя за hard pending cap.
Следующий обычный admission должен снова учитывать нужный reserve.
Regression: маленький pending_bytes limit, сохранённый future ciphertext,
приходящий predecessor помещается в reserve и запускает readable drain;
переполнение hard cap по-прежнему отклоняется без потери accepted bytes.

## I4 — P2: runtime-from-profile должен отказывать до generic mutations

`RuntimeFromProfile` не возвращает state_directory
([main.rs:2766](../../apps/kilogram-cli/src/main.rs#L2766)), поэтому не проходит
ранний ordered dispatch. Он вызывает runtime напрямую
([3966](../../apps/kilogram-cli/src/main.rs#L3966)). Runtime успевает выполнить
`install_own_authority_primary`
([21949](../../apps/kilogram-cli/src/main.rs#L21949)) до отказа ratchet loader.
Валидный более новый own device-list меняет trust защищённого профиля, хотя
этот generic writer заявлен запрещённым. Обход decrypt/network не утверждается:
ratchet fence всё ещё останавливает дальнейший запуск.

Добавить gate после разрешения пути профиля под правильным exclusive ownership,
до обычной подготовки runtime/authority mutations. Учесть существующий recovery
порядок, чтобы восстановленный protected account также отказал до writers.
Regression через этот entrypoint: newer valid own device-list, ожидаемый отказ,
vault generation/authority и защищённое состояние не меняются; endpoint не нужен.

## Сдача repair iteration

Исправить I1–I4 одним reviewable change-set с targeted regressions. Не добавлять
domains, migration, GUI, public activation или обходной путь отключения fence.
Сохранить чужие изменения и evidence итерации 1. Обновить компактный отчёт:
пункт → изменение → regression/результат, исходники и hashes на финальном diff.

Сначала быстрые новые regressions и существующие negatives/ratchet tests, затем
один финальный 256-message acceptance на стабильном EXE после окончания code
changes. Повторять его после каждой мелкой правки не нужно. Применимые fmt/Clippy/
обязательные gates сохранить; весь mutation corpus без причины не запускать.

Производительность 256-теста (~18 минут) пока лишь измерение этой функциональной
интеграции, не acceptance реального общения. Избежать случайного ухудшения при
исправлениях; отдельный performance redesign сюда не добавлять.

После сдачи отправить один содержательный отчёт оркестратору и ждать следующего
review. Человек уже разрешил эти циклы в userMessage
`01a1657e-b03f-7d01-aaf0-e5af979125fd`; повторное разрешение не требуется.
