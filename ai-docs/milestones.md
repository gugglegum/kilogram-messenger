# Технические этапы

Актуально на: 2026-08-24.

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

### Следующее расширение M0.1

1. Выделить transport interface из CLI.
2. Добавить постоянный device key в локальное хранилище разработки.
3. Определить минимальный canonical signed event.
4. Передавать и проверять подписанное событие вместо строки.
5. Сохранить событие локально и синхронизировать после перезапуска.
