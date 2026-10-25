# A3-S4 contract review 1: направление принято, нужны R1–R3

2026-10-25, Asia/Yekaterinburg. Root review draft71101 bytes /711 lines, SHA256
`9bf73fc65d8e838472c82b46ed06aad8da3e1060a8373e85a6904699e54262e0`,
[S4 contract](long-lived-dialog-a3-runtime-s4-contract.md), baseline
`7f82036ee5803edd64be98b73572604250f5961c`.

**Контракт целиком пока не принят; product implementation и commit/push не разрешены.**
Следующее поручение — только адресная доработка контракта и matrix. Ни один finding
ниже не выдаётся за воспроизведённый runtime bug: S4 ещё не существует, выводы сделаны
из proposed transitions и чтения actual pinned sources. Тесты не запускались.

## Что принято по существу

Fresh production S4 и обязательный S5 для V4→V5/indexed-L1/live handoff — разумная
граница. S4 должен закончиться работающими CLI/IPC/transport/owner и P/F gates;
весь A3/resumed-L1/MVP остаётся открыт. S5 конкретен, не заменяется отказом старому
профилю и не превращается в новую произвольную серию retirement-этапов.

Поддержаны single owner, общий атомарный writer для Head/Ready/native/history,
current/historical authority separation, local Root2 permission без fake lease,
exact queue retry, bounded disk-backed scheduling, ACK после readable commit,
same session за511 и честный mapping reused/new/deferred evidence.

Namespaces21/22 не пересекаются с существующими slot21/45/22 и global Author48–50.
Само добавление namespaces допустимо; оно требует R1 ниже. Reject-only commit_gate
после staging — правильный способ повторно проверить time/connection перед logical
commit, при явном разделении precommit отказа и IO/ambiguous outcomes.

Ten-field locator нужен: actual S3 parser действительно имеет9 fields и не несёт
relay URL. Новый строгий именованный decoder, без trial fallback, может сохранить
старые vectors и domain2: canonical arity9/10 различается и исключает принятие тех
же bytes обоими parsers. Но lifecycle/route rules требуют R2/R3.

## R1 [P1]: marker в новом namespace не останавливает предыдущий indexed writer

Draft §4.1/§10 обещает закрыть component/generic обход и old-writer mutation.
Новая проверка RuntimeProfile в текущем коде недостаточна для уже опубликованного
S3 writer7f82036: его schema5 reader принимает все64 root summaries, не требует
пустоты будущих slots21/22/45 и не знает нового marker.

Evidence baseline:

- [Root::decode/check](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L255)
  читает фиксированную Root5 форму и все64 summaries; проверяет totals/ledger,
  но не semantics новых roots.
- [IndexedDialogVault::open](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L437)
  проверяет schema5/sentinel/account/claim и возвращает writable vault без marker.
- [history2_guard_old](../../crates/kilogram-state/src/vault/indexed_dialog/history_protocol2.rs#L500)
  разрешает старые QueueAccept/RepairCredit даже для bound2, а
  [QueueAccept](../../crates/kilogram-state/src/vault/indexed_dialog/history_writer.rs#L350)
  не знает runtime policy/Head/activation. Публичный Protocol2 writer также не знает
  production marker. Это корректно для S3 component scope, но не fence нового runtime.

При неизменном Root5 format старый library writer после освобождения lock может
открыть новый profile и выполнить mutation без runtime-purpose checks. Проверка
одним d9b V4 CLI доказывает старый V4 fence, но не отказ ближайшего schema5 writer.
Это accidental downgrade/API compatibility boundary, не обещание защиты от
злонамеренного локального процесса с доступом к секретам и произвольным кодом.

Нужно: exact authenticated local-root/runtime format discriminator, который старый
S3 reader уже гарантированно отвергает до выдачи writable API. Например, отдельная
строго versioned runtime Root encoding/обязательное расширение, несовместимое с
его Reader::end; выбрать конкретные bytes/rules, не только название feature flag.
Новый reader отличает component roots от runtime roots без generic fallback.
Bootstrap фиксирует discriminator и marker в первом durable Root, без окна
component-compatible committed runtime. S1–S3 component encodings/fixtures frozen.

В matrix добавить ближайший schema5 predecessor, а не только V4 EXE: обычный
публичный open/queue/component/generic путь должен получить refusal до protected
logical mutation. Future test helper компилируется из exact S3 sources, с fixed
path/hash; текущие contract-only edits не требуют его запуска. Проверить отдельно
same-version bypass и cold old-version open; prior legitimate component profiles
сохраняют прежнюю поддержку. Root/Head/native/queue/credit не меняются после отказа.

## R2 [P1]: exact locator retry после crash не связан с реальным endpoint

Draft §8, lines474–478: startup выдаёт revision+1, но crash после locator commit до
export должен повторять прежние raw bytes. Одновременно разрешена ротация endpoint
после restart. В durable records §4 нет transport endpoint secret или стратегии
восстановления прежнего endpoint/IP binding.

Текущий [bind_loopback](../../crates/kilogram-transport-iroh/src/ordered.rs#L15) и
[route builder](../../crates/kilogram-transport-iroh/src/lib.rs#L99) не задают secret_key.
Pinned iroh1.1.0 `src/endpoint.rs:226` при bind использует новый SecretKey, если он
не задан; port0 также может дать новый адрес. В старом
[startup](../../apps/kilogram-cli/src/ordered_dialog/runtime.rs#L1140) новый locator
соответствует фактически созданному endpoint. Простое повторение durable старого
locator после нового bind рекламирует endpoint, которым процесс уже не владеет;
peer не сможет пройти exact TLS/locator binding и восстановить диалог.

Нужно выбрать один полный lifecycle. Для S4 предпочтительна уже допустимая ротация:
exact export/retry пока жив тот же endpoint и locator ещё current; после process
restart новый endpoint получает новый checked monotonic revision, commit до export,
старый locator не объявляется текущим адресом нового process. Account/Device/native
D/E/SID/Q при этом прежние. Если выбирается persistent endpoint вместо этого —
явно добавить secret custody, bind/port failure и recovery, не подразумевать их.

В F/P добавить real kill после locator commit до export/import, новый bind с другим
endpoint/port, повторный crash в этом окне, old/same/lower revision refusal, import
актуального locator и возобновление B-only backlog без нового пользовательского send
у A. Проверять actual endpoint ID/адрес в выданном artifact, не только bytes/signature.

## R3 [P2]: конкретизировать route builder и пустой IP-list для RelayOnly

Draft §8 ссылается на обычные builders. Actual
[endpoint_builder](../../crates/kilogram-transport-iroh/src/lib.rs#L99) использует
presets::N0, а `endpoint_builder_with_relay(..., None)` сохраняет его defaults.
Pinned N0 включает public relay selection и n0 address publication/resolution.
Даже explicit relay не убирает address lookup автоматически. Принятый
[local relay test](../../crates/kilogram-transport-iroh/tests/local_relay_transport.rs#L25)
явно применяет Minimal/clear_address_lookup; это не свойство одного route enum.

S4 ограничен operator locators/local direct/own relay. Зафиксировать production
конфигурацию этого scope: отсутствие implicit public relay/discovery fallback,
явный источник допустимого URL и правила DirectOnly/Auto/RelayOnly при URL None/Some.
Не переносить ограничения только в test helper: P должен вызывать тот же runtime
builder. Await_route_policy сохраняется, меняются только необходимая конфигурация
и зарегистрированный ALPN; существующий общий transport вне S4 не перестраивать.

RelayOnly builder убирает IP transports; у реального endpoint IP-list может быть
пустым. Draft `<=16` и «IP tuples как S3» не задают явно допустимость этого случая,
а S3 descriptor parser запрещает empty addresses. Указать exact RuntimeLocator2
predicate: разрешённый URL-only locator, запрет отсутствия всех routes, sorted
canonical IP tuples/URL, соответствие выбранному local route policy. Новый parser
не должен случайно наследовать S3 nonempty-IP predicate.

Matrix: direct-only locator, URL-only real RelayOnly locator, empty/empty reject,
отсутствие допустимого relay при RelayOnly, signed locator/actual endpoint mismatch,
policy switch/reconnect с неизменными Event bytes. Проверка конфигурации показывает,
что extra public lookup/relay не были включены; external traffic для этого не нужен.

## Небольшие уточнения уже заявленных гарантий

В обновлённом тексте зафиксировать, что bounded connections/tasks охватывает также
TLS/admission до admitted, с deadlines и отказом до allocation/spawn за лимитом.
Прежний runtime уже берёт semaphore до TLS. Для profile-total128 refs пояснить,
когда возвращаются slots и как C2 получает место, если C1 заполнил окно и ждёт
ACK/gap; fairness ready turns сама по себе не определяет admission. P07 должен
наблюдать прогресс C2 именно при занятом/задержанном C1, без удаления durable debt.

Назвать конкретно Cargo profile для P/F/components (обычный `verification`, если
нет отдельно обоснованного изменения). Дополнить F01 точкой expiry/connection-close
после plan/staging перед physical commit: commit_gate отказывает без Root/native/
source advance и без публикации. Уточнить outcome нормального gate refusal против
IO/ambiguous failure; не выдавать новый gate за уже существующую функцию S3.

## Следующее действие и выполненные проверки

Исполнитель дорабатывает только контракт по R1–R3 и уточнениям, сохраняет этот
review и frozen A3/S1–S3. Сдать новый hash и disposition с exact anchors, один callback
root, затем остановка. Новый human approval не нужен: review/repair уже разрешены.

Root прочитал весь711-line draft и адресные actual source paths для root codec/open/
writer/queue/Ready/transport/startup. Baseline/hash проверены; до root edits index
пуст, executor добавил только contract. `git diff --check` PASS. P/F/tests/benchmarks
не запускались, product/dependencies/fixtures не менялись. Этот review не принимает
ещё не реализованные guarantees, performance или whole-A3 readiness.
