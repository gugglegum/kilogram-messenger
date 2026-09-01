# RFC-0028: bounded multi-page history recovery session (M0.9.1)

Статус: реализовано в M0.9.1  
Дата: 2026-09-02

## 1. Задача

M0.7.9 сделал восстановление истории возобновляемым и crash-consistent, но
каждая страница требовала нового listener, ticket и ручного запуска команды.
Криптографическая безопасность уже была достаточной для продолжения после
сбоя, однако orchestration оставался слишком ручным для будущего фонового
клиента.

M0.9.1 добавляет первый ограниченный coordinator поверх прежнего протокола:

- один явно одобренный source listener обслуживает несколько смежных страниц;
- recipient использует одно аутентифицированное Iroh-соединение и один ticket;
- каждая страница по-прежнему отдельно подписывается, проверяется и атомарно
  коммитится вместе с очередным checkpoint;
- сессия ограничена 64 страницами и окном source consent;
- после лимита или обрыва следующий ticket продолжает ту же checkpoint chain.

Это ещё не постоянный background daemon и не автоматический поиск устройств.

## 2. Неизменные границы доверия

Coordinator не расширяет authority. Пользователи по-прежнему должны явно
выбрать source Device ID, подтвердить общий Account ID и независимо сравнить
role-bound SAS. Source одобряет ровно один conversation, recipient device и
окно `[range_start, range_start + count)`.

Каждый page request:

1. подписан recipient device и привязан к transport session;
2. целиком лежит внутри одобренного окна;
3. начинается с конца предыдущей успешно выданной страницы этой сессии;
4. содержит не более 256 событий;
5. получает source-signed transfer, связанный с точным request.

Source использует один immutable DB-primary event/projection snapshot на всю
сессию. Поэтому inventory count/digest не может измениться между страницами из-за
параллельной локальной записи.

## 3. Source session

После device authorization listener принимает первый `HistoryRewrap` request и
может принять следующие bidirectional streams того же QUIC/Iroh connection.
После каждой страницы он хранит только три элемента transient progress:

- число выданных страниц;
- ожидаемый `range_start` следующей страницы;
- признак достижения меньшего из конца consent window и source inventory.

Несмежный request отклоняется как approval mismatch. После 64 страниц listener
закрывает сессию независимо от размера одобренного окна. Между страницами он
ждёт следующий stream не более 60 секунд; закрытие connection recipient-ом
считается штатной остановкой, а не разрешением повторно использовать ticket.

Одностраничный `history-rewrap-fetch` совместим с новым listener: после ответа
он закрывает connection, и source завершает сессию с `session_complete=false`,
если окно ещё не исчерпано.

## 4. Recipient coordinator

`history-recovery-resume` получил `--max-pages` со значением по умолчанию 64.
Значение обязано лежать в диапазоне `1..=64`; `--page-size` остаётся в диапазоне
`1..=256`.

Recipient один раз проверяет ticket, source certificate, signed device list,
Account ID, exact Device ID, SAS и conversation membership, затем открывает
одно соединение. Для каждой страницы он:

1. вычисляет request из последнего durable checkpoint;
2. проверяет source transfer и неизменный inventory claim;
3. дешифрует и авторизует все entries;
4. одной state transaction сохраняет bundle, transfer, events, projections и
   новый recipient-signed checkpoint;
5. только после commit запрашивает следующую страницу.

Если сеть или процесс завершаются между страницами, уже committed страницы не
теряются. Если ошибка произошла до commit текущей страницы, checkpoint не
продвигается. Достижение `--max-pages` без завершения плана является успешной
паузой со статусом `history-recovery-session-paused`, а не потерей прогресса.

## 5. Boundedness и ресурсные ограничения

Полный M0 inventory ограничен 4096 event IDs. Source consent window теперь также
не может заканчиваться за этой границей. При стандартном `page_size=64` лимит
64 страниц позволяет покрыть весь максимальный M0 inventory одним соединением.
Меньшие страницы могут потребовать следующую сессию.

CLI coordinator исполняется в отдельном потоке со стеком 8 MiB. Это не часть
wire protocol: граница нужна из-за крупного debug async state machine и
предотвращает platform-dependent stack overflow до запуска выбранной команды.

## 6. Совместимость

Wire objects, signatures, ticket v9 и ALPN `kilogram/m0/sync/7` не меняются.
Каждая страница использует прежние `SignedHistoryRewrapRequest` и
`SignedHistoryRewrapTransfer`; новая семантика состоит только в нескольких
последовательных streams одного уже авторизованного connection.

Новый source совместим со старым одностраничным recipient. Новый recipient при
работе со старым source успеет атомарно сохранить первую страницу, после чего
получит закрытие connection; повтор с новым ticket безопасно продолжит plan.
Для автоматической многостраничной сессии обе стороны должны использовать
M0.9.1 или новее.

## 7. Проверки

- unit regression проверяет обязательную смежность страниц, completion stop и
  hard cap 64;
- прежние protocol/state regressions продолжают проверять signature chain,
  inventory claim, rollback и atomic import;
- все 103 workspace tests, rustfmt, strict Clippy и release build проходят;
- Windows direct process smoke `.tmp/m091-smoke-20260902-004647` одним ticket и
  одним authenticated connection передал страницы `0..2` и `2..3`, создал два
  immutable checkpoint и получил byte-identical verified history на source и
  recipient.

## 8. Что остаётся дальше

- автоматический discovery доступных same-account source devices;
- постоянный background scheduler с power/network policy и retry backoff;
- QR/device-link ceremony, которая передаст Account ID, exact device role, SAS
  и bootstrap coordinates без ручного копирования;
- multi-source scheduling и UI для divergent/incomplete claims;
- compact Merkle/range summary вместо полного bounded inventory.

Компактный signed descriptor и явное принятие QR-ready device link реализованы
в M0.9.2 и описаны в
[`RFC-0029`](RFC-0029-signed-history-recovery-device-link.md). Автоматическая
публикация/discovery и QR renderer остаются следующими отдельными срезами.
