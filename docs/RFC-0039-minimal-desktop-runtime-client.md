# RFC-0039: Minimal desktop runtime client (M0.9.12)

Статус: реализовано в M0.9.12

## 1. Цель

M0.9.11 отделил authenticated local IPC от CLI и оставил runtime единственным
владельцем device state. M0.9.12 добавляет первый оконный клиент, который
подключается к уже запущенному runtime, показывает его identity и durable
outbox, а также ставит новые сообщения в очередь.

Этап не меняет P2P wire protocol, E2EE, ratchet или формат state. GUI не знает
расположение `STATE_DIR`, не загружает ключи и не становится вторым writer.

## 2. Приложение и зависимости

Новый workspace package `apps/kilogram-windows` собирает
`kilogram-windows.exe`. Собственный код использует safe Rust; нативное окно
создаёт `eframe`/`egui` 0.33.3. Эта версия поддерживает Rust 1.88, поэтому
workspace сохраняет заявленный MSRV 1.91. UI-код не привязан к Win32 API и
может стать общей desktop-основой для macOS/Linux, но M0.9.12 проверяется и
поставляется только как Windows slice.

Crate зависит от `kilogram-runtime-ipc` и публичного `AccountId` parser. Он
намеренно не зависит от `kilogram-state`, `kilogram-store`, ratchet/session или
transport crates.

## 3. Подключение

Путь к private descriptor задаётся:

```powershell
kilogram-windows.exe --ipc-file C:\private\runtime.ipc.json
```

Его также можно изменить в поле окна или перетащить JSON-файл в окно. GUI не
открывает и не отображает bearer token: `kilogram-runtime-ipc` загружает файл,
проверяет device signature и loopback address, затем выполняет `Ping`.
Полученные Account ID и Device ID показываются пользователю.

Runtime должен быть запущен отдельно с тем же `--ipc-file`. Descriptor остаётся
machine-local secret и не должен попадать в Yandex Disk или другой sync.

## 4. Отправка сообщения

Composer принимает существующий conversation label, peer Account ID и body.
До IPC проверяются:

- непустой conversation не длиннее 4 KiB;
- 64-символьный hexadecimal Account ID;
- непустой message не длиннее 64 KiB.

GUI генерирует 256-bit `RuntimeIpcRequestId` перед первой попыткой. Если ответ
IPC потерян или завершился timeout, неизменённый draft повторно использует тот
же ID. Runtime поэтому возвращает существующую Queue ID вместо создания
дубликата. Изменение любого поля draft создаёт новую request identity.

Успешный ответ очищает только body; conversation и peer остаются для
продолжения диалога. Plaintext существует в UI/process memory и IPC request,
но GUI не записывает его на диск. Runtime немедленно шифрует durable outbox по
правилам M0.9.10.

## 5. Асинхронность и bounded polling

Оконный event loop никогда не выполняет blocking IPC. Один выделенный worker
thread содержит current-thread Tokio runtime и последовательно выполняет
`Ping`, `QueueMessage` и `OutboxStatus`. В UI одновременно допускается одна
операция, поэтому пользователь не создаёт конкурирующие queue mutations.

После подключения GUI опрашивает `OutboxStatus` не чаще одного раза в две
секунды. Ответ показывает contact/queue/pending/materialized/delivered/retry
counters и metadata каждой Queue ID без plaintext. Ошибка refresh переводит
индикатор в disconnected; явный reconnect повторно проверяет descriptor.

## 6. Проверка

- pure view-model tests проверяют validation, сохранение whitespace в body,
  feedback, compact IDs и безопасное повторное использование request ID;
- integration unit test поднимает настоящий подписанный loopback
  `RuntimeIpcServer` и пропускает GUI adapter через `Ping` → `QueueMessage` →
  `OutboxStatus`;
- rustfmt, strict Clippy, все workspace tests и release build должны проходить.

## 7. Оставшиеся ограничения

Это первый operational shell, а не готовый messenger UI. Runtime IPC пока не
возвращает contact list, conversation summaries или локальную расшифрованную
history и не принимает contact onboarding. Поэтому M0.9.12 требует заранее
добавленного CLI contact и ручного ввода conversation/Account ID. GUI также не
запускает runtime, не регистрирует autostart и не сохраняет настройки.

Следующий этап M0.9.13 — actor-owned read model и bounded IPC-команды для
contacts, conversation list и paginated local history. После этого desktop UI
сможет стать обычным экраном списка чатов, не обходя single-writer boundary.

Примечание 2026-09-03: этот следующий срез реализован в
[`RFC-0040-actor-owned-chat-read-model.md`](RFC-0040-actor-owned-chat-read-model.md).
