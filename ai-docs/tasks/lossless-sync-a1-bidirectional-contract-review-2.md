# OD1 contract — приёмка iteration 2 и запуск реализации

2026-10-23. Baseline до сохранения контракта:
`1fce03ee2d2886f871e0ce44a641e3eeba9cdd0f`.
Проверены [контракт](lossless-sync-a1-bidirectional-contract.md) iteration2,
[исходная версия](lossless-sync-a1-bidirectional-contract-iteration1.md) и
[замечания C1–C3](lossless-sync-a1-bidirectional-contract-review-1.md).
Входной SHA-256 iteration2 до обновления отметки приёмки:
`39c6f3a95fef6acab9b01c8740ed0a051316cbb8f3b2cf1f0b253444f7b4f78d`.
Архив iteration1 сохранил SHA-256
`77c0c69babc7363353bcc422d144c057c374382a993560192415bdfca37686ab`.
Финальный SHA-256 принятого контракта после обновления только статуса:
`587cf786cff1cb4e946a68d37ea508f82141b27e4909e80b2b95236485f54b18`.

**Контракт принят. C1–C3 закрыты на уровне дизайна; разрешена реализация
согласованного этапа после commit/push принятого комплекта документов.**
Новых блокирующих противоречий в проверенных исправлениях не обнаружено.
Ещё один документальный repair cycle не требуется. Это не приёмка runtime-кода,
не доказательство всех security properties и не завершение A1/MVP.

## Проверенное закрытие замечаний

| Пункт | Принятое исправление | Gate реализации |
| --- | --- | --- |
| C1 | Стабильный C namespace, current authenticated ContextHead, отдельные Absent/OfferedA/GrantedB/Selected, доступные на каждой фазе bindings и разные HPKE info/AAD. Нет зависимости раннего Account от ещё неизвестного SID. Cutover, OTK consumption, genesis/native/outbox и Head/blob согласованы одним primary commit; shadow не выбирает состояние. | Restart после Offer/Grant, до/после Selected primary commit, exact Account/OTK/bytes, swapped/stale/cross-context blobs и реальные IO failures. |
| C2 | Durable enabled_contexts запускает maintain A независимо от outbox/IPC; один auth creator, bounded reconnect/idle probe/reauth. Ручной locator export/import явно отделён от автоматического reconnect при действующих pins. | B-only send, разрыв/relaunch idle A без open/send на A; новый signed locator на B при необходимости; lease/locator/consent expiry и recovery без смены E/SID. |
| C3 | Удалено обещание несуществующего remote possession inventory. После recovery receiver сам drain/fetch от receive_next; source сохраняет ledger, prioritizes head repair и queries exact ACK. Resume/credit не трактуются как pending IDs. | Full64 partial/lost-Complete, source без knowledge accepted subset, head reserve, автоматические drain batches, byte-exact replay и отсутствие ложного completion/ACK. |

Сверен diff iteration1→iteration2 (313 добавлений/25 удалений до отметки приёмки)
и соответствующие нормативные разделы, схемы и traces. Для C1 дополнительно
проверены настоящие HPKE seal/open/SealedMessage поля в
`crates/kilogram-crypto/src/lib.rs` и публичные encrypt/from_encrypted APIs native
AccountPickle/SessionPickle в закреплённом vodozemac 0.10.0. Поля private JSON не
нужны. Head/blob hash не включён циклически в собственный sealing AAD.

Проверены локальные ссылки, whitespace, архивный hash, отсутствие product/manifests/
scripts diff и пустой index. Арифметика: 128 MiB/context, 4160 KiB/pending lane,
334.25 MiB/profile; partial trace 47+17=64, drain 65 records требует девяти batches≤8.
Это статические проверки контракта. Rust builds/tests и прежний 18-минутный
acceptance не запускались. Фактический encode/staging fit должен доказать T9.

## Существенные границы приёмки

- Один committed/published результат на logical request; unpublished volatile trial
  после crash можно повторить. После durable materialization ciphertext неизменен.
- Fresh-only OD1/L1, одна epoch/context, максимум 511 Text на lane. Этот предел
  временный для этапа, не daily-MVP capacity. Reset/eviction ради продолжения запрещены.
- Direct local process acceptance с настоящим handshake/transport, без trusted
  session fixture или подмены transport binding. Synthetic identities допустимы, настоящая
  проверка credentials и полный protocol flow обязательны.
- Operator-assisted locators, expiry до часа и необходимость import после некоторых
  restarts остаются явными ограничениями. Automatic B-only progress проверяется
  после выполнения этих prerequisites; seamless discovery/recovery не заявляется.
- T1–T12 остаются обязательными; старые single-chain evidence не подменяют ни одну
  новую runtime/handshake/переключение-DH гарантию. Mailbox/relay/physical two-host,
  arbitrary profile migration и A2 до внешнего использования — отдельный scope.

## Следующее действие исполнителя

1. Сохранить принятый комплект документов обычным commit/push в существующий
   `origin/master`, сверив exact paths, hashes, remote и исключённый power audit.
   Нормативные разделы принятого контракта при подготовке commit не менять.
2. После подтверждённого push **без нового вопроса пользователю** начать реализацию
   [согласованного задания](lossless-sync-a1-bidirectional-task.md) по этому контракту.
   Полученный documentation commit — baseline нового implementation evidence.
3. До изменения кода сохранить проверяемый old-baseline CLI для T10 в постоянный
   compatibility EXE path, с source/EXE hashes. Текущий новый binary с legacy flag
   не заменяет старый исполняемый файл. Windows harnesses — только stable runner.
4. Реализовать предусмотренные модули, mode/journal gates и настоящие CLI/runtime
   paths, затем T1–T12 и применимые fmt/Clippy/readiness проверки. Не расширять
   задачу на M1.2.63/GUI/mailbox или новый общий design series. Найденное фактическое
   противоречие контракту вернуть с конкретным trace, не обходить ослаблением guards.
5. После реализации — один содержательный callback с documentation baseline,
   изменёнными файлами, полным reviewable diff/source+binary hashes, командами,
   outcome каждого T1–T12 и ограничениями. До code review не делать commit/push
   продуктового diff; принятый контракт уже сохранён отдельно. Затем обычный
   review/repair и commit/push после приёмки реализации.

Разрешение человека остаётся userMessage `01a1661b-1561-7ec0-9380-b2835fd35738`
в чате оркестратора, host local. Оно включает эту последовательность и callbacks.
`ai-docs/development-environment.md` исключён; его SHA-256 по-прежнему
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
