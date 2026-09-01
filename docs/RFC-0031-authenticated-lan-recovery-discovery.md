# RFC-0031: authenticated LAN history recovery discovery (M0.9.4)

Статус: реализовано в M0.9.4

Дата: 2026-09-02

## 1. Задача

M0.9.2 и M0.9.3 требуют вручную перенести signed recovery URI либо QR image.
M0.9.4 добавляет минимальный discovery-механизм для устройств в одной локальной
сети, не превращая обнаружение в доверие или согласие:

- source только по отдельному opt-in периодически публикует уже существующую
  recipient-specific URI;
- recipient пассивно собирает предложения, криптографически проверяет их и не
  открывает Iroh connection;
- найденная ссылка всё ещё требует отдельного `history-recovery-link-accept` и
  ручного подтверждения role-bound SAS;
- source после подключения независимо проверяет device authorization и своё
  локальное exact consent window.

Слово authenticated относится к содержимому descriptor: UDP multicast сам по
себе не аутентифицирован, но подделанная датаграмма не проходит Root/source
signature verification.

## 2. Публикация

Source добавляет к прежнему consent-gated listener:

```powershell
kilogram-cli listen `
  --state-dir .\source `
  --allow-account <ACCOUNT_ID> `
  --device-list-file .\account.devices `
  --peer-prekey-pool-file .\recipient.pool `
  --history-rewrap-conversation example `
  --history-rewrap-recipient-device <RECIPIENT_DEVICE_ID> `
  --history-rewrap-approve-sas 123-456-789-012 `
  --history-rewrap-range-start 0 `
  --history-rewrap-count 256 `
  --history-recovery-discovery-publish
```

Без этого флага listener ничего в LAN не публикует. При opt-in он отправляет
точный ASCII URI M0.9.2 каждые 750 ms на IPv4 administratively scoped multicast
`239.255.75.71:45371` с TTL 1. Дополнительная копия уходит на
`127.0.0.1:45371`, чтобы поддержать два локальных CLI-процесса и стабильный
same-host test даже при проблемном multicast route. Loopback не расширяет
сетевую область видимости.

Публикация живёт только пока активен конкретный one-connection listener. URI
действует не более часа и обычно 10 минут. В датаграмму не добавляется новый
unsigned envelope: endpoint, source/recipient Device ID, account authority,
conversation, range, page size, route policy и expiry уже связаны source
signature M0.9.2.

## 3. Обнаружение и проверка

Recipient запускает:

```powershell
kilogram-cli history-recovery-link-discover `
  --state-dir .\recipient `
  --conversation example `
  --expect-source <SOURCE_DEVICE_ID> `
  --wait-seconds 3 `
  --output-link-file .\discovered-recovery.link
```

`--expect-source` необязателен. Без него допустим любой корректно
авторизованный source device того же account. До принятия candidate scanner
проверяет:

1. bounded ASCII URI и versioned Kilogram prefix;
2. Root-signed complete device list и source signature;
3. expiry и exact local recipient certificate;
4. exact local Conversation ID и наличие recipient account в локально
   установленном membership snapshot;
5. optional exact source Device ID;
6. отсутствие rollback/equivocation относительно локальной authority revision.

Более новая корректно Root-signed authority revision может быть показана как
candidate; её durable install и повторная проверка выполняются существующим
accept/coordinator path. Более старая revision и другая snapshot на той же
revision игнорируются.

Scanner выводит Link ID, Account/Device IDs, Conversation ID, authority
revision, SAS, Endpoint ID, route policy, expiry и полную URI. Output file
создаётся с no-clobber semantics только если за весь bounded scan найден ровно
один candidate. Ноль завершается ошибкой. Несколько candidates или достижение
candidate cap считаются неоднозначностью: автоматического выбора нет.

## 4. Resource bounds

- scan длится `1..=30` секунд, default 3;
- принимается не более 4096 bytes на UDP datagram;
- signed URI по-прежнему ограничена 2953 bytes;
- один scan обрабатывает не более 512 датаграмм;
- собирается `1..=16` уникальных candidates, default 8;
- повторы одной signed URI дедуплицируются;
- malformed, wrong-recipient, expired и cryptographically invalid публикации
  учитываются как rejected и не прерывают scan;
- publisher использует один bounded background task, который отменяется вместе
  с listener.

Эти границы не являются полноценной anti-DoS защитой общей LAN. Они лишь не
дают одному CLI scan бесконечно накапливать данные или кандидатов.

## 5. Граница согласия и приватности

Успешный discovery всегда печатает:

```text
history_recovery_discovery_user_consent=not-granted
connection_attempted=false
```

Чтобы начать передачу, пользователь отдельно сравнивает SAS и запускает:

```powershell
kilogram-cli history-recovery-link-accept `
  --state-dir .\recipient `
  --link-file .\discovered-recovery.link `
  --conversation example `
  --confirm-sas 123-456-789-012
```

Multicast раскрывает всем наблюдателям этой LAN публичные account/device/
conversation identifiers, endpoint coordinates, размер recovery plan и время
активности source. E2EE keys и plaintext history в descriptor отсутствуют, но
это не делает publication анонимной или unlinkable. Поэтому функция выключена
по умолчанию и не является подходящим global discovery, mailbox либо privacy
relay.

## 6. Совместимость

Recovery URI v1, ticket v9, ALPN `kilogram/m0/sync/7`, wire messages и
checkpoint format не изменились. QR/text transfer остаются полностью
совместимыми fallback-вариантами. В workspace включён только Tokio `net` feature;
новых runtime dependencies нет.

## 7. Проверки

- unit test проверяет opt-in UDP publication, bounded receive и deduplication;
- все 110 workspace tests проходят;
- Windows direct process smoke `.tmp/m094-smoke-20260902-015219` подтвердил
  multicast join, три полученные публикации и дедупликацию двух повторов;
- wrong device получил ноль verified candidates и не вызвал connection;
- exact recipient обнаружил одну ссылку без connection, затем отдельным accept
  открыл один authenticated connection;
- две atomic pages создали два signed checkpoints и byte-identical source/
  recipient history.

## 8. Что остаётся дальше

- authenticated wide-area descriptor publication через DHT/gossip/mailbox с
  privacy-preserving lookup и first-contact freshness;
- несколько simultaneous local clients и явный interface selection;
- GUI candidate picker, live camera/clipboard и OS deep-link handler;
- background retry scheduler с power, Wi-Fi/Ethernet и metered/mobile policy;
- multi-source recovery claims и безопасный recovery от устройства собеседника;
- rate limiting, Sybil/eclipsing resistance и platform firewall UX.
