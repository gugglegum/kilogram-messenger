# Технические этапы

Актуально на: 2026-08-25.

## M0 — проверка сетевого и репликационного фундамента

### M0.1 — два процесса на одном хосте: выполнено

Реализовано:

- Git-репозиторий и Rust workspace;
- `kilogram-cli` с командами `listen` и `connect`;
- Iroh 1.0.3, QUIC и взаимно аутентифицированные Endpoint IDs;
- публичный base64url connection ticket без секретного ключа;
- передача ограниченного UTF-8 сообщения и acknowledgement;
- ограничение размера входного сообщения;
- unit-тесты round-trip и version rejection для ticket;
- строгие fmt, Clippy `-D warnings` и tests.

Проверено локально:

```text
Client endpoint 2512...5626
    -> hello from kilogram m0 smoke test
Listener endpoint 2397...23d9
    -> ack:hello from kilogram m0 smoke test
```

Это transport proof of concept, а не защищённый мессенджер. Endpoint identity
пока эфемерна; message-level E2EE, Account Identity, подпись события, история и
синхронизация отсутствуют.

### M0.2 — два хоста в одной LAN: не начато

- Передать ticket на второй физический хост.
- Проверить Windows Firewall и direct LAN path.
- Зафиксировать выбранный Iroh path и сетевую диагностику.

### M0.3 — два хоста в разных сетях: не начато

- Проверить hole punching при двух NAT.
- Отдельно принудительно проверить public relay fallback.
- Проверить reconnect и смену сетевого интерфейса.

### M0.1.1 — постоянная device identity и signed event: выполнено

Реализовано:

- отдельная прикладная Ed25519 device identity, не связанная с эфемерным Iroh
  Endpoint ID;
- сохранение development device secret и author sequence в явно заданном
  `--state-dir`;
- минимальная детерминированная модель `SignedEvent` с domain separation;
- BLAKE3 event ID, conversation ID, causal parents и лимиты входных данных;
- подписанные `Text` и `Acknowledgement`, строгая проверка подписи получателем;
- привязка публичного device ID listener к connection ticket и проверка автора
  acknowledgement клиентом;
- временный Postcard codec и новый ALPN `kilogram/m0/signed-event/1`;
- unit-тесты persistence, sequence, signature binding, tampering, versioning и
  causal acknowledgement;
- два успешных локальных прогона до и после перезапуска обоих процессов.

Проверка перезапуска показала: transport Endpoint IDs изменились, application
device IDs остались прежними, а sequence отправителя изменился с `0` на `1`.

Ограничения этого шага:

- secret key хранится без шифрования и пригоден только для разработки;
- allocator sequence не поддерживает два процесса на одном state directory;
- device key ещё не авторизован Account Root Identity;
- payload пока не имеет message-level E2EE и не сохраняется в историю;
- codec, схема события и derivation conversation ID не являются финальным wire
  protocol.

### M0.1.2 — локальный append-only event store: выполнено

Реализовано:

- отдельный crate `kilogram-store`;
- неизменяемый content-addressed файл на каждый проверенный signed event;
- запись через temporary file и atomic no-clobber persist;
- идемпотентная повторная вставка и ошибка при конфликте event ID;
- проверка подписи, event ID, имени файла и conversation directory при чтении;
- запрет equivocation: один device sequence не может обозначать два разных
  события в одном conversation;
- сохранение исходящего `Text` до сетевой отправки;
- сохранение входящего `Text` до создания acknowledgement;
- сохранение исходящего acknowledgement до отправки и входящего — до успешного
  завершения команды;
- вычисление causal frontier; новое сообщение ссылается на предыдущие локальные
  heads;
- команда `history` для проверки локальных events и frontier после перезапуска;
- 5 unit-тестов store: persistence, deduplication, corruption detection,
  writer-sequence conflict и frontier.

Storage smoke test после двух последовательных соединений и перезапуска:

- у Alice и Bob по 4 одинаковых события;
- у обоих один одинаковый frontier;
- второе сообщение причинно ссылается на acknowledgement первого;
- application device IDs и author sequences продолжаются между запусками.

Ограничения:

- event store хранит M0 plaintext без at-rest encryption;
- это проверка модели, а не выбранная production database;
- хранение рассчитано на один процесс на `state-dir`;
- история пока не синхронизируется, если один из участников пропустил событие;
- вывод `history` отсортирован по event ID и не является timeline ordering.

### Следующее расширение M0.1

1. Реализовать bounded протокол обмена inventory/summary для одного
   conversation.
2. Передавать отсутствующие events, повторно проверять и идемпотентно сохранять
   их на принимающей стороне.
3. Проверить восстановление намеренно пропущенного события после перезапуска.
4. Затем выделить transport interface из CLI перед M0.2 на двух хостах.
