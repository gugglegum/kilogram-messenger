# A3-S3: review integration plan, требуется уточнение storage seam

2026-10-25, Asia/Yekaterinburg. Проверен [план](long-lived-dialog-a3-protocol-s3-plan.md),
62252 bytes, SHA256 `3acd22a1557dd1acb29a8fda71b96af28fdae7df11b9a9e13fd9d2dd013a8ece`.
Baseline HEAD `e136c59b829c6ecc5eb501273f25091e9b9e4789`.

**Контракт следующей реализации пока не принят.** Общая граница libraries + bounded
guard + closed writer подходит; до Rust edits нужно устранить R1 и конкретизировать
R2. Это уточнение текущего S3, без изменения принятого A3 wire/native решения.

## Что подтверждено

Root прочитал весь план и адресно сверил принятый A3 contract/review2, Codec1,
Guard1, byte-oriented native adapter, S2 HistoryOperation/Binding/private lowering,
retention checks и направления зависимостей Cargo. В pinned vodozemac повторно
сверены Active0/Inactive0 constructors и ветвление advance для Inactive.

План сохраняет единый Q и exact old bytes, checked j+1/T+1 до native, отсутствие
растущего seen-set, проверку generated own key после trial, отказ от неизвестного
pickle происхождения и atomic native/source/seen/receipt. Scope честно отделён от
production bootstrap, current policy, migration и полного two-process A3.
Private bounded structural reader в state допустим при ясной границе с crypto
verification и проверке согласованности с Codec2; новая dependency edge не требуется.

## R1 — genesis request несовместим с текущим storage Binding

План требует ненулевой в общем случае wire request: первые16 bytes Offer/Grant ID,
и обещает выводить request из exact source перед существующим private lowering.
Но [Binding::check](../../crates/kilogram-state/src/vault/indexed_dialog/history_types.rs#L420)
для любого genesis требует `request == [0;16]`. Этот check вызывается при построении
row, encode и decode сохранённого Binding. Непосредственная реализация предложенного
пути отвергнет корректные A0/B0 ещё до проверки долговременного диалога.

Нужно выбрать и описать точное соответствие wire genesis request и stored projection
metadata, включая reader/reopen и существующие HPB1/S2 fixtures. Допустимо отдельное
явное представление wire metadata или документированная normalization, если signed
raw/Core сохраняются и проверяются, но нельзя молча подставлять нули в wire либо
ослаблять R4. Не менять общий Binding predicate без описания совместимости и проверки
старого trusted storage контракта.

Добавить positive A0/B0 с заведомо ненулевыми request prefixes: commit, read/reopen,
exact raw/Core/ID. Отдельно incorrect request с корректной Device signature должен
отклоняться до native; корректное storage представление не легализует wrong request.

## R2 — точно определить resolution и сохранение authority bytes

План оставляет `Vec<HistoryAuth>` и повторно использует CapturePending, но существующий
[add_auth](../../crates/kilogram-state/src/vault/indexed_dialog/history_writer.rs#L79)
проверяет только размер и пишет bytes под caller-supplied id. Новая секция source/
receipt correspondence не задаёт точного алгоритма проверки этих аргументов при
прямом вызове нового state API. Фраза «сравнить receiver cert/auth refs со source»
также недостаточна: source содержит author_auth_ref автора, а receipt — credential
другой стороны; эти hashes не должны просто равняться друг другу.

Для bound2 описать таблицу original author / admission author / final readable author /
receipt author: откуда получается exact bundle, какие refs берутся из source,
receipt либо trusted adapter/current-policy context, когда разрешён уже сохранённый
Auth record, как проверяются raw hash, certificate reference и role. Renewal может
менять authority/AuthRef, сохраняя Device certificate; его нельзя случайно запретить
сравнением с прежним auth hash. Root signatures/current revocation остаются у adapter,
state выполняет обещанные structural/hash correspondence checks.

Уточнить fencing старых AcceptPending/CapturePending для bound2 и поведение при
missing/Integrity/IO, дублированных и unused auth аргументах. Нельзя принимать caller
Auth id, не совпадающий с BLAKE3 raw, или снимать own debt без resolvable receipt auth.
Описать, какие из четырёх credential roles сохраняются в каждом operation, чтобы
retention/accounting и repeated ACK не создавали лишних постоянных записей.

Добавить bounded negatives для wrong raw hash, missing referenced bundle и wrong
opposite certificate; positive renewal с тем же cert и новым signed authority.
Чистая storage проверка correspondence не должна объявляться crypto/policy proof.

## R3 — уточнить differential matrix

План обещает structural state reader для source/receipt/proof, но называет все28 C01
fixtures, включая21 Frame kinds, входами одного differential set. Нужно перечислить
точные поддерживаемые reader entrypoints и пары Codec2/state. Не расширять state до
дублирующего полного Frame codec ради счётчика48 и не засчитывать reject-unsupported
как проверку совпадения extracted fields. Raw fixtures для Frame остаются codec tests.
Числа vectors после уточнения должны соответствовать реально перечисленным cases.

## Следующая сдача

Только исправленный S3 plan и краткий список изменений R1–R3. Текущую review версию
сохранить. Rust/helper/Cargo/dependencies, frozen S1/S2 evidence/fixtures и чужие файлы
не менять; heavy tests не нужны. Проверить ссылки/whitespace и вернуть bytes/SHA.
Implementation и commit/push до нового root review не поручены.

Предыдущий callback исполнителя отклонён auto-review; root получил готовый результат
через read_thread и локальный файл. Это не новое human authorization на callback.
Повторять отклонённую обратную отправку без признанного разрешения не нужно:
результат исправления достаточно оставить в финальном ответе чата исполнителя,
root может прочитать его штатным read-only инструментом.
