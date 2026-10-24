# A3: первое независимое review контракта

2026-10-24. Baseline `860b351fb747562e1fe12d469b1ad053217a7c0f`.
Проверен [контракт](long-lived-dialog-a3-contract.md), исходный SHA-256
`88ee043195d824b86c448b13a7669877b08c42429fb033a09aa163427044aa7d`, 603 строки.
Это review документа и используемых исходников, не подтверждение новой реализации.

**Решение: направление поддержано, контракт пока не принят.** Требуются три
адресных исправления ниже. Продуктовая реализация и commit/push пока не разрешены.
Новой серии альтернативных RFC и расширения scope не требуется.

## Архитектурные решения, которые не требуют нового согласования

- Сохранить E/D/SID и native session, непрерывные ordinals и ordered decrypt.
  Периодический reset/rollover ради прежней константы512 не нужен.
- Индексируемое authenticated хранилище, отдельно проверенные membership/absence,
  atomic root/data/credits и immutable история. Выбранный compressed radix подход
  допустим; его реализация остаётся отдельной существенной поверхностью review.
- Двухполосный causal cursor со snapshot frontier и MAC. В исходном outbox own
  predecessor действительно входит в parents каждого Text после первого
  (`engine.rs:458–466`); versioned semantics для прочих ранее принятых V1 оговорена.
- Rolling credits с сохранением accepted bytes/debt, отдельным head slot и честной
  остановкой при исчерпании ресурсов. Освобождение временного pending не выдаётся
  за освобождение retained истории.
- **Offline local encryption с explicit Root2 permission принимается по смыслу.**
  Это необходимо для настоящего offline ciphertext backlog256. Последняя локально
  известная signed authority не объявляется знанием об удалённых revocations.
  Перед commit повторяется local policy/time/generation; известный revoke, expiry
  и operational clock/read failure запрещают новую encryption. Network dispatch,
  receive и ACK сохраняют fresh admission/current authority. Нужны отдельные tests
  plaintext queue, durable ciphertext и delivery, включая renewal/revoke/expiry.
- Forward-only storage fence и запрет rollback после durable P допустимы: peer мог
  уже принять P при потере ответа. Это не создаёт права перезапускать genesis или
  удалять старую очередь. Resume и read-only история при паузе обязательны.
- Защита старых клиентов через schema5+mode fence соответствует текущим
  `validate_manifest`/`reject_ordered_writer`; проверить живой owner и recovery
  dispatch до старых mutable paths всё равно необходимо в реализации.

## R1 — P1: лимит physical growth конфликтует с production redb

**Место:** контракт, исходные строки352 и421–428.

Введён hard bound64MiB file growth/transaction; превышение backend должен отклонять.
В используемом redb4.2.0 production region size по умолчанию4GiB. Пока нет полного
региона, `TransactionalMemory::grow` выбирает минимум удвоение usable size; после
заполнения региона добавляет следующий регион. Например, при128MiB usable capacity
следующая обычная малая запись, требующая роста, запросит примерно ещё128MiB плюс
layout overhead. Она не может уложиться в64MiB даже при свободных quota/disk и
единственном frame. Уменьшение actor batch это не исправляет.

Независимо проверенные local dependency sources:

- `redb-4.2.0/src/tree_store/page_store/page_manager.rs:36,533–538,1546–1593`:
  default4GiB, расчёт нового layout, resize и sync до публикации layout;
- `redb-4.2.0/src/db.rs:1475–1479`: `set_region_size` существует только под
  `cfg(any(test, fuzzing))`, это не production knob;
- `redb-4.2.0/src/db.rs:44–62`: StorageBackend обязан исполнить запрошенный set_len
  с нулевыми новыми позициями; нельзя молча вернуть успех с меньшей длиной.

**Требуемое исправление:** одна конкретная production policy, согласованная с
реальным allocator/growth API, memory/physical quota/headroom и recovery. Можно
скорректировать обоснованные bounds либо определить явный конечный maintenance
переход; выбрать простейший достаточный вариант, без нового общего storage framework.
Не считать штатную недостижимость growth-limit «нехваткой диска». Должен быть
определён путь продолжения того же C после такой границы, без потери source/debt.
Не использовать test/fuzz feature для изменения поведения production dependency.

**Приёмка исправления:** source-backed расчёт worst growth/layout overhead и условия
preflight; в будущие gates добавить реальный переход allocator growth boundary с
production configuration, обычной single append, успешными reopen/continuation и
точным учётом фактического set_len/write/sync. Отдельно отказ по настоящей configured
capacity. Полный48GiB run не требуется; небольшой детерминированный boundary test
плюс проверка верхней формулы. Закончить это до принятия всего storage контракта.

## R2 — P2: определить единый mode reference для fresh и migrated диалога

**Место:** исходные строки103–105,150–155,181–183,202–216,392.

Q сейчас определён только как hash `Certificate2(P,A,K,Consent2)` для existing
handoff. Fresh2 вместо этого задаёт `mode_ref=H(C,OfferId,GrantId)` и наследует старый
Ready handshake. Но ReceiptBody2, Mode2/Resume и условие offline materialization
используют Q/mode certificate/Active2 без отдельного определения fresh случая.
Реализатор вынужден сам выбрать, чем заполнять Q и когда fresh C становится Active2;
расхождение здесь несовместимо с единой проверкой receipt/event/current mode.

**Требуемое исправление:** явно определить один тип/значение mode reference для обоих
origin, его равенство Core2.mode_ref/Receipt2/Resume2, источник authenticated proof
для fresh2 и точный durable переход Ready→Active2. Не создавать фиктивные P/A/K для
fresh и не активировать его только из peer hint. После migration старый Q/certificate
остаётся неизменным при renewal. Названия можно сохранить, семантику сделать общей.

**Приёмка:** таблица fresh/existing происхождения reference и два коротких restart
trace: fresh Ready/первый Text/Receipt/offline send и existing Selected/Active при
потере Committed. Их future tests должны сравнивать exact reference, reject mismatch
до native commit и сохранять already accepted data. Existing trace не требует
перепроектирования всей handoff схемы.

## R3 — P2: guards должны учитывать следующий внутренний native counter

**Место:** исходные строки62–66,121–128 и A3-3.

Контракт обещает проверку переполнения до native call, но перечисляет n/g/index/
sequence без связи с двумя внутренними счётчиками используемой библиотеки.
В vodozemac0.10.0 `ChainKey::create_message_key` и `RemoteChainKey::create_message_key`
инкрементируют внутренний index после выдачи текущего message key. Дополнительно
`RatchetCount::advance` считает DH turns обеих сторон, не g только одной lane, и
использует обычное `count + 1` (`session/double_ratchet.rs:299–325`;
`session/chain_key.rs:81–112`). Проверка только representability Core.expected.index
или каждого g отдельно не доказывает отсутствие overflow внутри вызова.

**Требуемое исправление:** задать verified предикат для pre/post native index и
общего DH turn count, выведенный из authenticated guards/selected origin через
public API и контрактную последовательность переходов. Оговорить genesis,
fresh/imported L1 и отсутствие private-pickle parsing. Допустим консервативный
checked bound; это предел представления, не новый небольшой lifetime budget.

**Приёмка:** короткий вывод счётчиков на A0/B0/A1/B1 и последующих turns плюс
обычные checked-arithmetic unit vectors около u64 maximum. Не конструировать и не
запускать huge native state/protocol traffic; тестировать именно preflight predicate.
Предельная арифметика не заменяет уже запланированный реальный one-way>512 test.

## Что проверено и следующее поручение

Root прочитал весь контракт и исходный task, сверил актуальные ratchet/outbox/
ingress/engine, schema validation/owner lock/recovery и локальные pinned redb/
vodozemac sources. Новые runtime/harness tests не запускались: реализации ещё нет.
Чужой `development-environment.md` сохраняет SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
HEAD остаётся860b351. Supporting A2 и его ограничения не меняются.

Тот же исполнитель GPT-6.1 Sol Max/default без Fast исправляет только contract,
добавляет компактный disposition R1–R3 и возвращает один substantive callback.
Самостоятельно продуктовую реализацию, commit/push, новые чаты/агенты не начинать.
После повторного review root примет целый согласованный контракт и поручит
publication точных docs, затем реализацию с предусмотренными review gates.
