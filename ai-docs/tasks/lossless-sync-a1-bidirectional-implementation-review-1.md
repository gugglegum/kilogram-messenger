# OD1 bidirectional: implementation review 1

Дата: 2026-10-23. Решение: **нужны исправления R1–R3, code diff пока не принят**.
Следующий шаг — repair тем же исполнителем, затем повторное независимое review.
Commit/push продуктового изменения пока не поручен. Это продолжение уже разрешённого
пользователем этапа, не запрос нового разрешения.

## Проверенная ревизия и границы evidence

- HEAD, origin/master и фактический remote master: `47d19c41be449deea9c1af003f7a27cb6992710c`.
  В коммите ровно 7 принятых документов контракта; implementation baseline кода — `dcc1300b`.
- Входной [implementation report](lossless-sync-a1-bidirectional-implementation.md):
  SHA-256 `f7c8f6d5cee91f23eafaf3e2deef72f339190d47196fc1460ef394aeb39699db`.
- Входной `review-code.patch`: `a392d15d9102774799d22793845fb224105e7b931058ab1b034cb0f5ea4ce697`.
  `git apply --check --reverse` прошёл; index пуст. Все 544 файла frozen-source manifest
  и все записанные artifact hashes сверены без расхождений. Manifest/report этой итерации
  нужно архивировать перед обновлением evidence следующего candidate.
- Текущий EXE: `532a5a76e836faacca63a71306ba10f1129718e7b0aa157e363f58bf7642ce93`.
  Duplex256/reopen/C3 и два malicious-input traces относятся к нему. Остальные process
  gates отчёта относятся преимущественно к `f32c6650…`; после них изменялось typed early
  admission. Эти разные fingerprints проверены, не объявляются одним прогоном.
- Read-only review охватил wire/framing/exporter/handshake/guard, phase/native binding,
  vault primary/recovery/fences/reservations, queue/ingress/repair/receipts, runtime/IPC,
  caches и мост к legacy. Секретные native поля/Debug не использовались как доказательство.

Независимо повторены:

1. `scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter 'ordered_dialog::' -CargoJobs 2 -Run -NoCapture`
   при `CARGO_NET_OFFLINE=true`: **3 PASS**, 0 failed. Это guard/phase/native-bound components,
   не полный process acceptance. Максимальные event/blob 4978/37727 bytes; lifetime512.
2. `scripts/test-ordered-dialog-process.ps1 -Scenario Gap` на текущем постоянном EXE:
   **PASS**, 67 exact Text на обоих устройствах, настоящая смена DH, 64 future pending при
   `receive_next=2`, настоящий kill/relaunch, восстановление до67, pending=0/unacked=0,
   SID неизменен. 11 immutable records сохранены; полный run73.18s.
   [Лог](lossless-sync-a1-bidirectional-evidence/root-review1-gap.log),
   [result](lossless-sync-a1-bidirectional-evidence/root-review1-gap-result.json),
   [full64 checkpoint](lossless-sync-a1-bidirectional-evidence/root-review1-gap-before-kill.json).
   Первая sandbox попытка остановилась на loopback IPC os10013 и не засчитывается как
   продуктовый failure/PASS; разрешённый запуск с native token завершился успешно.
3. `git diff --check` прошёл. Чужой `ai-docs/development-environment.md` неизменён:
   `330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.

## R1 — P2: consent renewal до Offer блокирует первый handshake

Связанные места: `admin.rs:329–354`, `runtime.rs:218–234`, `handshake.rs:336–355`,
`engine.rs:507–514` в `apps/kilogram-cli/src/ordered_dialog/`.

Повторный import того же C/cert refs/membership сохраняет исходный immutable Spec,
но обновляет consent-current. Hello подписывает hash актуального consent. Затем
verify_initial_pair сравнивает его с исходным `spec.consent_ref()`. Поэтому валидный,
явно подписанный обоими владельцами renewal до первого Offer допускает fresh TLS
admission, но каждый ensure_offer завершается `initial Hello scope/role`.
Повтор подключения не исправляет состояние. Это нарушает разрешённый renewal (§2/§5).

Нужно разделить проверку выбора первого InitialPair и проверку уже выбранного
исторического pair. Сохранить signatures, exact C/devices/roles, согласованность двух
Hello, текущую consent/authority admission policy и неизменность выбранного T/E/SID.
Простая замена всех сравнений на consent-current сломает старый selected pair после
renewal. Не ослаблять Root consent или принимать произвольный peer consent по подписи.

Проверки: valid renewal до первого Offer; renewal после durable Offer/Grant и active
epoch с restart; несовпадающий/неподписанный consent по-прежнему отклоняется. Проверить
также повтор старого выбранного Offer после renewal, когда B ещё не сохранил Grant.
Если исправление требует нормативной wire/schema развилки — сначала вернуть точный
вариант на review; менять утверждённый контракт молча нельзя.

## R2 — P2: fence-only interrupted creation усыновляется с новыми owner/keys

`admin.rs:91–130` при отсутствии schema4 manifest заново создаёт signing/encryption keys
и claim nonce. `crates/kilogram-state/src/vault/ordered.rs:96–119` допускает resume по
одному account fence; claim/keys durable только при первом DB commit на строке163.

Trace: enrollment X пишет fence; kill до commit_snapshot; enrollment Y того же root
проходит resuming=true и записывает новые claim/keys. Даже для X ключи до этого commit
не восстанавливаются. Это противоречит §12 (same inputs/saved keys, никакого adoption).
Здесь ещё не были опубликованы credentials/messages: потеря опубликованных сообщений
или компрометация чужого готового profile не утверждается.

Допустимо fail closed с CreationConflict для ambiguous fence-only root, либо durable
Initializing claim, позволяющий проверить owner и восстановить те же keys. Не стирать
fence/state и не ослаблять legacy protection ради retry. Проверки interrupted windows
до/после первого manifest с same/different owner, плюс обычный enrollment/reopen и old
binary fence. После успешного durable claim ожидается resume тех же сохранённых keys.

## R3 — P2: lifetime exhaustion скрывается за Active и успешным enqueue

`outbox.rs:106–145,210–214`, `runtime.rs:1075–1097,1297–1341`.
При own.next=512 materialize_one возвращает None, queue по-прежнему принимает новые
requests до128, а при отсутствии pending/unacked runtime сообщает Active. Клиент получает
успешное durable enqueue, но не видит, что отправка невозможна в этой epoch. §11 требует
явный UpgradeRequired после n511; числовой send_next в status не заменяет этот outcome.

Нужен явный local send-blocked UpgradeRequired в status и при новых enqueue за доступной
lifetime capacity. Уже принятые queue items не удалять; если они могут остаться из-за
ранее принятой очереди, показывать причину блокировки. Idempotent retry старого request
должен работать. Не прерывать получение оставшихся допустимых сообщений противоположной
lane, prefix/ACK repair, history или retry exact committed ciphertext. Не создавать epoch
и не сбрасывать guard автоматически. Узкий test должен пройти настоящий engine/IPC status
на границе 511 Text, включая reopen; одного guard expected_send error недостаточно.

## R4 — небольшое уточнение валидатора genesis

Получатель Init/Confirm не проверяет нормативные request16==OfferId[..16]/GrantId[..16]
и пустой user-parent список (engine.rs:665/754); корректные значения задаёт только sender.
Согласованный malicious peer может подписать иной Core/Inner. Нарушения конфиденциальности
или продвижения неправильного native cursor здесь не найдено. Добавить узкие проверки
и negative coverage рядом с исправлением handshake, чтобы sender/receiver реализовывали
одну каноническую схему. Не расширять задачу до redesign протокола.

## Повторная сдача

R1–R3 блокируют приёмку, R4 можно закрыть в том же repair. Изменять только необходимое;
сохранить текущий iteration1 patch/manifests, подготовить адресный repair diff и новый
полный frozen source manifest/EXE hashes. Сначала regression, показывающий дефект либо
точный trace, затем исправленный результат. Повторить затронутые handshake/enrollment/
exhaustion gates и компактный duplex/restart на final EXE. Полный дорогой corpus не
повторять без причины; для сохранённого evidence явно указать applicability старых hashes.
Stable test executables, CargoJobs2, без secret dumps. После отчёта один содержательный
callback оркестратору и остановка до приёмки; commit/push только по следующему поручению.

Этап остаётся opt-in local two-process OD1, L1 511 Text/lane, одна epoch, два C,
operator-assisted locators. Он не объявляет готовым внешний/daily MVP и не снимает A2.
