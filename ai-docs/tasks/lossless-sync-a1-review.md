# A1 proposal — независимое архитектурное review и следующий шаг

Дата: 2026-10-23. Implementation HEAD: `3e82e6380c30d0f9443b6692ea34faccaacddef7`.
Предмет: [предложение Sol](lossless-sync-a1-proposal.md),
[первоначальная задача](lossless-sync-a1-design.md).

## Заключение

Context-scoped ratchet domains и общий durable ingress для delivery paths
принимаются как направление для следующей ограниченной проверки. Большой
production change-set из §4 proposal пока не готов к реализации: остаются
проверяемые вопросы legacy migration, получения predecessor при квотах,
crypto/session binding и admission/commit semantics.

Baseline выполнен корректно в заявленных границах: лог показывает ordered 256,
первую hash page 64 и `MissingMessageKey(0)` после reload. Это component evidence,
не runtime/GUI/network acceptance. Повторять тот же случай в этом review не
требовалось. Проверены proposal, лог, текущие source paths и закреплённый
vodozemac 0.10.0; проведены отдельные crypto/runtime разбора. Новый код/тесты
продукта не запускались и не изменялись. Новые замечания ниже — контрпримеры и
недостающие контракты проекта решения, не найденные уязвимости ещё не написанного кода.

Положительные решения: не клонировать старые pickles в новые domains; разделять
possession и readable delivery; сохранять immutable events; выполнять пробный
decrypt с rollback; ACK только после readable commit; использовать mandatory
vault schema gate вместо необязательного файла; явно обозначать цену gaps.

## R1. Legacy DH tail нельзя восстановить из одного receive frontier

[Таблица совместимости proposal](lossless-sync-a1-proposal.md) признаёт unknown tails, но
исполняемое правило остановки и граница совместимости должны быть определены.

Контрпример: Alice отправила K0 и задержанный K1 одной DH chain. Bob получил K0
и ответил B0. Alice получила B0 и отправила L0 следующей chain. Состояние Bob
после K0 не сообщает, существовал ли K1. Заголовок Olm L0 содержит ratchet key
и chain index, но не длину предыдущей chain. Последующие DH transitions могут
вытеснить старую receiving chain вместе с возможностью дочитать K1.

Экспорт native pickle эту информацию об отправителе не создаёт. Для legacy
нужны либо явная остановка при недоказанном завершении old chain, либо отдельно
спроектированное authenticated closure evidence. При остановке может зависнуть
обычная старая переписка уже на первом таком DH transition. Сохранённый ciphertext
не равнозначен гарантии его будущей расшифровки. C1 от известного genesis не
доказывает безопасный upgrade произвольного существующего профиля.

Для новых domains собственный ordinal/predecessor от genesis способен дать
недостающее свидетельство порядка. У vodozemac публичны `Message::ratchet_key`,
`chain_index`, `PreKeyMessage::message`, `Session::pickle/from_pickle`; это
позволяет проверить guard без замены Olm. Но native `SessionPickle` содержит
закрытые поля и не предоставляет публичный receive-frontier getter. Требуется
выбор: version-pinned serde adapter с валидацией, новый upstream API или явно
ограниченная миграция. `Debug` и max author_sequence не являются importer.

Проверенный source библиотеки: `vodozemac-0.10.0/src/olm/messages/message.rs`
47–53 и 80–92; `messages/pre_key.rs:85`; `session/mod.rs:53,113–118,333–343,510–535`.
В нём максимум пять receiving chains, oldest удаляется при заполнении.
Это отличается от Signal Double Ratchet header с previous-chain length;
нельзя переносить алгоритм Signal на Olm, предполагая наличие этого поля.

## R2. Hash paging и заполненный inbox не гарантируют получение head

[Раздел об ingress в proposal](lossless-sync-a1-proposal.md) одновременно обещает
ограниченный inbox, получение prefix и адаптацию размера страниц.
[Текущий planner](../../crates/kilogram-session/src/lib.rs#L412) выбирает первые
64 missing hash IDs. Старый peer не понимает нового lane-range запроса и не
получает byte/admission budget из текущего inventory.

Если pending заполнен поздними ciphertext, genesis имеет поздний hash, а
следующая страница тоже не содержит genesis, получатель не может принять страницу
или начать drain. Повтор неизменного inventory даёт тот же результат. Резерв E
для head оставляет место, но не заставляет peer прислать нужное событие.

Нужен точный алгоритм bounded legacy repair либо явный compatibility result
`blocked/upgrade-required` для такого случая. Новый negotiated prefix fetch
может решить это у обновлённых peers; его поддержку нельзя приписывать старым.
Для mailbox определить scan cursor, повторный обход и обработку полных квот,
не связывая продвижение list cursor с readable frontier.

## R3. Admission должен покрывать формат, frame и полный commit

Резерв одной ciphertext-записи из [proposal](lossless-sync-a1-proposal.md)
не гарантирует завершения transaction: нужны event/authorization, projection,
ratchet/frontier, ACK intent, а также staging/shadow overhead там, где они есть.
У vault общий лимит snapshot 512 MiB
([constants](../../crates/kilogram-state/src/vault.rs#L44),
[validation](../../crates/kilogram-state/src/vault.rs#L2586)). Отдельные квоты
pending и readable history должны согласовываться с этим общим пределом.
При заполненном history/vault head может быть принят, но readable commit не
поместится. Не обещать progress на полном диске; заранее различать reservable
capacity и честный storage-blocked state, сохранять accepted данные.

Новый E≤512 KiB нельзя молча применять к старым валидным events: существующий
fan-out до 32 устройств и text до 64 KiB способны дать signed event больше E
([devices](../../crates/kilogram-identity/src/account.rs#L61),
[text](../../crates/kilogram-protocol/src/lib.rs#L94)). Сохранить byte-exact legacy
event и одновременно отбросить его по новому лимиту невозможно.
Даже новые 64×512 KiB превышают
[frame cap 8 MiB](../../crates/kilogram-transport-iroh/src/lib.rs#L20).
Нужно byte-bounded response construction с понятной negotiation и явная
политика oversized legacy event. Конкретные квоты proposal пока не утверждены.

## R4. Частичный sync admission, crypto epoch и возобновление требуют точных переходов

- [Legacy completion](../../crates/kilogram-session/src/lib.rs#L179) требует
  exact equality всех sent IDs; он не принимает произвольное подмножество.
  Если два pending уже committed, а третий упёрся в quota/обрыв, определить
  error/response, durable checkpoint и следующий inventory. Возможность
  продолжить после неуспешного round допустима только с явным контрактом.
  `SyncComplete` сам по себе не является message ACK и не подписан отдельно;
  контекст аутентификации и signed inventory/diff сохраняются.
- Possession inventory, crypto receive frontier, readable history и causal
  authoring frontier различны. Pending нельзя автоматически включать в parents
  нового исходящего события или считать readable convergence.
- Epoch должен выбирать ровно одну agreed Olm session до активации writers.
  Указать роли, base key/OTK/session ID bindings, одновременное начало и расход
  handshake позиций. Нельзя незаметно перенести нынешний механизм двух candidates
  ([code](../../crates/kilogram-ratchet/src/lib.rs#L957)) в один новый n=0 slot.
  Нужны правила same-chain index, new-key index=0, old-chain replay, двойной
  predecessor/fork и rollback после неуспешной inner-binding проверки.
- Permanent gap намеренно останавливает lane. При двух retained blocked epochs
  третья попытка user-approved epoch тоже может упереться в лимит. Нельзя обещать
  безусловное возобновление новой эпохой и одновременно бессрочно сохранять все
  старые epochs при жёстком лимите два. Нужна явная политика capacity/owner choice;
  автоматическое удаление старых данных или снятие лимита недопустимо.
- Writer fencing должен охватывать уже запущенный старый процесс, а не только
  запуск старого binary после upgrade. Правильный manifest-version gate — основа,
  но это ещё не проверка всех mutable entrypoints и crash windows.

Ожидание gap удерживает chain secrets и откладывает healing. Это осознанная
security/availability цена предложенной надстройки. Общая проблема хранения
пропущенных ключей описана в [Signal §8.4](https://signal.org/docs/specifications/doubleratchet/#deletion-of-skipped-message-keys);
strict gap blocking не является требованием самой этой спецификации.

## Следующая задача тому же чату: ограниченный offline feasibility spike

Не повторять весь аудит и не писать новый общий RFC. Сделать один небольшой
test-only executable prototype с двумя секциями ниже и коротко уточнить proposal.
Продуктовые crates, wire default, vault loader и зависимости workspace не менять.
Никаких sockets/GUI/physical tests. Новую production-криптосхему не вводить.
Пользоваться закреплённой библиотекой и синтетическими ключами.

### F1. Реальный Olm и сохраняемый guard

1. Проверить guard нового domain с явно выбранной session, своим frontier и
   pickle/reload: ≥256 messages, минимум два contexts и ≥7 DH transitions.
   При задержанном predecessor новый successor остаётся pending; сначала
   доказать неизменность ключей при ожидании, затем доставить predecessor и
   продолжить. Не требовать продвижения через незакрытый gap.
2. Same-chain wrong index, подписанный неверный ordinal/binding, другая session,
   wrong context/epoch, replay и fork не меняют canonical serialized ratchet,
   frontier или readable results. Перечислить, какие auth checks реальны, а
   какие заданы trusted test transcript; mock не является доказательством wire.
3. Legacy: известный genesis single-chain drain проходит; воспроизводится trace
   K0/K1/B0/L0 и явно возвращается unresolved legacy tail, не фиктивная успешная
   миграция. Проверить reload существующего native pickle с ненулевым index и
   skipped keys: показать, какие frontiers importer умеет/не умеет доказать.
   Не восстанавливать отсутствующую длину старой chain догадками.
4. Выбрать конкретный способ чтения native pickle и зафиксировать coupling к
   версии. Допустим отказ от неподтверждённой автоматической миграции с точной
   классификацией состояний. Не менять и не патчить Cargo registry ради теста.

### F2. Маленькая детерминированная модель ingress/sync

Использовать fake ciphertext для проверки очередей и квот, а не повторять crypto.
Минимальный trace: ёмкость на три обычные pending записи с явно учтённым head
reserve; шесть сообщений, genesis последним по hash; разрыв после двух durable
admissions. Отдельно задать полный committed-history budget и oversized event.

Модель должна показать исходный deadlock и проверить выбранный контракт для:
legacy hash sender, обновлённого prefix-capable sender и mailbox cursor.
Если old peer не может обеспечить progress в заданных пределах, результат —
явный compatibility blocked state без потери ciphertext/ложного ACK; это не
засчитывается как lossless convergence для старого peer.

Таблица переходов должна охватывать AcceptedPending, AcceptedReadable,
Backpressure/StorageBlocked, InterruptedPartial и Invalid. Зафиксировать:
полноту/ошибку batch response, inventory после restart, byte budget, резерв
полного commit, отдельный causal frontier и условия retry без busy loop.
Проверить обе границы: slot reserve и общий disk/history admission.

### Сдача и остановка

- Один компактный воспроизводимый offline harness, фиксированный путь EXE,
  exit nonzero при нарушении проверяемого инварианта; actual stdout/exit и README.
  Отрицательный сценарий считается пройденным только при ожидаемой явной
  блокировке и неизменности state, а не при произвольной ошибке.
- Небольшое дополнение к proposal: выбранные переходы и legacy limits; таблица
  R1–R4: подтверждено/уточнено/осталось открытым с ссылкой на evidence. Отдельно
  описать policy третьего epoch и future writer-fencing test; полную migration
  в этом spike не реализовывать.
- Указать предлагаемый первый production slice и его end-to-end результат.
  Не объявлять весь A1 закрытым по этим компонентным опытам. После сдачи
  остановиться перед review результатов; не начинать большой change-set §4.
- Соблюдать AGENTS.md и resource policy; build offline/locked, jobs=2 через
  `invoke-cargo-friendly.ps1`. Standalone network-free probe — постоянный EXE;
  любые Rust network harnesses — только `run-cargo-tests-stable.ps1`.
  Полный workspace/mutation corpus для spike не нужен.

Если конкретный пункт требует модификации зависимости, нового протокола или
большой интеграции, записать доказанный барьер вместо расширения scope. Это
конечная проверка осуществимости, не начало новой серии абстрактных milestones.
