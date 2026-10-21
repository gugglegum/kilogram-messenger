# Среда разработки

## M1.2.53 verification (2026-10-22)

- Workspace locked/offline check passed. Initial sandboxed focused-test launch
  could not create the Windows temp fixture root; the identical approved
  stable-path launch with normal temp access passed and exposed no code error.
- Two new recovery-assembly issuer fixtures passed 2/2 in 119.36s, then the
  combined M1.2.47–M1.2.53 guided-recovery regression passed 12/12 in 1368.98s
  through `scripts/run-cargo-tests-stable.ps1`; direct `cargo test` was not
  used and the executable path remained stable.
- Dedicated issuer boundary rejected 17/17 weakening mutations. Host,
  installation-owner, start-producer, local-UX and controller predecessor
  boundaries retained 18/18, 21/21, 17/17, 15/15 and 14/14. Accepted-tag M1
  readiness passed 179/179 gates with rustfmt and workspace-wide locked/offline
  strict Clippy.
- Product network, second host, release build, archive, tag and automatic
  GitHub build were not used. Ordinary startup creates no issuer/owner/host;
  CLI/desktop/runtime IPC/timer/background remain unregistered and public
  default remains `legacy-only`.
- Cargo target after regression is 54.59 GiB. Cache is retained for build
  speed; no verification or incremental pruning was requested in this stage.

## M1.2.52 verification (2026-10-22)

- Targeted locked/offline `cargo check` и strict Clippy для recovery
  runtime/execution passed; обычные cross-volume incremental hard-link fallback
  warnings не изменились.
- Два новых host-installation owner fixtures прошли 2/2 за 117.98s, затем
  combined guided-recovery regression M1.2.47–M1.2.52 прошёл 10/10 за 1259.16s
  через `scripts/run-cargo-tests-stable.ps1` с постоянным EXE и
  workspace-local TEMP/TMP; direct `cargo test` не использовался.
- Dedicated installation-owner boundary rejected 21/21 weakening mutations;
  host/start-producer/local-UX/controller boundaries сохранили 18/18, 17/17,
  15/15 и 14/14. Accepted-tag M1 readiness прошёл 178/178 gates с rustfmt и
  workspace-wide locked/offline strict Clippy.
- Product network, second host, release build, archive, tag и automatic GitHub
  build не использовались. CLI/desktop/runtime IPC/timer/background entry point
  отсутствует; ordinary startup не создаёт owner/host, default — `legacy-only`.
- Cargo target после regression занимает 53.87 GiB; кэш сохранён ради скорости,
  disposable verification profile и incremental generations не очищались.

## M1.2.51 verification (2026-10-22)

- Targeted locked/offline `cargo check` и strict Clippy для recovery
  runtime/execution passed; обычные cross-volume incremental hard-link fallback
  warnings не изменились.
- Два новых process-owned host fixtures прошли 2/2 за 292.55s, затем полный
  combined guided-recovery regression M1.2.47–M1.2.51 прошёл 8/8 за 1156.88s
  через `scripts/run-cargo-tests-stable.ps1` с постоянным EXE и
  workspace-local TEMP/TMP; direct `cargo test` не использовался.
- Dedicated host boundary rejected 18/18 weakening mutations; соседние
  start-producer/local-UX/controller boundaries сохранили 17/17, 15/15 и
  14/14. Accepted-tag M1 readiness прошёл 177/177 gates с rustfmt и
  workspace-wide locked/offline strict Clippy.
- Product network, second host, release build, archive, tag и automatic GitHub
  build не использовались. CLI/desktop/runtime IPC/timer/background entry point
  отсутствует; ordinary startup не создаёт host, default — `legacy-only`.

## M1.2.50 verification (2026-10-21)

- Targeted locked/offline `cargo check` и strict Clippy для recovery
  runtime/execution passed; cross-volume incremental hard-link fallback
  остаётся обычным Cargo warning.
- Два новых one-shot producer fixtures прошли 2/2 за 159.64s, затем полный
  combined M1.2.47–M1.2.50 guided-session regression прошёл 6/6 за 862.90s
  через `scripts/run-cargo-tests-stable.ps1` с постоянным EXE и
  workspace-local TEMP/TMP; direct `cargo test` не использовался.
- Dedicated producer boundary rejected 17/17 weakening mutations; соседние
  controller/local-UX boundaries сохранили 14/14 и 15/15. Accepted-tag M1
  readiness прошёл 176/176 gates с rustfmt и workspace-wide locked/offline
  strict Clippy.
- Product network, second host, release build, archive, tag и automatic GitHub
  build не использовались.
- CLI/desktop/runtime IPC/timer/background entry point отсутствует; ordinary
  startup не получает handoff и остаётся idle/unattached, default —
  `legacy-only`.

## M1.2.49 verification (2026-10-21)

- Targeted locked/offline `cargo check` и strict Clippy для recovery
  runtime/execution passed; existing cross-volume incremental hard-link
  fallback warnings не изменились.
- Два новых local-UX fixtures сначала прошли 2/2 за 333.68s. После усиления
  preflight весь combined M1.2.48–M1.2.49 regression прошёл 4/4 за 635.68s
  через `scripts/run-cargo-tests-stable.ps1` с постоянным EXE; direct
  `cargo test` не использовался.
- Dedicated local-UX boundary rejected 15/15 mutations. Accepted-tag M1
  readiness прошёл 175/175 gates с rustfmt и workspace-wide locked/offline
  strict Clippy.
- Общий `invoke-cargo-friendly.ps1` теперь отличает обычный native stderr
  Cargo от failure по реальному exit code. Это устраняет ложное падение
  readiness на Windows PowerShell 5.1 при incremental hard-link fallback,
  не ослабляя проверку ненулевого завершения Cargo.
- Product network, second host, release build, archive, tag и automatic GitHub
  build не использовались. CLI/desktop/runtime IPC/timer остаются без нового
  entry point; ordinary startup idle/unattached, default — `legacy-only`.

## M1.2.48 verification (2026-10-21)

- `cargo check` и targeted locked/offline strict Clippy для recovery
  runtime/execution passed; existing cross-volume incremental hard-link
  fallback warnings не изменились.
- Два real recovery fixtures полного guided-session lifecycle и fail-closed
  phase matrix прошли 2/2 через `scripts/run-cargo-tests-stable.ps1` с
  постоянным именем EXE за 302.30s; direct `cargo test` не использовался.
- Dedicated controller boundary rejected 14/14 mutations. Accepted-tag M1
  readiness passed 174/174 gates, включая rustfmt и workspace-wide
  locked/offline strict Clippy.
- Product network, second host, release build, archive, tag и automatic GitHub
  build не использовались. Controller не зарегистрирован в runtime IPC,
  CLI/desktop или timer; ordinary startup остаётся idle/unattached, public
  default — `legacy-only`.

## M1.2.47 verification (2026-10-21)

- `cargo check` и targeted locked/offline strict Clippy для recovery
  runtime/execution passed; accepted-tag full workspace readiness затем passed
  rustfmt и locked/offline strict Clippy.
- Новый real recovery fixture
  `recipient_import_consumption_authenticated_producer_drives_loopback_exactly_once`
  passed 1/1 за 197.97s, а mutation-adapter regression
  `recipient_import_mutation_adapter_executes_persists_and_replays_exact_receipt`
  passed 1/1 за 157.61s. Оба запускались через
  `scripts/run-cargo-tests-stable.ps1` с постоянным EXE; direct `cargo test` не
  использовался.
- Dedicated authenticated-producer boundary rejected 12/12 mutations;
  historical attachment successor boundary rejected 11/11. Accepted-tag M1
  readiness passed 173/173 gates после согласованного обновления собственного
  inventory invariant с 172 до 173.
- Product network, second host, release build, archive, tag и automatic GitHub
  build не использовались. Ordinary CLI/runtime startup остаётся unattached;
  authenticated guided context появляется только из свежего успешного complete
  recipient-import mutation dispatch.

## M1.2.46 verification (2026-10-21)

- `cargo check` для recovery runtime и CLI passed locked/offline; existing
  cross-volume incremental hard-link fallback warnings не изменились.
- Real recovery actor fixture и M1.2.45 service regression passed 2/2 за
  387.46s через `scripts/run-cargo-tests-stable.ps1` с постоянным EXE; direct
  `cargo test` не использовался. После добавления exact-install replay и live
  replacement rejection actor fixture отдельно снова passed 1/1 за 203.38s.
- Dedicated attachment boundary rejected 11/11 mutations; historical v34
  envelope и M1.2.45 service boundaries passed their 10/10 and 8/8 successor
  checks. Accepted-tag M1 readiness passed 172/172 gates, rustfmt и
  locked/offline strict workspace Clippy. Targeted strict Clippy для recovery
  runtime/execution и CLI также passed.
- Product network, second host, release build, archive, tag и automatic GitHub
  build не использовались. Production runtime по умолчанию остаётся unattached.

## M1.2.45 verification (2026-10-21)

- `cargo fmt --all -- --check` passed.
- `cargo check -p kilogram-peer-history-recovery-runtime --locked --offline`
  passed with the existing cross-volume incremental hard-link fallback warning.
- The real recovery fixture
  `recipient_import_consumption_runtime_service_replays_exact_plan_and_consumes_once`
  passed 1/1 through `scripts/run-cargo-tests-stable.ps1`; direct `cargo test`
  was not used and the stable executable path was preserved.
- The dedicated service boundary rejected 8/8 negative mutations. Accepted-tag
  M1 readiness passed 171/171 gates, including rustfmt and locked/offline strict
  workspace Clippy; the ordinary moving-HEAD mode stopped only at its expected
  accepted-candidate revision boundary.
- No network, second host, release build, archive, tag or automatic GitHub build
  was used. The ordinary runtime handler intentionally remains unconfigured.

Проверено: 2026-08-31 во время внешней проверки M0.3.

## Текущий Windows-хост

- Host target: `x86_64-pc-windows-msvc`.
- Rust установлен через rustup: `rustc 1.98.0`, `cargo 1.98.0`.
- Установлены компоненты `rustfmt`, `clippy`, `rust-src`, `rust-docs`.
- Установлен target `x86_64-pc-windows-msvc`.
- Git: `2.45.1.windows.1`.
- Visual Studio 2022 Community установлен.
- MSVC: `14.44.35207`; `cl.exe` и `link.exe` присутствуют.
- Windows SDK: `10.0.28000.0` и `10.0.19041.0`.
- `cl.exe` не находится в обычном PATH, что допустимо вне Developer Shell.
  Cargo build и локальный Iroh smoke test успешны, следовательно MSVC linker и
  Windows SDK доступны Rust toolchain.
- CMake, Ninja и protoc в PATH не найдены. Для начального Iroh/Rust M0 они не
  считаются обязательными; добавлять только при появлении подтверждённой
  зависимости.

## Совместимость

Актуальный `iroh 1.0.3` требует Rust `1.91`. Текущая ветка OpenMLS также
указывает Rust `1.91`. Установленный Rust `1.98.0` удовлетворяет этим
требованиям. Workspace задаёт MSRV `1.91`, а `rust-toolchain.toml` фиксирует
toolchain `1.98.0` для воспроизводимой разработки.

Выполненная проверка:

1. `rustc --version --verbose` и `cargo --version` — успешно.
2. Компиляция минимального executable — успешно.
3. `cargo fmt --all -- --check` — успешно.
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — успешно.
5. `cargo test --workspace --all-targets` — 25 тестов успешно.
6. Два локальных Iroh endpoint обменялись signed event и signed
   acknowledgement — успешно.
7. После перезапуска обоих процессов application device IDs сохранились,
   transport Endpoint IDs сменились, author sequence продолжился — успешно.
8. После двух storage-aware соединений оба state directories содержат по 4
   одинаковых валидных events, один общий frontier и causal link между
   последовательными обменами — успешно.
9. Пустой recovery store с прежним device key восстановил историю у полного
   peer; частичный listener запросил отсутствующее событие у клиента — успешно.
10. Device, не совпадающий с allowed requester signed ticket, отклонён до
    соединения; allowed, но неизвестный истории device получил явный
    `RequesterNotKnown` без передачи events и зависания — успешно.
11. Transport-independent session test синхронизировал расхождение 70 events в
    каждом направлении за два bounded rounds (64 + 6) — успешно.
12. Реальный локальный Iroh smoke после разделения crates: отставший client
    восстановил 2 events за один round; `sync_rounds_completed=1`, итоговый
    store содержит 2 events и один frontier — успешно.
13. Path diagnostics на обоих концах локального Iroh exchange сообщили
    `transport_path=direct`, IP transport addresses, RTT и один открытый path —
    успешно.
14. При `LongPathsEnabled=1` Windows regression test воспроизвёл `os error 3`
    на event path длиннее 260 символов; canonical/verbatim root устранил ошибку.
    Полный release recovery smoke сохранил 2 events по пути длиной 298 символов,
    показал `event_count=2`, `frontier_count=1` и direct path — успешно.
15. Первый реальный M0.2 exchange между Windows-PC Alice `192.168.0.134` и Bob
    `192.168.0.135` доставил и подтвердил signed event напрямую по LAN с RTT
    около 1 ms. Recovery sync исправленным build получил 2 events в пустой
    store за один round по direct path с RTT 1.6 ms; `history` подтвердил
    `event_count=2`, `frontier_count=1` и точную causal acknowledgement — M0.2
    успешно завершён.
16. Подготовительный M0.3 process smoke проверил signed route policies.
    `direct-only` delivery
    и sync после restart listener выбрали direct IP path. Строгий `relay-only`
    endpoint публиковал ticket только с Relay address, delivery и sync после
    restart выбрали `euc1-1.relay.n0.iroh.link`; RTT составил примерно
    240–450 ms. На этом шаге проходили все 28 workspace tests.
17. Первый cross-network M0.3 direct-only прогон между домашней сетью и 4G
    обнаружил ошибку listener lifecycle. Iroh получил QUIC Initial, который не
    прошёл packet authentication (`authentication failed`); такой результат
    документирован Iroh как допустимый для UDP/retransmitted datagrams. CLI
    ошибочно завершал весь listener на первой неудачной `Incoming`. Listener
    теперь логирует и игнорирует initial/handshake failures, продолжая ждать
    следующую валидную попытку. Regression test посылает сначала handshake с
    неподдерживаемым ALPN, затем валидный; listener принимает второй connection.
    После добавления regression coverage проходят все 29 workspace tests.
18. Повторный direct-only прогон после отключения VPN успешно доставил signed
    Text/Acknowledgement: обе стороны сообщили `transport_ready_path=direct`,
    `transport_path=direct`, RTT 9.7–11.9 ms и два open paths. Однако выбранные
    remote addresses — `192.168.0.134` и `192.168.0.111` — принадлежат одной
    private `/24` LAN. Поэтому прогон подтверждает влияние VPN и рабочий direct
    transport, но не засчитывается как доказательство Internet hole punching;
    перед повтором надо исключить Ethernet/второй Wi-Fi/bridge/virtual route и
    сравнить внешние IP обоих хостов.
19. При корректном cellular-only прогоне Bob ticket содержал mobile public
    IPv4 mapping `91.79.200.55:16029`, private hotspot address
    `10.108.80.130` и global IPv6 candidates. Перед тестом Alice не имела
    `singbox_tun` и использовала единственный default route через домашний
    router `192.168.0.1`; VMware routes не были default. QUIC peer
    authentication через relay прошла, но direct path не появился за 15
    секунд. Это валидный отрицательный результат Internet hole punching,
    согласующийся с double NAT/CGNAT мобильного подключения Bob.
20. Последующий cross-network `relay-only` control дошёл у Bob до
    `relay_status=online` и `status=listening`, но Alice не получила `peer_id` и
    завершилась connection timeout через 30 секунд. На локальном хосте тот же
    build успешно повторил строгий public `euc1` relay-only delivery. Для
    следующего внешнего прогона client теперь явно ждёт online relay и печатает
    target Endpoint ID: это отличает stale/wrong ticket от недоступности relay
    на стороне Alice.
21. Диагностический build исключил stale ticket: Alice target Endpoint ID
    совпал с живым listener Bob, обе стороны были relay-online, но строгий
    relay-only снова завершился до `peer_id`. Контрольный `auto` с теми же
    хостами успешно доставил event и acknowledgement исключительно через
    relay: один open path, Bob/Alice RTT 457/686 ms. Ticket Bob рекламировал
    `euc1`, тогда как выбранный connection path на обеих сторонах стал `aps1`.
    Следовательно, public relay fallback и прикладной exchange исправны, а
    дефект локализован в строгом relay-only при разных relay selections. Новый
    dialer pin-ит свой relay map к relay URL подписанного listener ticket и
    печатает home/target relay URLs; внешний retest ещё требуется.
22. Внешний retest с pinning подтвердил `euc1` как home/target relay на обеих
    сторонах, но strict relay-only снова завершился connection timeout до
    `peer_id`; значит, расхождение home relay не было достаточным объяснением.
    CLI listener теперь принимает `--relay-url`: следующий контроль принудит
    оба endpoints использовать `aps1`, который уже успешно перенёс тот же
    exchange в режиме `auto`. Это разделит неисправность конкретного public
    relay route и общий дефект Iroh `clear_ip_transports` между сетями.
23. Локальный release smoke явно зафиксировал оба strict relay-only endpoints
    на `aps1`: listener ticket, client target/home URL и итоговый selected path
    совпали; delivery и acknowledgement прошли через один relay path с RTT
    около 484–485 ms. Механизм `--relay-url` готов к внешнему контрольному
    прогону.
24. Внешний test5 между домашней сетью Alice и cellular hotspot Bob успешно
    выполнил strict relay-only delivery через явно выбранный `aps1`. Bob home,
    Alice target/home и selected path совпали; был ровно один open relay path.
    Signed event `c64a2149...` и acknowledgement `b13047d3...` совпали на обеих
    сторонах, RTT составил 461.9/466.6 ms. Следовательно, Iroh
    `clear_ip_transports` работает между этими сетями, а прежний failure был
    специфичен для доступности `euc1` route во время тестов. Остался внешний
    relay-only sync после restart listener.
25. Финальный test6 перезапустил Bob с новым transport Endpoint ID и новым
    signed `aps1` relay-only ticket. Alice inventory содержал 8 events; за один
    bounded round она отправила ровно 6 отсутствующих, Bob получил те же 6 и
    ничего не отправил обратно. Обе стороны сообщили `status=synchronized`,
    `sync_more_available=false`, один relay path и RTT 472.0/472.8 ms. Bob
    начал с 2 events и закончил теми же 8, что Alice, поэтому M0.3 завершён без
    дополнительного history dump.
26. Локальный M0.4 wire smoke использовал два persistent device state и 70/70
    уникальных fixture events поверх 2 общих. Первый direct-only connection
    штатно остановился после 64/64 через `SyncPause` / `SyncPaused`; обе стороны
    вывели `status=paused` и `sync_resume_checkpoint=event-store`. После restart
    listener новый Endpoint ID и новый session binding передали ровно остаток
    6/6. Итоговые `history` полностью совпали: `event_count=142`,
    `frontier_count=2`. Форматирование, строгий Clippy и 32 workspace tests
    прошли.
27. Для внешнего M0.4 test7 в общей двусторонне синхронизируемой папке
    `C:\Users\Paul\YandexDisk\!M\test7` размещены release EXE commit
    `62a9d37` и пронумерованные PowerShell wrappers. Они динамически создают
    уникальные Run ID/conversation/ticket names, используют прошлые Alice/Bob
    device states, проверяют SHA-256 EXE, обязательные status/counters и
    автоматически сохраняют/сравнивают финальные histories. Phase 1 выполняет
    shared delivery и pause 64/64 по direct LAN; после ручного переключения Bob
    на cellular phase 2 ожидает resume 6/6 через pinned `aps1`. Все семь `.ps1`
    файлов успешно разобраны PowerShell parser; на момент подготовки внешний
    прогон ещё не был выполнен.
28. Внешний M0.4 test7 (`run_id=20260831-055424`, conversation
    `m04-test7-20260831-055424`) успешно выполнил весь self-checking сценарий.
    Phase 1 передал 64/64 и остановился после подтверждённого round. После
    переключения Bob с LAN на cellular Phase 2 через relay передал только 6/6.
    Финальная проверка получила `event_count=142`, `frontier_count=2` и полное
    совпадение детерминированного `history` Alice/Bob, включая оба frontier IDs.
    Сохранённый `SUCCESS.txt` содержит `histories_equal=true` и
    `resume_path=relay`. M0.4 завершён.
29. M0.5.1 добавил отдельный Account Root, root-signed device certificate и
    permanent revocation. Локальный CLI smoke создал account/device, успешно
    проверил capabilities `sign-events,sync-history`, затем применил revocation
    sequence 1 и получил ожидаемый ненулевой exit code с `DeviceRevoked`.
    Форматирование, строгий Clippy, release build и все 38 workspace tests
    прошли.
30. M0.5.2 заменил `--allow-device` / known-author на ticket v3 и отдельный
    Endpoint-bound authorization stream. Локальный process smoke между двумя
    Account IDs успешно выполнил delivery; второе новое устройство Alice с
    пустой историей получило 2 events Bob. После передачи Bob root-signed
    revocation этого device обе стороны завершили новый session ненулевым exit
    code до inventory (`authorization=rejected`). Форматирование, строгий
    Clippy, release build и все 42 workspace tests прошли.
31. M0.6.1 добавил durable root revocation set, root-signed complete authority
    snapshot и persistent max-seen anti-rollback/equivocation protection.
    Ticket v4 несёт listener snapshot, session proof — requester snapshot;
    сетевые `--peer-revocation-file` удалены. Lifecycle дополнен
    `account-snapshot` и `device-authority-update`. Старый root с уже выданными
    sequences без durable log отклоняется вместо небезопасной автоматической
    миграции. Локальный свежий Alice/Bob process smoke подтвердил ticket v4,
    двусторонний pin revision 1, authorization и direct delivery. Форматирование,
    строгий Clippy, release build и все 46 workspace tests проходят.
32. M0.6.2 добавил owner-signed add-only conversation membership,
    `AuthorizedEvent` и обязательные immutable authorization sidecars. Sync
    wire/signature domains повышены до v2, Iroh ALPN — до
    `kilogram/m0/sync/2`. Unit tests покрывают membership persistence,
    rollback/equivocation/removal refusal, non-member events и missing sidecar.
    После обнаружения stale debug EXE финальная проверка использовала явно
    пересобранный release binary. Локальный direct Alice/Bob smoke создал
    membership revision 1, установил его на оба отдельных Account ID, доставил
    Text/Acknowledgement, добавил Alice 3 events и одним sync round передал Bob
    ровно эти 3. Обе авторизованные истории содержали одинаковые 5 events. Все
    50 workspace tests проходят.
33. M0.7.1 добавил `kilogram-crypto` с HPKE 0.14 Base mode
    X25519/HKDF-SHA256/ChaCha20-Poly1305, отдельный persistent encryption key
    устройства и его root-signed binding в DeviceCertificate v2. Plaintext
    Text event удалён; delivery/history используют два recipient boxes, а
    store/sync сохраняют ciphertext. Event/sync/session/ticket/ALPN версии
    повышены несовместимо. Форматирование, строгий Clippy, release workspace
    build и 54 tests прошли. Свежий release process smoke
    `.tmp/m071-smoke-20260831-184557` между отдельными Alice/Bob Account Roots
    выполнил direct encrypted delivery, передал 3 encrypted seed events через
    reconnect sync, получил одинаковые histories из 5 events и не нашёл
    plaintext marker в сырых `.event` обоих устройств.
34. M0.7.2 разделил replicated ciphertext и local readable history. Event v3
    содержит только один peer HPKE box; sender/recipient создают immutable
    `local-messages/*.local-text`, зашифрованный на local device key и связанный
    с Event ID. Delivery и sync сохраняют projection до event, history требует
    projection, outsider event fail-closed отклоняется. Sync/session/ticket/ALPN
    повышены до v4/v4/v6/`kilogram/m0/sync/4`. Форматирование, строгий Clippy,
    release workspace build и все 56 tests проходят. Свежий release process
    smoke `.tmp/m072-smoke-20260831-194220` выполнил direct delivery, sync трёх
    seed events, получил одинаковые histories из 5 events и по 4 local
    projections на endpoint; plaintext markers отсутствуют в `.event` и
    `.local-text`.
35. M0.7.3 добавил `kilogram-ratchet` на `vodozemac` 0.10.0. Olm account,
    one-time prekey и pairwise sessions сохраняются encrypted pickle под
    `STATE_DIR/ratchet`; public ratchet identity/prekey подписаны application
    Device key. Event v4 хранит PreKey/Normal ciphertext, ticket v7 переносит
    listener bundle, sync/session/ALPN повышены до v5/v5/`kilogram/m0/sync/5`.
    `seed-history` теперь требует также `--peer-prekey-bundle-file`, а команда
    `ratchet-bundle` экспортирует bundle без запуска listener. Unit/integration
    tests покрывают persistent PreKey → Normal → Normal cycle и sync нескольких
    prekey-events. Форматирование, строгий Clippy, release workspace build и все
    60 tests проходят. Release smoke `.tmp/m073-smoke-20260831-232212` выполнил
    direct PreKey → Normal → Normal через три listener restart, подтвердил один
    session ID, одинаковые histories из 6 events и отсутствие plaintext в 22
    проверенных ciphertext state files.
36. M0.7.4 добавил root-signed `AccountDeviceListSnapshot`, exact
    `AccountPrekeyDirectory` и ticket v8. Event v5 содержит embedded device list
    и отдельный canonical Olm ciphertext slot каждого peer device; sync/session/
    ALPN повышены до v6/v6/`kilogram/m0/sync/6`. `account-device-list` публикует
    список, `listen` собирает bundle directory, а offline device создаёт local
    projection после sync своего slot. Форматирование, строгий Clippy, release
    workspace build и все 63 tests проходят. Release smoke
    `.tmp/m074-smoke-20260901-004422` подтвердил один Alice event для Bob-1 и
    Bob-2, последующий sync Bob-2, одинаковые histories, session counts 2/1/1
    и отсутствие plaintext в 16 ciphertext state files.
37. M0.7.5 добавил `HistoryRewrapBundle` v1 и local projection v2. Source
    подписывает canonical text-inventory digest/range и каждую HPKE entry с
    исходным `AuthorizedEvent`; import проверяет same-account device list,
    membership, target key и durable provenance. Direct projections v1 остаются
    совместимыми. `history-rewrap-export/import` поддерживают диапазоны до 256,
    partial/full marker и идемпотентное перекрытие. Форматирование, строгий
    Clippy, release workspace build и все 64 tests проходят. Smoke
    `.tmp/m075-smoke-20260901-020138` восстановил новому Bob device три старых
    events, отклонил wrong recipient, повторно получил удалённый event через
    sync и не нашёл plaintext в 16 ciphertext files.
38. M0.7.6 добавил `SignedPrekeyPool` v1, `AccountPrekeyDirectory` v2, ticket
    v9 и persistent ratchet session record v2. Пул по умолчанию содержит 16
    OTK, signed generation/sequence/expiry; observer сохраняет per-device
    max-seen pool. Crossed outbound sessions сходятся на lexicographic-min
    active session и сохраняют одну retained branch. CLI использует
    `ratchet-prekey-pool` и `--peer-prekey-pool-file`. Форматирование, строгий
    Clippy, release build и все 69 tests проходят. Smoke
    `.tmp/m076-smoke-20260901-032421` подтвердил crossed sync 1/1, rotation
    generation `0 -> 1`, sequence `0..15 -> 16..31`, stale rejection,
    post-convergence delivery и отсутствие plaintext в 24 ciphertext files.
39. M0.7.7 добавил crate `kilogram-state`: exclusive OS lock canonical
    `STATE_DIR`, journal v1 с mutable backup `ratchet`/`next-sequence`, baseline
    append-only roots и markers prepared/committed/rolled-back. CLI transaction
    охватывает delivery, sync materialization, seed, rewrap import и prekey
    update. Unit fault tests проверяют operation rollback, next-start recovery,
    interrupted committed cleanup и повторное получение lock. Форматирование,
    строгий Clippy, release workspace build и все 75 tests проходят. Release
    process smoke `.tmp/m077-smoke-20260901-035517` проверил refusal второго CLI
    (`exit=1`), reuse после освобождения (`exit=0`) и отсутствие active journal.
40. M0.7.8 добавил `history-rewrap-sas`, `history-rewrap-fetch` и
    `history-rewrap-reconcile`. Recipient request подписан и session-bound,
    source transfer подписывает точный request+bundle, source/listener требует
    exact consent для device/conversation/range/SAS. ALPN —
    `kilogram/m0/sync/7`; event v5, sync/session v6 и ticket v9 не менялись.
    `.rewrap`, `.transfer`, events и projections сохраняются одной M0.7.7
    transaction. Async command dispatcher box-pinned: без indirection новый
    крупный command future переполнял 1 MiB main-thread stack Windows до разбора
    CLI. Process smoke `.tmp/m078-smoke-20260901-044605` передал 3/3 старых
    events по direct Iroh, сохранил transfer 4,674 bytes, получил complete
    `single-source`, `global_completeness_proven=false` и clean plaintext scan.
41. M0.7.9 добавил `SignedHistoryRecoveryCheckpoint` v1 и команду
    `history-recovery-resume`. Source consent теперь является полным окном, а
    каждый session-bound request остаётся страницей до 256 events. Recipient
    требует exact `--source-device`, восстанавливает hash-linked append-only
    checkpoint chain, фиксирует первый source inventory claim и отклоняет его
    смену. Bundle, transfer, events, projections и новый checkpoint коммитятся
    одной transaction; `history-recovery` добавлен в append-only journal roots.
    Reconciliation публикует `selected_inventory_*` только при согласии минимум
    двух полных claims. Wire objects/ALPN `kilogram/m0/sync/7` и ticket v9 не
    менялись. Все 78 tests проходят. Process smoke
    `.tmp/m079-smoke-20260901-051549` через два fresh direct Iroh sessions
    восстановил страницы `0..2` и `2..3`, создал два checkpoint, подтвердил
    network-free completed retry и полное совпадение source/recipient history.
42. M0.8.1 добавил `redb` 4.2 в `kilogram-state` и encrypted shadow vault v1.
    `state-vault-migrate` одной immediate-durability transaction сохраняет
    XChaCha20-Poly1305 records и keyed BLAKE3 manifest; `verify` сверяет vault с
    retained legacy tree, а `restore` публикует проверенный staging только в
    новый каталог. Unit tests покрывают abort, drift, wrong key, plaintext scan
    и exact restore. Все 81 workspace tests, strict Clippy и release build
    проходят. Release smoke `.tmp/m081-smoke-20260901-070000` мигрировал 23
    файла/26,037 bytes из реального M0.7.9 recipient state, получил idempotent
    `already-current`, восстановил byte-identical tree и те же 3 history events;
    raw DB scan не нашёл plaintext fixtures/path markers. Случайный key пока
    лежит рядом development-файлом; primary CLI repositories ещё используют
    filesystem state.
43. M0.8.2 добавил `StateMirrorRepository`, authenticated mirror intent и
    monotonic generation. Каждая live device-state CLI-команда после инициализации
    vault готовит immediate-durability intent, а после своего фактического
    результата зеркалирует committed legacy tree и очищает intent одной DB
    transaction. Next-start сначала выполняет M0.7.7 filesystem recovery, затем
    завершает mirror только при валидном intent; drift без intent fail-closed.
    Unit/CLI tests покрывают crash, abort, retry, forged intent и tamper. Все 83
    workspace tests, strict Clippy и release build проходят. Release smoke
    `.tmp/m082-smoke-20260901-080000` сохранил generation 1 на read-only
    `identity`, повысил её до 2 после live prekey rotation, восстановил exact 23
    files/3 history events и не нашёл plaintext markers в raw DB.
44. M0.8.3 заменил full live rewrite на atomic record delta и добавил typed
    shadow inventory. `VaultMirrorCommit` публикует upsert/remove/unchanged;
    девять `StateRecordKind` проверяются exact DB/legacy path+content через
    `state-vault-shadow-read`. Unit test доказывает, что unchanged ciphertext
    остаётся byte-identical, changed ciphertext заменяется, deletion удаляется,
    а abort не публикует delta. Все 84 workspace tests, strict Clippy и release
    build проходят. Release smoke `.tmp/m083-smoke-20260901-100000` получил
    `0/0/23` для read-only и `2/0/21` для prekey rotation, generation 1→2,
    exact typed inventory, byte-exact restore 23 files/3 history events и clean
    raw DB marker scan. Полный scan/decrypt остаётся `O(state)`, но encryption и
    DB writes стали `O(changed + deleted)`.
45. M0.8.4 добавил первый DB-primary read canary. `history` при наличии vault
    получает `.event`, `.authorization` и `.local-text` bytes из owned
    encrypted snapshot после exact full shadow compare. Новые object-safe
    `EventReadRepository`/`LocalMessageReadRepository` реализованы filesystem
    stores и strict snapshot adapters; snapshots повторяют signature/ID/
    membership/projection validation и не обращаются к legacy files. Все 85
    tests, strict Clippy и release build проходят. Process smoke
    `.tmp/m084-smoke-20260901-120000` прочитал generation 2, 6 event records,
    3 projections и 3 history messages с mirror delta `0/0/23`; отдельный
    projection drift дал exit 1 typed mismatch и `silent_fallback=false`.
46. M0.8.5 распространил immutable vault-primary read-set на manual
    `history-rewrap-export` и source-side network rewrap. Listener захватывает
    snapshot только при explicit approval и до authority/prekey/request work;
    bundle builder использует `EventReadRepository`/
    `LocalMessageReadRepository`, а event trait получил authorized inventory и
    events-by-ID для будущего sync overlay. Все 85 tests, strict Clippy и
    release build проходят. Release smoke
    `.tmp/m085-smoke-20260901-153149` создал manual и network bundle по 3
    events из vault generation 1 (6 event records, 3 projections), сохранил
    exact post-transfer generation 1; projection drift дал exit 1 без fallback.
47. M0.8.6 добавил command-local committed overlay над immutable vault sync
    base. Event/projection records staged до M0.7.7 transaction и публикуются
    только после её commit; последующие rounds используют merged inventory.
    Store/CLI tests проверяют невидимость staged data, два последовательных
    batches, idempotency и rollback failure. Все 85 tests, strict Clippy и
    release build проходят. Process smoke
    `.tmp/m086-smoke-20260901-160635` передал 73 events rounds `64 + 9`, получил
    listener overlay `73/73`, одинаковые vault-primary histories, peer vault
    generation 2 и fail-closed exit 1 на projection drift.
48. M0.8.7 сделал immediate redb checkpoint точкой commit для всех
    `StateTransaction` paths. DB transaction атомарно меняет records/manifest/
    generation и публикует keyed primary-shadow intent; filesystem journal
    затем подтверждает exact retained shadow. Crash test откатывает prepared
    staging и восстанавливает event+ratchet из vault; forged marker отклоняется.
    Все 86 tests, strict Clippy и release build проходят. Process smoke
    `.tmp/m087-smoke-20260901-170711` получил source generation 4, listener
    generation 3, commit до network status, final `already-current` mirrors и
    совпадающие DB-primary delivery/ack events.
49. M0.8.8 заменил live full-tree checkpoint на direct typed journal delta.
    Transaction сравнивает bounded ratchet/sequence с backup и читает payload
    только новых append-only records; vault проверяет canonical kind/path и
    публикует delta с прежним primary-shadow marker. Bounded trust compatibility
    ingress включает authority/certificate/membership/peer-authority records,
    чьи writers пока находятся вне journal, и fail-closed отклоняет их удаление.
    `next-sequence` теперь лениво читается из authenticated DB generation,
    filesystem служит staged shadow. State/CLI tests покрывают exact write-set,
    append-only removal,
    abort, чужой root и намеренно изменённый shadow counter. Все 90 workspace
    tests, strict Clippy и release build проходят. Release process smoke
    `.tmp/m088-smoke-20260901-185655` завершил delivery/ack, получил source/
    listener generations `4/3`, DB-primary sequence, typed journal delta и
    exact совпадение final vault/legacy histories.
50. M0.8.9 добавил DB-primary ratchet staging: authenticated vault records
    заменяют retained workspace до `RatchetState`, а отдельный primary backup
    гарантирует DB-authoritative ratchet/sequence rollback. Все live append-only
    writers регистрируют canonical paths; второй directory walk удалён,
    modification/removal immutable records запрещены. Все 92 workspace tests,
    strict Clippy и release build проходят. Release process smoke
    `.tmp/m089-smoke-20260901-193740` завершил delivery/ack с ratchet+sequence
    DB-primary, append write-set `3/5`, generations `4/3`, совпавшей history и
    final `already-current` compatibility mirror.
51. M0.8.10 добавил repository-owned `AppendOnlyWriteReceipt` и vault schema v2
    с AEAD-encrypted manifest index. Direct typed commit применяет journal delta
    к path/length/hash metadata и сообщает `vault_payload_records_loaded=0`, не
    перечисляя unchanged DB payload. Schema-v1 test проверяет one-time rebuild,
    а index tamper завершается fail-closed. Все 93 workspace tests, strict
    Clippy и release build проходят. Release process smoke
    `.tmp/m0810-smoke-20260901-201413` выполнил два delivery+ack, свёл Alice/Bob
    к одинаковым histories из 4 events и валидным schema-v2 vault с 21 record;
    receipts составили sender `3/2`, listener `5`, normal commit использовал
    incremental index с `payload_records_loaded=0`.
52. M0.8.11 добавил `TrustStateRepository`, который через authenticated
    schema-v2 index decrypt-ит только certificate/authority/membership payloads.
    Все production trust writes используют DB-hydrated crash-journal workspace
    и typed `Trust` delta; bounded filesystem ingress удалён из direct commit.
    State/CLI regressions проверяют tampered shadow, rollback, interrupted
    recovery и unregistered ingress. Все 95 workspace tests, rustfmt, strict
    Clippy и release build проходят. Release process smoke
    `.tmp/m0811-smoke-20260901-204423` выполнил DB-primary trust reads, по одному
    explicit peer-authority upsert, direct delivery/ack, одинаковые histories
    из 2 events и валидные schema-v2 vault generations `5/4` с 16 records.
53. M0.8.12 заменил raw `state-vault.key` versioned protected envelope. Windows
    provider использует DPAPI CurrentUser; 32-byte legacy key автоматически
    rewrap-ится до открытия redb, а corrupt blob завершается fail-closed. Все
    96 workspace tests, rustfmt, strict Clippy и release build проходят.
    Windows release smoke `.tmp/m0812-key-smoke-20260901-220000` на копии
    настоящего M0.8.11 Alice vault сохранил generation 5, 16 records и snapshot
    ID, изменил key file `32 -> 282` bytes с magic `KILOGRAM-VAULTK1`; второй
    verify сообщил `vault_key_load=already-current`.
54. M0.8.13 добавил `state-vault-key-export/import`: внешний recovery package
    использует Argon2id v0x13 (`64 MiB`, `t=3`, `p=1`) и
    XChaCha20-Poly1305, содержит authenticated generation/snapshot witness и
    никогда не перезаписывает существующий output. Import проверяет candidate
    key на всей DB, rollback и same-generation fork до atomic local-provider
    install. State regressions покрывают wrong passphrase/tamper/wrong vault,
    no-clobber, отсутствие key mutation, rollback и fork. Все 98 workspace
    tests, rustfmt, strict Clippy и release build проходят. Windows smoke
    `.tmp/m0813-recovery-smoke-20260901-230000` на реальном vault generation 5
    удалил local key и восстановил 282-byte DPAPI envelope из 148-byte package,
    сохранив 16 records и snapshot
    `a81942b5e5935c02514617aa605d79bd74dcb2b6ccf2b1a03570aae9d7ee2da8`.
55. M0.8.14 добавил immutable `DeviceIdentityStateRepository` и единый CLI
    loader. Для initialized vault все 17 production call sites decrypt-ят только
    две identity records из authenticated schema-v2 index; missing/invalid/
    tampered record не имеет filesystem fallback. Secret buffers zeroize-ятся,
    а identity не включена в direct mutation set. Все 101 workspace test,
    rustfmt, strict Clippy и release build проходят. Windows release process
    smoke `.tmp/m0814-identity-smoke-20260901-234500` выполнил `identity` на
    копии реального vault, сообщил `vault_device_identity_read_source=db-primary`
    и generation 5, сохранил device ID
    `e9c761facda7ea876ee310e04192a88fa2b2c77a261b52a29e515289a94a3740`,
    encryption public key
    `674cd65a76d98d25b499b2630ce8aeef271912a629fce753ab2e31518ec8fc26`,
    16 records и прежний snapshot ID без DB delta.
56. M0.8.15 вводит schema-v3 DB-only identity layout. Schema-v1/v2 vault
    полностью аутентифицируется, non-identity shadow сравнивается exact, затем
    новая schema/generation коммитится до удаления совпавших raw signing и
    encryption keys. Effective snapshot сохраняет эти records только из DB,
    final gate безопасно завершает matching interrupted cleanup, mismatched
    copy fail-closed, а primary-shadow recovery keys не воссоздаёт. State/CLI
    regressions покрывают upgrade с уже отсутствующим raw file, resumable
    cleanup, reappearance и recovery. Все 102 workspace tests, rustfmt, strict
    Clippy и release build проходят. Windows release smoke
    `.tmp/m0815-identity-retirement-smoke-20260902-002043` сохранил 16 records,
    5654 bytes, snapshot/device/encryption IDs при переходе schema `2 -> 3`,
    generation `5 -> 6`; оба raw key files отсутствуют, повторный migrate
    idempotent, typed identity shadow содержит 0 records.
57. M0.9.1 добавил bounded multi-page recovery coordinator. Source обслуживает
    до 64 contiguous pages одного consent window по одному authenticated Iroh
    connection и immutable DB-primary snapshot; recipient коммитит каждый
    transfer/checkpoint отдельной state transaction. `--max-pages` даёт
    resumable pause, wire/ticket/ALPN не менялись. Debug CLI coordinator вынесен
    в отдельный 8 MiB stack thread после воспроизводимого pre-dispatch stack
    overflow. Все 103 workspace tests, rustfmt, strict Clippy и release build
    проходят. Direct smoke `.tmp/m091-smoke-20260902-004647` одним ticket
    перенёс две страницы, создал два checkpoint и подтвердил identical history.
58. M0.9.2 добавил CLI-local `recovery_link` contract: source-signed Postcard
    payload кодируется как versioned base64url URI с hard limit 2953 bytes,
    exact recipient/conversation/range и expiry до часа, без prekey pools.
    `history-recovery-link-inspect` работает offline, а `...-accept` выполняет
    local recipient/conversation/SAS preflight и затем использует общий M0.9.1
    bootstrap/coordinator. Все 104 workspace tests, rustfmt, strict Clippy и
    release build проходят. Direct smoke `.tmp/m092-smoke-20260902-010717`
    получил 1202-byte link, отклонил wrong device до сети и перенёс две pages
    одним authenticated connection с identical history.
59. M0.9.3 добавил `recovery_qr`: `qrcode 0.14.1` рендерит no-clobber PNG с EC
    L/quiet zone, `rqrr 0.10.1` декодирует ровно один QR, `image 0.25.10`
    собран только с PNG/JPEG. Input ограничен 16 MiB, 4096×4096, 64 MiB image
    allocation budget и 2953-byte ASCII URI. Listener пишет QR напрямую,
    standalone render повторяет его, inspect/accept принимают `--qr-file`.
    Все 109 workspace tests, rustfmt, strict Clippy и release build проходят.
    Direct smoke `.tmp/m093-smoke-20260902-012850` проверил Version 25 PNG,
    offline round-trip/no-clobber/wrong-device preflight и две recovery pages.
60. M0.9.4 включил Tokio `net` и добавил `recovery_discovery`: opt-in publisher
    отправляет signed URI на IPv4 multicast `239.255.75.71:45371` с TTL 1 и
    loopback каждые 750 ms. Scan длится не более 30 s, читает до 512 датаграмм,
    собирает до 16 unique candidates и выполняет exact recipient/conversation/
    membership/expiry/source/authority проверки без Iroh connection. Все 110
    workspace tests, rustfmt, strict Clippy и release build проходят. Direct
    smoke `.tmp/m094-smoke-20260902-015219` проверил wrong-device rejection,
    verified unique discovery без connection и отдельный SAS-gated accept с
    двумя recovery pages и identical history.
61. M0.9.5 добавил `recovery_plan`: recipient-signed plan v1 до 64 KiB живёт
    `1..=168` часов и связывает exact source/device-list/SAS/conversation/range/
    page/route с Ethernet/Wi-Fi/mobile/unknown и external-power policy. Runner
    выполняет до 8 LAN discovery attempts с delay до 300 s, принимает новый
    endpoint только при exact-plan match и продолжает signed checkpoints.
    Outer state lock/vault intent отключён для runner; короткие locks окружают
    только preflight, active transfer и checkpoint read. Все 112 workspace
    tests, rustfmt, strict Clippy и release build проходят. Direct smoke
    `.tmp/m095-smoke-20260902-022156` подтвердил mobile block до discovery,
    no-candidate → source restart → fresh endpoint, foreground state access во
    время backoff и recovery двух pages с identical history.
62. M0.9.6 добавил `recovery_scheduler`: recipient-signed append-only state v1
    под `history-recovery/scheduler/<plan-id>` хранит generation/previous ID,
    lifecycle, persistent attempts/failures, wall-clock high-water и deadline.
    Перед discovery записывается bounded lease до двух часов; failure использует
    exponential equal-jitter с base `0..=300` и max `0..=3600` seconds. Immediate
    restart до deadline не открывает UDP; истёкший lease становится signed
    failure, а clock rollback блокирует запуск. `history-recovery-plan-cancel`
    добавляет необратимый terminal record. Все 114 workspace tests, rustfmt,
    strict Clippy и release build проходят. Direct smoke
    `.tmp/m096-smoke-20260902-130246` подтвердил process restart 1 → deferred →
    persistent attempt 2, две pages/identical history и отдельный terminal
    cancellation restart без discovery.
63. M0.9.7 добавил `recovery_platform`: platform-neutral provider boundary и
    Windows implementation на WinRT `Networking.Connectivity`/`System.Power`.
    Runner без context flags использует `windows-native`; manual
    `--network-class`/`--power-source` остаются только полной парой. Exact
    WLAN/WWAN/IANA media, connection cost и roaming определяют policy class;
    fixed/variable/metered/roaming используют `mobile`, tunnel/VPN может быть
    разрешён только через единственный active physical profile, ambiguity
    становится `unknown`. Power `external` требует adequate supply. CLI не
    печатает profile name, SSID, GUID или IP. Добавлена target dependency
    `windows 0.62.2` с `Foundation_Collections`, `Networking_Connectivity` и
    `System_Power`. Все 118 workspace tests, rustfmt, strict Clippy и release
    build проходят. Regression `.tmp/m096-smoke-20260902-140653` прошёл полностью;
    `.tmp/m097-smoke-20260902-142158` подтвердил native Ethernet/unmetered/
    non-roaming/external snapshot под активным VPN, automatic runner policy и
    отказ partial manual override.
64. M0.9.8 добавил `history-recovery-plan-watch`: bounded process ждёт signed
    scheduler deadline, WinRT NetworkStatusChanged и PowerManager supply/
    battery/EnergySaver events, а terminal state опрашивает без удержания lock.
    Runtime 1..86400 s, meaningful wakeups 1..1024, cancel poll 1..30 s; typed
    lock contention повторяется до 2 s. Native policy перечитывается перед
    lease/discovery и перед connect. Все 121 workspace tests, rustfmt, strict
    Clippy и release build проходят. Regression
    `.tmp/m096-smoke-20260902-171150` прошёл полностью; direct worker smoke
    `.tmp/m098-smoke-20260902-173045` подтвердил native event registration,
    свободный state lock во время wait и signed cross-process cancel за 1.733 s
    без connection после cancel; отдельный long discovery был прерван runtime
    bound, lease записан failure, bounded cleanup завершилась за 4.022 s.
65. M0.9.9 добавил `runtime`: stable Iroh endpoint/ticket обслуживает successive
    delivery/sync sessions, каждый connection повторяет authorization, а failed
    session не завершает процесс. Runtime исключён из outer state lock/vault
    guard; route wait проходит без lock, accepted application session получает
    typed retry до 15 s и отдельный dual-write. Ticket публикуется atomic replace;
    Tokio workspace получил feature `signal` для clean Ctrl+C. Unit test покрывает
    create/replace ticket. Process smoke
    `.tmp/m099-smoke-20260902-182050` выполнил два connects + sync через один
    Endpoint ID (3 sessions, 4 events, identical histories), затем restart
    сменил Endpoint ID/ticket и доставил третье сообщение (6 events, histories
    equal). Final regression выполнил ещё delivery + sync через двухсессионный
    runtime, histories сошлись на 8 events; idle-bound control завершился через
    одну секунду с 0 sessions. Все 122 workspace tests, rustfmt, strict Clippy и
    release build проходят.

Публичный relay проверен между двумя сетями в принудительном `relay-only` через
`aps1`. Внешний M0.3 direct-only тест корректно доказал невозможность hole
punching в выбранной home-to-cellular topology; `auto` и strict relay-only
подтвердили рабочий fallback. Restart/sync через `aps1` сошёлся за один round;
M0.3 и M0.4 завершены. Смена физической сети LAN → cellular между pause и
resume подтверждает, что durable event set продолжает bounded sync с новым
transport Endpoint/session binding без повторной передачи подтверждённого
batch. M0.5.1 завершает локальную authority-модель; сетевое применение
certificate/revocation завершено в M0.5.2. Completeness на подписанной revision
и anti-rollback реализованы в M0.6.1; M0.6.2 закрывает минимальный add-only
conversation membership и author verification. M0.7.3 завершил первый
persistent pairwise Double Ratchet spike; M0.7.4 расширил его до проверяемого
account-wide fan-out. M0.7.6 добавил bounded signed pools, local freshness
high-water и concurrent initiation resolution; first-contact global freshness
и production network discovery остаются открыты. M0.7.5 реализовал
same-account history rewrap; M0.7.8 добавил сетевой consent/SAS и local
multi-source claim reconciliation, M0.7.9 — signed checkpoint pagination и
safe retry, M0.9.1 — до 64 atomic pages в одном connection, M0.9.2 — compact
signed recovery device link с offline inspect и explicit accept, M0.9.3 —
bounded PNG/JPEG QR file ceremony, M0.9.4 — opt-in authenticated LAN
publication/discovery без automatic trust, а M0.9.5 — recipient-signed retry
plan и bounded coordinator с caller-supplied network/power context. M0.9.6
добавляет persistent signed retry state, lease, jitter, clock high-water и
terminal cancellation, M0.9.7 — Windows-native platform context с conservative
VPN/metered/roaming/power mapping, M0.9.8 — bounded worker с native change
events и повторным policy gate. Wide-area descriptor discovery, регистрация OS
background service/task, non-Windows adapters, live camera/clipboard, membership
removal/epochs и group E2EE остаются открыты. M0.7.7 закрывает M0 crash consistency для device
filesystem state; M0.8.1 доказывает атомарную encrypted shadow migration в
`redb`, M0.8.2 — recoverable versioned dual-write всех live CLI commands,
M0.8.3 — typed incremental encrypted delta и exact shadow reads, M0.8.4 —
первый DB-primary canary для read-only history, M0.8.5 — тот же cutover для
manual/network history-rewrap source reads, M0.8.6 — command-local overlay для
mixed sync reads, а M0.8.7 — vault-primary commit barrier для journaled writes.
M0.8.8 добавляет typed direct journal delta и mutable DB-primary sequence.
M0.8.9 добавляет DB-primary ratchet workspace и durable primary rollback backup;
M0.8.10 — repository-owned receipts и encrypted incremental manifest index.
M0.8.11 добавляет DB-primary trust repository и удаляет compatibility ingress.
M0.8.12 добавляет Windows DPAPI CurrentUser key envelope и legacy-key rewrap;
M0.8.13 — portable passphrase recovery package и внешний snapshot witness;
M0.8.14 — immutable DB-primary device identity без filesystem fallback;
M0.8.15 — schema-v3 DB-only device identity и physical namespace retirement;
M0.9.1 — bounded multi-page history recovery coordinator; M0.9.2 — signed
QR-ready history recovery device link; M0.9.3 — bounded QR image round-trip;
M0.9.4 — opt-in bounded LAN discovery signed descriptors без auto-connect;
M0.9.5 — consent-bound retry plan/coordinator с fresh endpoint matching;
M0.9.6 — signed append-only scheduler state с restart resume и cancellation.
M0.9.7 — Windows-native recovery platform context и provider boundary;
M0.9.8 — bounded Windows recovery worker, native change events и повторный
policy gate перед discovery/connect; M0.9.9 — long-lived multi-session
messaging runtime со stable endpoint и per-session state transaction;
M0.9.10 — signed persistent contact, locally encrypted durable outbox,
materialize-once delivery, persistent retry и runtime-owned automatic sync.
Paged/Merkle index, production non-Windows local provider, согласованный
monotonic rollback witness, автоматический backup lifecycle и физическое
retirement остальных compatibility shadows ещё не реализованы.

## Решения, которые ещё нельзя фиксировать

- Версию OpenMLS следует выбрать по стабильному crates.io-релизу, а не по `main`.
- Protobuf пока не выбран, поэтому отсутствие системного `protoc` не является
  блокером; при выборе Protobuf желательно использовать воспроизводимый
  vendored protoc.
- CMake/Ninja устанавливать заранее не требуется.

## M0.9.10 verification snapshot (2026-09-03)

- Rust workspace: `cargo fmt --all`, strict
  `cargo clippy --workspace --all-targets -- -D warnings` проходят.
- Все 124 workspace tests проходят; CLI test count вырос до 44.
- Новый process test поднимает Alice/Bob direct-only runtimes на одном host,
  проверяет durable queued delivery, ACK, automatic sync, identical two-event
  histories и delivered outbox marker.
- Первый запуск теста обнаружил cancellation race: polling пересоздавал accept
  future и Iroh закрывал handshake. Accept/ctrl-c futures теперь сохраняются
  между ticks; повторный process test и полный regression проходят.
- Release artifact нужно продолжать собирать обычным `cargo build --release`;
  новых native/system dependencies M0.9.10 не добавляет.

## M0.9.11 verification snapshot (2026-09-03)

- Workspace добавил pure-Rust `kilogram-runtime-ipc`; новых системных/native
  dependencies нет.
- `cargo fmt --all` и strict `cargo clippy --workspace --all-targets -- -D
  warnings` проходят.
- Все 129 workspace tests проходят; shared IPC crate имеет 5 unit tests, а CLI
  process test подтверждает idempotent actor queue → P2P delivery → ACK → sync.
- `cargo build --workspace --release` проходит на Windows host.

## M0.9.12 verification snapshot (2026-09-03)

- Workspace добавил safe-Rust `kilogram-windows` desktop package поверх
  `kilogram-runtime-ipc`; GUI не зависит от state/store/session/transport.
- `eframe`/`egui` закреплён на 0.33.3 с declared MSRV Rust 1.88, поэтому
  workspace сохраняет собственный `rust-version = 1.91`.
- `cargo fmt --all -- --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- Все 135 workspace tests проходят; 6 новых tests включают signed loopback IPC
  round-trip `Ping` → `QueueMessage` → `OutboxStatus` и uncertain-send retry с
  тем же request ID.
- `cargo build --workspace --release` проходит. Windows artifact
  `target/release/kilogram-windows.exe`: 6,175,232 bytes, SHA-256
  `D77D55107BBA300422CE5D4372298FF9FB81C1F520FA18FEF98903560F7F9B92`.

## M0.9.13 verification snapshot (2026-09-03)

- Runtime IPC v2 дополнен typed `ConversationList` и snapshot-bound
  `HistoryPage`; новых external/system dependencies нет.
- GUI по-прежнему зависит только от `kilogram-runtime-ipc`, public identity
  types и `eframe`, но теперь показывает signed-contact chat list и локально
  расшифрованную paginated history.
- Targeted tests проверяют cursor parse/staleness, causal ordering, desktop
  selection/adapter и настоящий runtime process до и после P2P delivery.
- Повторный process run обнаружил startup race: empty last-sync map делал
  periodic sync немедленным и мог исчерпать test action budget до GUI queue.
  Первый sync теперь due только после полного interval; delivery остаётся
  приоритетной.
- `cargo fmt --all -- --check`, strict all-target/all-feature Clippy, все 139
  workspace tests и `cargo build --workspace --release` проходят.
- Windows artifact `target/release/kilogram-windows.exe`: 6,245,888 bytes,
  SHA-256
  `1F732775050C37AFAA93ADF0708F835BDDCCB0DD744A5905FE53C6C4C598FDC1`.

## M0.9.14 verification snapshot (2026-09-03)

- Runtime IPC v3 добавил actor-owned `AddContact` и graceful `Shutdown`;
  desktop package по-прежнему не зависит от state/store/session/transport.
- `RuntimeLaunchProfile` v1 bounded до 64 KiB, no-clobber, требует абсолютные
  authority/output paths и расположение profile/IPC descriptor вне protected
  state; seed/device/vault/bearer secrets в profile отсутствуют.
- CLI `runtime-profile-create` и `runtime-from-profile` проверены release help;
  GUI запускает соседний CLI child, ждёт authenticated readiness до 60 секунд и
  останавливает его IPC-командой с bounded hard-kill fallback только при сбое.
- Targeted process test подтверждает profile start/shutdown/descriptor cleanup;
  Alice/Bob test теперь импортирует signed contact через runtime IPC до queue.
- `cargo fmt --all -- --check`, strict workspace all-target Clippy, все 144
  workspace tests и `cargo build --workspace --release` проходят.
- Windows artifacts: `target/release/kilogram-windows.exe` — 6,372,352 bytes,
  SHA-256 `48361DCB344064D5DA9556A01351B3CBE2E4CE41A8B9D4D1EB74AA99F6D8C891`;
  `target/release/kilogram-cli.exe` — 22,520,320 bytes, SHA-256
  `2A439A1219090E7743B44FB14F26FDCD6120CD04CA94D84F43523C6BD3A93F26`.

## M0.9.15 verification snapshot (2026-09-03)

- Runtime IPC v4 добавил bounded `WaitForChange`/`ChangeState`; connection-task
  long poll использует in-memory watch revision и не занимает actor MPSC.
- Runtime change revision публикуется после committed/meaningful contact,
  queue, delivery/retry, automatic sync и successful inbound session; exact
  idempotent contact/queue replay не создаёт wake-up.
- Desktop отдельным worker-ом coalesces change wake-ups в actor-owned
  conversations → selected history → outbox refresh; двухсекундный polling
  удалён.
- GUI load/edit/save создаёт атомарно заменяемый launch profile только для уже
  enrolled device, canonicalizes public inputs и не читает device/vault secret.
- `cargo fmt --all -- --check`, strict workspace all-target/all-feature Clippy,
  все 147 workspace tests и `cargo build --workspace --release` проходят.
- Windows artifacts: `target/release/kilogram-windows.exe` — 6,566,912 bytes,
  SHA-256 `8F2DC29B22BECEE7301D6B420426BD56E6767C5DBE0F78BDF8B87C0CA1B3681E`;
  `target/release/kilogram-cli.exe` — 22,529,024 bytes, SHA-256
  `CC581322CD473FF0137D270713B352CFA2016547F34B424EDA3BD4FB5E8E9095`.

## M0.9.16 verification snapshot (2026-09-04)

- Workspace добавил `kilogram-bootstrap` и shared bounded
  `kilogram-bootstrap-contract`; 24-word recovery phrase не попадает в receipt,
  launch profile или `Debug`.
- Account Root key использует Windows DPAPI CurrentUser envelope; device state
  сразу мигрирует в DB-primary encrypted vault без retained plaintext identity.
- `cargo fmt --all -- --check`, strict workspace all-target/all-feature Clippy,
  все 151 workspace tests и `cargo build --release --workspace` проходят.
- Release process smoke подтвердил atomic first-account layout, 24 слова,
  Root/vault protection, valid certificate/device list/prekeys и отсутствие
  phrase в persistent receipt.

## M0.9.17 verification snapshot (2026-09-04)

- `kilogram-bootstrap` добавил `device-link-request`, `device-link-inspect`,
  `device-link-authorize` и `device-link-accept`; новых system dependencies нет.
- Root enrollment использует OS file lock и atomic complete-list publication;
  accept использует authenticated DB-primary identity и trust transaction.
- Targeted tests проверяют full round-trip, exact retry, repeated accept, wrong
  SAS, wrong recipient, tampering и Device ID/key conflict.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  154 workspace tests и `cargo build --release --workspace` проходят.
- Real release process smoke `.tmp/m0917-smoke-20260904-014515` прошёл четыре
  helper-команды: request fresh, SAS equal, authority revision 2, encrypted
  response 911 bytes, accept status `device-link-accepted`.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 3,606,528 bytes,
  SHA-256 `125E8468031479EFA7C260FD5D68C54E12C737293127DAEBCCBFA0C5EB38839F`;
  `target/release/kilogram-windows.exe` — 6,545,920 bytes, SHA-256
  `A5A4667349F4567AD9356EADA398DF2229A8832CE4BCA930AE0B7748DBEE8F32`;
  `target/release/kilogram-cli.exe` — 22,547,968 bytes, SHA-256
  `99EFB35534BA4C0439564CAFA7C2DD342B9E4A5ACC275CFC0A4A1437F3B99A2D`.

## M0.9.18 verification snapshot (2026-09-04)

- `kilogram-windows` добавил four-step existing-account device-link wizard:
  explicit request/response drop targets, large SAS, exact inspected-path gate,
  bounded strict helper JSON и accepted launch-profile draft cleanup.
- Multi-source recovery UI утверждает и добавляет independent signed plans,
  выполняет одну bounded attempt, показывает scheduler status/lifecycle/attempts/
  completion, а irreversible cancel требует отдельный confirmation и создаёт
  signed terminal transition.
- Reconciliation показывает observed/complete sources, covered events,
  equivocation и exact `incomplete`/`single-source`/`agreed`/`divergent`, не
  скрывая `global_completeness_proven=false`.
- Release process smoke `.tmp/m0918-smoke-20260904-102552` прошёл create →
  request → inspect → authorize → accept и empty reconcile: request fresh,
  authority revision 2, `incomplete`, `global_completeness_proven=false`.
- Первый полный workspace test run поймал flaky valid-handshake timeout после
  deliberate invalid clients; второй — отдельный 30-second Iroh handshake
  timeout. Оба exact rerun прошли; окончательный serial workspace regression
  исключил конкуренцию transport tests и прошёл целиком: 157 tests.
- `cargo fmt --all -- --check`, strict workspace all-target/all-feature Clippy,
  все 157 workspace tests (`--test-threads=1`) и
  `cargo build --release --workspace` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 3,606,528 bytes,
  SHA-256
  `CFFBFF50FA707B37966507AE88128BB5C75C78733DB24F2ACCF7E1A55C607B08`;
  `target/release/kilogram-windows.exe` — 7,102,464 bytes, SHA-256
  `504289DC42A48C558DF9AA75F8758A645C06F965D9D565F6B8EC7EE5D572180C`;
  `target/release/kilogram-cli.exe` — 22,547,968 bytes, SHA-256
  `99EFB35534BA4C0439564CAFA7C2DD342B9E4A5ACC275CFC0A4A1437F3B99A2D`.

## M0.9.19 verification snapshot (2026-09-04)

- `kilogram-identity` добавил Root-signed portable authority package и отдельный
  exact-package witness; package охватывает current authority/revocations,
  complete device list и canonical current conversation-membership heads.
- `kilogram-bootstrap` добавил `account-recovery-export`,
  `account-recovery-inspect` и stdin-only `account-recovery-restore`; restore
  использует same-parent staging, new target и новый local provider envelope.
- Targeted regression проверил exact round-trip, продолжение monotonic enrollment,
  wrong phrase, tamper, stale package + latest witness, existing target и unsafe
  output path.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  159 workspace tests (`--test-threads=1`) и
  `cargo build --release --workspace` проходят.
- Release process smoke `.tmp/m0919-release-smoke-20260904-112123` прошёл create
  → export → inspect → stdin phrase restore: authority revision 1, package 548
  bytes, witness 147 bytes, recovered provider `windows-dpapi-current-user`.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 3,761,664 bytes,
  SHA-256 `E30E74A526F52888A361DA53C0096BB18C9FBED4BE27AE9F6F3C3A51FA8DCCD0`;
  `target/release/kilogram-windows.exe` — 7,110,656 bytes, SHA-256
  `269FF629D275393AC3D8000CCA7539C5B07F030D8D6C23C1082618D0F0EC2166`;
  `target/release/kilogram-cli.exe` — 22,564,864 bytes, SHA-256
  `D1D7D8A56AFD0CFC839D3E32A3A7D198A03BEA7716D00D67D5BABEA469519DF2`.

## M0.9.20 verification snapshot (2026-09-04)

- `kilogram-windows` добавил stopped-runtime Account Root export/inspect/restore
  panel, dedicated artifact drop targets, explicit newest-witness confirmation
  и переход к существующей device-link ceremony.
- Wizard adapter принимает bounded strict recovery JSON; phrase находится в
  zeroizing/redacted state и поступает в helper только через bounded stdin.
- Helper restore требует inspected package ID/revision и сверяет их до staging;
  regression отклоняет valid same-path package replacement без создания target.
- Configured real-process smoke отдельно прошёл с debug и release
  `kilogram-bootstrap.exe`: create → export → inspect → stdin-only restore,
  восстановленный Windows provider — `windows-dpapi-current-user`.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  162 workspace tests (`--test-threads=1`) и
  `cargo build --release --workspace` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 3,773,440 bytes,
  SHA-256 `C331BB7E8EE978C8A2921D750806A73A08F93B284AEA3DF74C125AAEC1E0CCDE`;
  `target/release/kilogram-windows.exe` — 7,203,840 bytes, SHA-256
  `0BFD06F2C0056E77809CDD72DAB821C70DCFCB7FD115B8BD920022193AB62968`;
  `target/release/kilogram-cli.exe` — 22,564,864 bytes, SHA-256
  `D1D7D8A56AFD0CFC839D3E32A3A7D198A03BEA7716D00D67D5BABEA469519DF2`.

## M0.9.21 verification snapshot (2026-09-04)

- `kilogram-identity` записывает local exact-export witness receipt только
  после повторной Root-locked проверки опубликованного package; status
  пересобирает canonical current package и различает `current`/
  `update-required` для authority и membership-only изменений.
- `kilogram-bootstrap account-recovery-status` и Windows recovery panel
  показывают current/recorded IDs/revisions и явно маркируют lifecycle scope как
  не являющийся global freshness proof.
- Targeted identity/bootstrap/desktop regression прошёл 40 tests; configured
  real debug и release helper-process smoke прошёл create → initial due → export
  current → restore current.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` и
  `cargo build --release --workspace` проходят.
- Full serial workspace run прошёл 161 из 162 tests; единственный прежний Iroh
  `runtime_outbox_delivers_and_automatic_sync_converges` не установил direct
  session до idle timeout, но exact isolated rerun прошёл. Это не затронутый
  recovery-код, поэтому причинность с M0.9.21 не установлена.
- Unit-only listener authorization tests теперь используют один IPv4 loopback
  transport с отключённым production relay map; два ранее order-dependent
  handshake tests после изменения проходят вместе.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 3,798,016 bytes,
  SHA-256 `BB78CE4D9AD5896769507586FE45170F997891820F1A2C4F27A9F6F07CD30442`;
  `target/release/kilogram-windows.exe` — 7,236,608 bytes, SHA-256
  `124006831DA0CCE6250BA70DCF98B21DA64FC116CD8176661FA98DAE86A3F73A`;
  `target/release/kilogram-cli.exe` — 22,563,840 bytes, SHA-256
  `92C7456B85EBF646C13C0EC6CB9F12F9E6AD1DB8A01C5BCD729E6220E7D39E94`.

## M0.9.22 verification snapshot (2026-09-04)

- `kilogram-identity` добавил bounded `.karq`/`.kara`, fresh challenge, exact
  package/state-vector/roster binding, expiry/replay checks, distinct-device
  strict-majority verifier и explicit three-level freshness claim.
- Recovery package теперь требует device list exact current authority revision;
  approver проверяет exact certificate, current candidate revocation и
  dominance всех локальных own-account authority/membership heads.
- `recovery-approval/latest.approval` классифицируется как Trust, читается из
  DB-primary vault и коммитится вместе с advanced high-water до публикации
  подписи. Same request retry возвращает exact committed bytes; roster change
  fail closed до joint transition.
- Targeted regression покрывает expiry/replay, duplicate/insufficient quorum,
  offline integrity-only fallback, output failure после head commit,
  idempotent publish retry, missing/stale membership, same-revision membership
  fork и different-roster conflict.
- Debug process smoke `.tmp/m0922-smoke-20260904-182036` и финальный release
  process smoke `.tmp/m0922-release-final-20260904-184009` прошли create → export → request →
  DB-primary approve → strict 1-of-1 verify: claim
  `current-device-majority-observed`, `cross_roster_fork_safety=false`.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  169 workspace tests (`--test-threads=1`) и
  `cargo build --release --workspace` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 3,977,216 bytes,
  SHA-256 `980A160AEF581304310D6489352C3B7137AB2987C9CCD437BF3149CEB8F51D1C`;
  `target/release/kilogram-windows.exe` — 7,257,600 bytes, SHA-256
  `3EF59BF8ADA6377C5FA9586E3923D81A75D231E1DA6D44BC03B849E59D154E46`;
  `target/release/kilogram-cli.exe` — 22,579,712 bytes, SHA-256
  `4594E21006566BC6E11B09EF36D91AF9803AEF463A2407C60ED7B33667B6F116`.

## M0.9.23 verification snapshot (2026-09-04)

- `kilogram-bootstrap` получил signed one-shot `.kart` listener/collector поверх
  Iroh с exact request/expiry/certificate/endpoint/bearer/route binding.
- Approval head коммитится в DB-primary до публикации ticket; network response
  повторно проходит unchanged `.kara` verifier, duplicate ticket Device IDs и
  route-policy mismatch fail closed.
- Direct loopback unit regression прошёл полный ticket publication → bearer
  fetch → signed approval → majority verification; отдельный regression
  отклоняет tampered request/route ticket fields.
- Debug process smoke `.tmp/m0923-process-smoke-20260904-190155` и release smoke
  в той же изолированной workspace прошли one-shot `direct-only` listener →
  collector: 1/1 approvals, `current-device-majority-observed`,
  `cross_roster_fork_safety=false`.
- Windows desktop adapter строго валидирует все новые JSON outputs/paths/route
  diagnostics, показывает literal claim и отделяет majority gate от explicit
  reduced-assurance offline fallback.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  172 workspace tests (`--test-threads=1`) и
  `cargo build --release --workspace` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 18,395,136 bytes,
  SHA-256 `E567028012600C23CFFEF3A36A1B4EA6B0AE3298E3B1F0BBACA9F6A256DF1C0E`;
  `target/release/kilogram-windows.exe` — 7,482,368 bytes, SHA-256
  `B87D7263A514668D7E17C466878B620103481AFFD15F71909AE7A4812FEFDC65`;
  `target/release/kilogram-cli.exe` — 22,579,712 bytes, SHA-256
  `4594E21006566BC6E11B09EF36D91AF9803AEF463A2407C60ED7B33667B6F116`.

## M0.9.24 verification snapshot (2026-09-04)

- Добавлены versioned recovery-policy state, bounded `.karpt`/`.karpa` и
  permanent canonical `.karpc` с independent strict-majority old/new roster.
- End-to-end bootstrap regression проходит `1 -> 2`: first old recovery head,
  encrypted device-link, transition approvals обоих devices, partial-quorum
  отказ, certificate/install на обоих DB-primary vault, новый 2/2 recovery
  quorum и `cross_roster_fork_safety=true` только с exact `.karpc`.
- Release process smoke
  `.tmp/m0924-release-smoke-20260904-201623` прошёл тот же CLI lifecycle:
  transition `387b75a99dd17e2d1f71c512f113c4e05573cc88b68d6b9b8e8c11b08ec8f470`,
  certificate `b3db90151f8ab0864c19aecdf8f5339b1281c56e2230f5b68d57f550cd9abba3`,
  old quorum 1/1, new quorum 2/2, policy epoch 1 на обоих devices, final
  recovery quorum 2/2 и cross-roster claim true.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  175 workspace tests (`--test-threads=1`) и
  `cargo build --release --workspace` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 18,586,112 bytes,
  SHA-256 `347820A55C351ADA11C1333A9A917307C9368C50C318DD04176F04B48032183F`;
  `target/release/kilogram-windows.exe` — 7,499,264 bytes, SHA-256
  `EFC678CCC8C145A732B072D17DC56AA77D88A49180733FD8D047DE029ED747A5`;
  `target/release/kilogram-cli.exe` — 22,584,320 bytes, SHA-256
  `E6D93BC70A82307D8CBF05F9A13BEB30AE33BBA4E34178D4A977EDB82AB3BF34`.

## M0.9.25 verification snapshot (2026-09-04)

- Recovery-policy `.karpa` теперь передаётся через signed one-shot
  `.karpticket` по Iroh `auto`/`direct-only`/`relay-only`; request, expiry,
  exact union-roster certificate, endpoint, bearer и route подписаны voter
  device key, а anti-equivocation head коммитится до ticket publication.
- Direct loopback regression запускает два listener для transition `1 -> 2`,
  собирает old 1/1 и new 2/2, отклоняет tampered route-policy ticket и создаёт
  canonical `.karpc`; collection claim остаётся false до certificate.
- Debug smoke `.tmp/m0925-process-smoke-20260904-205930` прошёл два реальных
  helper listener process, direct-only collect, certify и install epoch 1 на
  обоих devices.
- Release smoke `.tmp/m0925-release-process-smoke-20260904-210137` повторил
  lifecycle: request
  `d4fd7fa1a1c7e99bc2ccc10e52d3f339d8dc0bb85cfc39391a9b554179d0b33b`,
  certificate
  `ddb57ee11e787f2b2eb093ccfa836ed48d93dce5f6574afdb506d2c85da58095`,
  old 1/1, new 2/2, collection fork-safety false, certificate true, epoch 1
  installed on both devices.
- Windows desktop получил strict JSON-validated transition request/listen/
  collect/certify/install lifecycle и раздельные Root-operation,
  recovery-policy activation и history-recovery состояния.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  177 workspace tests (`--test-threads=1`) и
  `cargo build --release --workspace` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 18,939,904 bytes,
  SHA-256 `D8D6EE0D402E6356D541659DE5547BCEA10685FBCA911AEAF5BF08173F3DABDA`;
  `target/release/kilogram-windows.exe` — 7,740,416 bytes, SHA-256
  `8EAAB811C067AE132C4B158F09B1BE0D52CA4EFD260913CC25291B5FF956C77E`;
  `target/release/kilogram-cli.exe` — 22,584,320 bytes, SHA-256
  `E6D93BC70A82307D8CBF05F9A13BEB30AE33BBA4E34178D4A977EDB82AB3BF34`.

## M0.9.26 verification snapshot (2026-09-04)

- Identity regression моделирует durable revocation без следующей device-list
  публикации: recovery export fail-closed, exact retry переиспользует тот же
  authority operation, публикует revision 3 и снова разрешает exact export.
  Unknown target и удаление последнего active device отклоняются.
- Bootstrap regression проходит exact roster `2 -> 1`, independently witnessed
  before checkpoint, public revocation/list, after checkpoint receipt и
  byte-identical retry. Strict desktop JSON regression сохраняет пять отдельных
  lifecycle claims и отвергает ложное `history=deleted`.
- Release process smoke
  `.tmp/m0926-release-smoke-20260904-213349` прошёл create → encrypted
  device-link → before export → remove → idempotent remove retry → policy
  transition request. Authority `2 -> 3`, roster `2 -> 1`, checkpoint `current`,
  policy transition `0 -> 1` с thresholds old 2 devices/new 1 device.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets -- -D warnings`, все 180 workspace
  tests и `cargo build --workspace --release` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 19,014,656 bytes,
  SHA-256 `ED391038D34CBFBFFEF55CB4953B67AB99C742E9044D62F64AD8A2D3BDA015C5`;
  `target/release/kilogram-windows.exe` — 7,838,720 bytes, SHA-256
  `395E0BECA0AB9C2174B0014A5756BCF495D33D9F4C6FD6C0B3536C3195B0E680`;
  `target/release/kilogram-cli.exe` — 22,585,344 bytes, SHA-256
  `C1EB232703598BBDED6840B94562FDAC4EDFAECA024163D107C0A802B7430781`.

## M0.9.27 verification snapshot (2026-09-04)

- Live runtime regression применяет Root-signed roster `2 -> 1` без restart,
  сохраняет Endpoint/route/requester authority, атомарно заменяет public ticket,
  удаляет ratchet session и prekey observation и повторяет exact IPC command
  идемпотентно с нулём новых removals.
- Injected failure после удаления обоих ratchet records откатывает filesystem
  workspace; успешная transaction коммитит два authenticated vault removals.
  Windows adapter regression проверяет exact selected device-list path и typed
  IPC v5 result.
- Release process smoke
  `.tmp/m0927-release-smoke-20260904-222939` прошёл create → encrypted
  device-link → exact recovery export → permanent device removal → отдельный
  long-lived release runtime → authenticated live apply. Authority `2 -> 3`,
  active roster `2 -> 1`, removed Device ID
  `534939dda17bc5d3c231e6ce384bbd023bd62355fb5c3ba11efe34b8ac875c29`,
  ticket опубликован; future fanout exclusion и old-history readability выданы
  как отдельные claims.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  183 workspace tests (`--test-threads=1`) и
  `cargo build --workspace --release` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 19,014,144 bytes,
  SHA-256 `FCD1ED29600B466AE4A0111C2A559C423697DBE502BFC75CA1E3630C1AE5DBA5`;
  `target/release/kilogram-windows.exe` — 7,848,960 bytes, SHA-256
  `6C310932824B3485E2866EC2ACA78F10BE44246B20970E467D83064B189F654C`;
  `target/release/kilogram-cli.exe` — 22,645,760 bytes, SHA-256
  `C6F7E65436F1B19696BB47D424036FBA36208B72C6DF09B0AC3274C09F6B1AC8`.

## M0.9.28 verification snapshot (2026-09-04)

- Receipt-chain regression отвергает tamper, rollback и same-revision digest
  equivocation; exact live-apply retry сохраняет generation 1 и не повторяет
  ratchet/prekey retirement.
- DB-primary runtime regression атомарно коммитит Root-signed roster `2 -> 1`,
  ratchet retirement и signed receipt, затем останавливает actor, удаляет
  уже ненужный prekey-файл отозванного device и запускается со старым launch
  profile. Runtime выбирает receipt roster, публикует новый ticket без
  отозванного device и сообщает `authenticated-receipt-recovered` вместе с
  `convergence-required`.
- Windows adapter после authenticated ping запрашивает IPC v6 directory status.
  Bounded profile-reconcile повторно проверяет Root signature, Account/Device,
  revision/count и digest, заменяет только `device_list_file` и отказывается
  работать после внешнего path или roster-content drift.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  185 workspace tests (`--test-threads=1`) и
  `cargo build --release --workspace` проходят.
- Release-mode smoke
  `cargo test --release -p kilogram-cli
  live_runtime_applies_revocation_republishes_ticket_and_retires_ratchet`
  проходит полный DB-primary apply/retry/restart lifecycle.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 19,014,144 bytes,
  SHA-256 `FCD1ED29600B466AE4A0111C2A559C423697DBE502BFC75CA1E3630C1AE5DBA5`;
  `target/release/kilogram-windows.exe` — 7,913,472 bytes, SHA-256
  `D8DADA49E93CD348C26C1968A708DA030CDD92ACE96E64A9A733CC351EF5D3A0`;
  `target/release/kilogram-cli.exe` — 22,723,584 bytes, SHA-256
  `842663CA448DE3707728C1FA3BE231CBD9668F915D87C2D88864A6BAE38B2EB1`.

## M0.9.29 verification snapshot (2026-09-05)

- Crypto regression проверяет recipient-bound HPKE slots, wrong key/device,
  expiry, отдельные channels для publisher devices, signed observation chain,
  idempotent replay и rollback/equivocation rejection.
- Реальный loopback HTTP fixture получает только opaque envelope по exact
  pseudonymous path. Два long-lived runtime actor с взаимно enrolled contacts
  проходят publish → fetch/install → idempotent second fetch; publisher и
  receiver high-water chains остаются в vault-primary `Runtime` repository.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  189 workspace tests и `cargo build --release --workspace` проходят.
- Release-mode lifecycle
  `cargo test --release -p kilogram-cli
  runtime_publishes_and_refreshes_an_opaque_contact_ticket_idempotently`
  проходит с двумя реальными runtime actor и локальным opaque store.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 20,460,544 bytes,
  SHA-256 `3AC84084FB06FB255FEDDF3816CA7B14FCBF410D5200EBB75D6082BC2B9BF2F3`;
  `target/release/kilogram-windows.exe` — 7,954,432 bytes, SHA-256
  `B77F0FFD831C3610FA693DAFAE982D2407811F5BA27400E7533DD8495B45B9B7`;
  `target/release/kilogram-cli.exe` — 24,397,824 bytes, SHA-256
  `5FF658D027BF428C448E0EF9A2CE45581D24E3FFCDCC73DFFE9FE9566C2BA8D3`.

## M0.9.30 verification snapshot (2026-09-05)

- Отдельный release `kilogram-ticket-store.exe` запущен на случайном loopback
  порту с Redb в `.tmp/m0930-release-smoke-final`. HTTP lifecycle дал `201`
  create, `200` exact replay, `409` same-generation conflict, `204` greater
  generation replace и `200` GET с generation 2/body `06070809`.
- Процесс сервиса был принудительно завершён без graceful shutdown. Новый
  release process с тем же data-dir успешно вернул generation 2 и те же opaque
  bytes, подтверждая crash/restart durability финального EXE.
- Release-mode runtime test
  `runtime_publishes_and_refreshes_an_opaque_contact_ticket_idempotently`
  проходит полный lifecycle двух live actors через production store.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  193 workspace tests и `cargo build --release --workspace` проходят.
- Windows artifacts: `target/release/kilogram-bootstrap.exe` — 20,460,544 bytes,
  SHA-256 `3AC84084FB06FB255FEDDF3816CA7B14FCBF410D5200EBB75D6082BC2B9BF2F3`;
  `target/release/kilogram-windows.exe` — 7,954,432 bytes, SHA-256
  `B77F0FFD831C3610FA693DAFAE982D2407811F5BA27400E7533DD8495B45B9B7`;
  `target/release/kilogram-cli.exe` — 24,397,824 bytes, SHA-256
  `DE56811114AC383DF41D678D0BD275563CB6EB256A488F630B8662BDB00BA079`;
  `target/release/kilogram-ticket-store.exe` — 2,427,904 bytes, SHA-256
  `BD7C3469E88355B81230F66F4C37C3BFC9A1586B8711708216D4E14851897179`.

- Windows Firewall hygiene: Iroh endpoints внутри Cargo test-harness теперь
  принудительно используют только `127.0.0.1:0`; relay, address lookup,
  portmapper/SSDP и optional net-report probes отключены, default wildcard
  transports удалены. Это устраняет необходимость разрешать
  меняющийся `target/*/deps/kilogram_cli-<hash>.exe`; production runtime и
  release process smoke сохраняют обычную сетевую конфигурацию. Отдельные 13
  runtime-related tests и полный serial regression из 193 tests проходят;
  workspace feature-unification действительно меняет harness suffix, но больше
  не меняет его сетевую экспозицию.

## M0.9.31 verification snapshot (2026-09-05)

- Signed scheduler unit regression проходит policy succession/disable,
  exponential `5 -> 10` backoff, success reset, expiry-derived schedule и
  minimum 30-second near-expiry recheck.
- Production opaque store между двумя live runtime actors проходит initial
  explicit bootstrap, automatic next-generation mutual publish, mutual refresh,
  persisted success heads и signed policy disable `1 -> 2`.
- Persistent monotonic runtime ticker устраняет starvation delivery/sync/
  automation при частых IPC status reads; старый outbox+automatic-sync lifecycle
  переведён на явный authenticated shutdown обоих unbounded test runtimes и
  повторно проходит.
- Windows adapter проверяет exact IPC v8 policy `TTL=900`, refresh lead `300`,
  retry `5..300`, default Ethernet/Wi-Fi allow, mobile/unknown deny и literal
  `only-while-runtime-process-is-running` без OS background service.
- `cargo fmt --all -- --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  194 workspace tests и `cargo build --workspace --release` проходят. Полный
  serial запуск составлен из зелёных bootstrap 14/14, CLI 55/55 и остальных
  workspace crates 125/125 после исправления единственной test-lifecycle гонки.
- Release-mode lifecycle
  `cargo test --release -p kilogram-cli
  runtime_publishes_and_refreshes_an_opaque_contact_ticket_idempotently`
  проходит с двумя actors, signed automation state и production store.
- Windows artifacts со стабильными именами: `target/release/kilogram-bootstrap.exe`
  — 20,460,544 bytes, SHA-256
  `3AC84084FB06FB255FEDDF3816CA7B14FCBF410D5200EBB75D6082BC2B9BF2F3`;
  `target/release/kilogram-cli.exe` — 24,533,504 bytes, SHA-256
  `D0FE0F8938248C8931D9A4D904BEFA695D35083CDD05B9C11287AE29B3FED36B`;
  `target/release/kilogram-windows.exe` — 7,988,224 bytes, SHA-256
  `9D9DEB787857DC386A892D421467F8F04AB77B522FC542845D46CF136DDF2EC6`;
  `target/release/kilogram-ticket-store.exe` — 2,427,904 bytes, SHA-256
  `BD7C3469E88355B81230F66F4C37C3BFC9A1586B8711708216D4E14851897179`.
- Cargo test harness по-прежнему использует меняющийся hash-suffixed EXE, но его
  Iroh transport ограничен loopback. Обычный пользовательский/release запуск
  использует стабильные имена выше и не требует нового firewall rule при каждой
  сборке из-за одного лишь имени файла.

## M0.9.32 verification snapshot (2026-09-05)

- State regression `authenticated_runtime_compaction_is_typed_and_rollback_safe`
  подтверждает typed Runtime-only removal, exact rollback old record/new
  checkpoint и recovery после simulated interrupted prepared transaction.
- DB-primary publication lifecycle создаёт generations `1..9`, compact-ит их до
  signed head 9/checkpoint 1, продолжает `10..17`, заменяет checkpoint на
  generation 2 и после restart видит один head 17 и один current `.rtc`.
- Multi-chain regression одновременно compact-ит observation, policy, publish
  attempt и refresh attempt chains: 32 prefix records заменяются четырьмя exact
  generation-9 anchors; signature tamper checkpoint отклоняется.
- Старый production automatic ticket lifecycle переведён с timing-dependent
  physical file counts на logical signed high-water и проходит после реальной
  background compaction.
- `cargo fmt --all -- --check`, `git diff --check`, strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, все
  197 workspace tests и `cargo build --workspace --release` проходят.
- Release-mode `runtime_ticket_compaction_` проходит оба checkpoint lifecycle;
  release-mode
  `runtime_publishes_and_refreshes_an_opaque_contact_ticket_idempotently`
  повторно проходит с двумя actors и production opaque store.
- Windows artifacts со стабильными именами: `target/release/kilogram-bootstrap.exe`
  — 20,476,928 bytes, SHA-256
  `1AB47A618A813F83E6E1243BD0855A708837B3BF2B47E5CA891BB353E9D507D9`;
  `target/release/kilogram-cli.exe` — 24,619,008 bytes, SHA-256
  `A534DF1729BEC3BB369D604D980F7D24F8934221004868E0F0AF8DD84995E5C1`;
  `target/release/kilogram-windows.exe` — 7,988,224 bytes, SHA-256
  `9D9DEB787857DC386A892D421467F8F04AB77B522FC542845D46CF136DDF2EC6`;
  `target/release/kilogram-ticket-store.exe` — 2,427,904 bytes, SHA-256
  `BD7C3469E88355B81230F66F4C37C3BFC9A1586B8711708216D4E14851897179`.

## M0.9.33 verification snapshot (2026-09-05)

- Новый shared crate `kilogram-ticket-publication` проверяет deterministic
  per-peer derivation, различие scopes, canonical lowercase key/channel/
  signature encoding и exact binding authorization к channel/generation/body.
- Production HTTP service требует write key/signature до Redb transaction:
  unsigned PUT получает 403; другой корректный Ed25519 key с `u64::MAX` на
  известном channel получает 403; legitimate generation 2 и exact bytes после
  атаки остаются читаемыми.
- Connection ticket/signature domain подняты до v10 и содержат проверенный
  derived public write key. Старые v9 contacts требуют однократного обмена
  свежими tickets; event/session/ALPN wire не изменились.
- Полный `cargo test --workspace --all-targets` проходит: 199 tests, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- Release tests `kilogram-ticket-publication`, `kilogram-ticket-store` и exact
  двухакторный
  `runtime_publishes_and_refreshes_an_opaque_contact_ticket_idempotently`
  проходят; `cargo build --workspace --release` и stable-name EXE `--help`
  smoke проходят.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,476,928 bytes, SHA-256
  `1AB47A618A813F83E6E1243BD0855A708837B3BF2B47E5CA891BB353E9D507D9`;
  `target/release/kilogram-cli.exe` — 24,667,136 bytes, SHA-256
  `A93C9B14A1A3E5F2ECD96B94289FF436621C0A9A203CA9C026316A0FC0EB223C`;
  `target/release/kilogram-windows.exe` — 7,988,224 bytes, SHA-256
  `9D9DEB787857DC386A892D421467F8F04AB77B522FC542845D46CF136DDF2EC6`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.34 verification snapshot (2026-09-05)

- Новый signed runtime endpoint-candidate record round-trip проверяет stable
  candidate ID, exact contact/device binding и signature tamper rejection.
- Contact-set regression импортирует четыре Device tickets одного peer account,
  сохраняет stable contact ID, проверяет idempotent duplicate, отклоняет пятый
  candidate и после удаления primary descriptor продолжает resolver с тремя
  authenticated candidates.
- Existing runtime outbox/automatic-sync process regression проходит без
  изменения single-candidate поведения; desktop IPC adapter и conversation
  model проходят с IPC v9 и endpoint count.
- `cargo test --workspace --all-targets`: 200 tests, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- `cargo build --workspace --release` проходит; stable-name CLI/bootstrap/store
  `--help` smoke успешен. Windows GUI stable EXE был запущен из release path и
  штатно остановлен после launch smoke (GUI не имеет CLI `--help` exit).
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,476,928 bytes, SHA-256
  `1AB47A618A813F83E6E1243BD0855A708837B3BF2B47E5CA891BB353E9D507D9`;
  `target/release/kilogram-cli.exe` — 24,745,984 bytes, SHA-256
  `038B35D41B91D396F75DB9081AB6295EB06CF409822636E7986A1BFB7677C52A`;
  `target/release/kilogram-windows.exe` — 7,990,784 bytes, SHA-256
  `E19A5A3EF0FEAF8D24D49BF54AA5CC37BC627AEB0F76F2ED022EDF92DAE1078F`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.35 verification snapshot (2026-09-05)

- Новый live-runtime regression с двумя peer Device tickets и production opaque
  store сначала публикует один из двух channels и получает честный partial
  `1/2`, затем публикует второй и получает complete `2/2`; snapshot содержит две
  независимые signed observation chains.
- Network fetches проверены параллельно, local state commits последовательно:
  первоначальный parallel-commit тест воспроизвёл lock race, после разделения
  фаз optimized regression стабильно проходит.
- Endpoint-set regression теперь проверяет четыре typed `usable` состояния,
  missing primary как отдельный `stale` при трёх surviving candidates и все
  descriptors как `authority-behind-local-high-water` после более нового pin.
- Desktop adapter и model обновлены до IPC v10; GUI показывает exact usable/
  stale counts, per-Device role/reason и local publication high-water.
- `cargo test --workspace --all-targets`: 201 tests, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- `cargo test --release -p kilogram-cli
  runtime_refreshes_each_enrolled_endpoint_channel_independently` и
  `cargo build --workspace --release` проходят; stable-name CLI/bootstrap/store
  `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,476,928 bytes, SHA-256
  `1AB47A618A813F83E6E1243BD0855A708837B3BF2B47E5CA891BB353E9D507D9`;
  `target/release/kilogram-cli.exe` — 24,806,400 bytes, SHA-256
  `6F3EE4A988CEBBF7BF437578F814671AA6C666502F9069B76171B14DD88F11FF`;
  `target/release/kilogram-windows.exe` — 8,013,312 bytes, SHA-256
  `91153956775543D3F38822C063BB13EE99E847C6F45990EA1AA6F3192E300E9B`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.36 verification snapshot (2026-09-05)

- Runtime binding unit regression проверяет signed round-trip, deterministic
  endpoint ID, exact channel key и tamper rejection.
- Live two-endpoint production-store regression сначала создаёт два durable
  bindings, удаляет один для симуляции M0.9.35 state и делает оба descriptors
  cryptographically valid, но expired. Первый refresh мигрирует legacy binding
  и даёт честный partial `1/2`; второй endpoint остаётся unusable до публикации,
  затем complete refresh даёт fresh `2/2` и две bindings/observation chains.
- `cargo test --workspace --all-targets`: 201 tests, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- `cargo test --release -p kilogram-cli
  runtime_refreshes_each_enrolled_endpoint_channel_independently` и
  `cargo build --workspace --release` проходят; stable-name CLI/bootstrap/store
  `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,476,928 bytes, SHA-256
  `1AB47A618A813F83E6E1243BD0855A708837B3BF2B47E5CA891BB353E9D507D9`;
  `target/release/kilogram-cli.exe` — 24,861,696 bytes, SHA-256
  `BE9017F96B06482449FC32C6651E53C5543BF5888595084F2213C5A4FC68FE37`;
  `target/release/kilogram-windows.exe` — 8,013,312 bytes, SHA-256
  `91153956775543D3F38822C063BB13EE99E847C6F45990EA1AA6F3192E300E9B`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.37 verification snapshot (2026-09-05)

- Новый endpoint-announcement crypto regression проверяет recipient-only HPKE
  open, bounded expiry, wrong-recipient rejection и ciphertext tamper rejection.
- Сквозной two-own-device regression создаёт exact Root-signed roster,
  переносит contact/endpoint/binding/latest observation с source на recipient,
  проверяет recipient-local records, sibling high-water и idempotent повторный
  import без дубликатов.
- `cargo test --workspace --all-targets`: 204 tests, 0 failed; final dedicated
  same-generation sibling-equivocation regression also passes, bringing the
  current suite to 205 tests.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- `cargo build --workspace --release` проходит; stable-name
  CLI/bootstrap/store `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,476,928 bytes, SHA-256
  `1AB47A618A813F83E6E1243BD0855A708837B3BF2B47E5CA891BB353E9D507D9`;
  `target/release/kilogram-cli.exe` — 25,040,384 bytes, SHA-256
  `B4C5BD1DF510239609EE4FD0FC6D936CB0E416181C115017DCBDE868643382EE`;
  `target/release/kilogram-windows.exe` — 8,021,504 bytes, SHA-256
  `9A99122A795464923815660BE6E43925FFA7BEEBD14FC281BFE78EC0BB3CE71B`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.38 verification snapshot (2026-09-05)

- Новый two-runtime network regression передаёт recipient-HPKE endpoint
  announcement между двумя устройствами одного Root Account через production
  Iroh/Device authorization/IPC path, материализует contact/endpoint/binding у
  получателя и проверяет recipient-signed session-bound acknowledgement.
- Отдельные crypto и wire regressions проверяют привязку acknowledgement к
  bundle/source/recipient/current transport session, signature tamper/replay
  rejection, non-empty envelope и лимиты 7 MiB/4 KiB.
- Same-account Device authorization принимает только byte-identical текущую
  Root authority; чужой Account сохраняет прежний monotonic peer pin/rollback
  gate. Network import использует ровно тот же M0.9.37 transaction gate, а
  каталог полученных public ticket descriptors выбирается локально и
  проверяется как обычная non-symlink directory.
- `cargo test --workspace --all-targets`: 208 tests, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- Dedicated `cargo test --release -p kilogram-cli
  runtime_pushes_endpoint_announcements_over_authenticated_own_device_session`
  и `cargo build --workspace --release` проходят; stable-name
  CLI/bootstrap/store `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,476,928 bytes, SHA-256
  `078CA596836462B2D1340A33A3FD2CAD5CE5F5A7BD8B69B043864526E706787A`;
  `target/release/kilogram-cli.exe` — 25,121,792 bytes, SHA-256
  `0DD3430CE23A9B17FA0685C2F47180D6A38BD40F20843A24067638614EA65019`;
  `target/release/kilogram-windows.exe` — 8,023,552 bytes, SHA-256
  `F9D06E5D2BA6A8F44714ACE88701FCBAF475E47C98F5F78DF740CAD7E82EA3DC`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.39 verification snapshot (2026-09-05)

- Новый two-runtime regression запускает оба listener с внешним peer Account
  как primary audience, ждёт автоматически опубликованные own-device tickets,
  выполняет explicit и scheduled same-account push по тому же endpoint и
  проверяет direct ACK, idempotent recipient import и persisted signed
  policy/attempt heads.
- Compaction regression теперь покрывает одновременно observation, ordinary
  policy/publish/refresh attempt и own-device policy/attempt chains: после
  transactional checkpoint остаётся шесть authenticated heads и restart
  принимает их без discarded prefix.
- Случайная гонка существующего ticket-automation regression устранена:
  сначала подтверждается Bob publication generation 2, затем convergence ждёт
  требуемую generation, а не любой ранее успешный refresh.
- `cargo test --workspace --all-targets`: 209 tests, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- Dedicated `cargo test --release -p kilogram-cli
  runtime_multi_audience_pushes_and_automates_own_device_announcements` и
  `cargo build --workspace --release` проходят; stable-name
  CLI/bootstrap/store `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,476,928 bytes, SHA-256
  `078CA596836462B2D1340A33A3FD2CAD5CE5F5A7BD8B69B043864526E706787A`;
  `target/release/kilogram-cli.exe` — 25,308,160 bytes, SHA-256
  `674F9842DBA0AA8A91C926AF89F8A1E537BDD0A1EADA1BD0DA254CB9ED496B5C`;
  `target/release/kilogram-windows.exe` — 8,008,192 bytes, SHA-256
  `470689F8B2BBB4356BEEB7DCF575601C2294F86BAD5A365145B60D8AF6B4EEA8`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.40 verification snapshot (2026-09-05)

- Новый two-runtime regression использует два отдельных public directories и
  реальный loopback opaque store: directional pairwise channels независимо
  совпадают у обоих Devices, shared ticket path отсутствует, свежие tickets
  устанавливаются в runtime-managed paths и authenticated announcement push
  завершается в обоих направлениях.
- Regression намеренно сначала получает `404` на ещё не опубликованном inverse
  channel, затем подтверждает retry с новой signed publication generation.
  Это закрывает randomized HPKE same-generation `409 Conflict`, включая
  неопределённый HTTP timeout после фактически принятого PUT.
- Crypto regression проверяет симметрию X25519-derived key, separation другого
  peer/context и запрет empty context. Compaction regression сохраняет семь
  authenticated chain heads, включая discovery policy, и принимает restart
  после удаления 56 prefix records.
- `cargo test --workspace --all-targets`: 211 tests, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- Dedicated `cargo test --release -p kilogram-cli
  runtime_pairwise_store_discovers_ticket_and_pushes_own_device_announcements`
  и `cargo build --workspace --release` проходят; stable-name
  CLI/bootstrap/store `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,480,512 bytes, SHA-256
  `BF65E44B6F815DD91B4A456E12E713319CB07935079B3ECAF0EF692F42D5F692`;
  `target/release/kilogram-cli.exe` — 25,509,376 bytes, SHA-256
  `1D07784141A8E1716C6EC95667E61060E82B775CA8EABAAEA13295A9B5E48896`;
  `target/release/kilogram-windows.exe` — 8,013,824 bytes, SHA-256
  `8224E35A2A2B5EC75F60169B7718A4CEB9B31846020EF1A4AA2590B98B973113`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.41 verification snapshot (2026-09-05)

- Новый pure roster regression проверяет initial projection на два sibling
  Devices, idempotent repeated configuration, запрет manual child override,
  automatic child creation после complete expanded roster, disabled generation
  после Root revocation и no-op repeated/restart reconciliation.
- Existing live runtime removal regression теперь сначала устанавливает
  roster-wide policy через IPC v15, применяет authenticated Root removal,
  проверяет parent generation 2, `active=0/configured=1/retired=1`, disabled
  child и то же состояние после receipt-based restart.
- Compaction regression сохраняет восемь authenticated heads, включая новую
  roster policy chain; после transactional checkpoint удаляются 64 verified
  prefix records, restart принимает retained generation 9.
- Первый полный прогон обнаружил ошибку только в старом test fixture: шаг с
  сообщением `re-enable discovery` передавал `enabled=false`. После исправления
  dedicated M0.9.40 two-runtime regression и повторный полный прогон успешны.
- `cargo test --workspace --all-targets`: 213 tests, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- Dedicated `cargo test --release -p kilogram-cli
  tests::roster_wide_own_device_policy_reconciles_addition_revocation_and_restart
  -- --exact` и `cargo build --workspace --release` проходят; stable-name
  CLI/bootstrap/store `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,480,512 bytes, SHA-256
  `BF65E44B6F815DD91B4A456E12E713319CB07935079B3ECAF0EF692F42D5F692`;
  `target/release/kilogram-cli.exe` — 25,729,536 bytes, SHA-256
  `48E44CA130BF95EAF968EDDE9BBFF322434414563B3A7274527EBDE43EA36201`;
  `target/release/kilogram-windows.exe` — 8,025,088 bytes, SHA-256
  `C1DF3B54316D41BF6378F9D72C201E4B39E9493662C0FE3DCDD69727EC04BAF0`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.42 verification snapshot (2026-09-05)

- Existing endpoint-announcement regression расширен до трёх authorized
  Devices: A direct observation импортируется B, B экспортирует signed accepted
  evidence, C принимает тот же publication high-water без A -> C session.
- Тот же regression создаёт validly signed conflicting same-generation claim
  от B; C отклоняет bundle до external descriptor/vault mutation, а число
  accepted records остаётся прежним.
- Девять monotonic `.aeo` generations запускают existing transactional
  compaction; после reload остаётся один highest record и matching
  `AcceptedEndpointObservation` checkpoint anchor.
- `cargo test --workspace`: 213 tests, 0 failed. `cargo fmt --all -- --check`,
  `git diff --check` и strict `cargo clippy --workspace --all-targets
  --all-features -- -D warnings` проходят.
- Dedicated `cargo test --release -p kilogram-cli
  tests::endpoint_announcements_transfer_enrollments_and_high_water_between_own_devices
  -- --exact` и `cargo build --workspace --release` проходят; stable-name
  CLI/bootstrap/store `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,480,512 bytes, SHA-256
  `BF65E44B6F815DD91B4A456E12E713319CB07935079B3ECAF0EF692F42D5F692`;
  `target/release/kilogram-cli.exe` — 25,764,864 bytes, SHA-256
  `3F7B957D3EB79D1C791D65DFEF8633631C9852828226ABAE37C801EF84F583A4`;
  `target/release/kilogram-windows.exe` — 8,025,088 bytes, SHA-256
  `C1DF3B54316D41BF6378F9D72C201E4B39E9493662C0FE3DCDD69727EC04BAF0`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.43 verification snapshot (2026-09-06)

- Three-Device endpoint-announcement regression теперь сохраняет первый
  same-generation mismatch как один detector-Device-signed `.pcf`, проверяет
  canonical proof ID, restart reload, tamper rejection и отсутствие mutation
  accepted evidence.
- Тот же regression проверяет explicit IPC `quarantined` state и proof ID,
  отказ delivery/sync candidate resolver, блокировку ticket lookup до HTTP,
  idempotent repeated conflict и отсутствие observation export из уже
  quarantined channel.
- Candidate resolver теперь сверяет fallback с pinned peer-authority high-water:
  lower revision и different bytes на equal revision не используются после
  quarantine более нового endpoint.
- `cargo test --workspace -- --test-threads=1`: 214 tests, 0 failed. Отдельно
  повторён long-running `runtime_outbox_delivers_and_automatic_sync_converges`
  после transient Windows temp-transaction `os error 3`: 1 passed, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- Dedicated `cargo test --release -p kilogram-cli
  tests::endpoint_announcements_transfer_enrollments_and_high_water_between_own_devices
  -- --exact` и `cargo build --workspace --release` проходят; stable-name
  CLI/bootstrap/store `--help` и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,480,512 bytes, SHA-256
  `BF65E44B6F815DD91B4A456E12E713319CB07935079B3ECAF0EF692F42D5F692`;
  `target/release/kilogram-cli.exe` — 25,805,824 bytes, SHA-256
  `ECF17959DE7C1921CDEABF12AFB9CC966B773D454F19D2A7FFFB6A0399819696`;
  `target/release/kilogram-windows.exe` — 8,024,064 bytes, SHA-256
  `E2000D1EECDF1079A493625294FFD63428C4D4982F4649BE61F41A33A47277DB`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `123887E3B8F607F77A5BEE8969B5B9B73ABC189BAEC1BD51AFFA925183020191`.

## M0.9.44 verification snapshot (2026-09-06)

- Three-Device regression расширен до C -> A -> B propagation полного signed
  conflict proof: local proof IDs различаются, stable evidence ID совпадает,
  replay не добавляет record.
- Тот же regression создаёт peer-signed ticket v11 с epoch 1, выпускает один
  exact-current Root-signed resolution и применяет его ко всем трём Devices.
  Старые `.pcf` остаются, effective binding использует новый channel, а
  post-resolution announcement снова импортируется.
- Tampered proof/resolution, wrong replacement ticket и silent same-channel
  replacement отклоняются. Два последовательных local `.pcrn` rotation records
  reload-ятся как contiguous epoch 1 -> 2.
- `cargo test --workspace --no-fail-fast -- --test-threads=1`: 214 tests,
  0 failed. После добавления final rotation assertions dedicated debug test
  повторён: 1 passed, 0 failed.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят.
- Dedicated `cargo test --release -p kilogram-cli
  tests::endpoint_announcements_transfer_enrollments_and_high_water_between_own_devices
  -- --exact` и `cargo build --workspace --release` проходят; stable-name
  CLI/new rotation command/bootstrap/store `--help` и hidden GUI launch smoke
  успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,480,512 bytes, SHA-256
  `84DB5B0B426A1C3F69D2B0E69BFC601BF138570B7590E983A2ECA93A535EE35E`;
  `target/release/kilogram-cli.exe` — 25,950,720 bytes, SHA-256
  `0C30B69E6F822E8EDCE2D9F19053DE21560920B763BE6FFDEEE86949D2F76976`;
  `target/release/kilogram-windows.exe` — 8,024,064 bytes, SHA-256
  `90126471E85AA39C7B03B6D88192C12F700C6963D45C392B1FE6648D97EF8AF5`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `4C90CDE6CEB527B1D02243A01EEFDA2DA043450A9127AE653F366B251CADAC7C`.

## M0.9.45 verification snapshot (2026-09-06)

- Three-Device regression проверяет разделённую recovery ceremony: C создаёт
  Device-signed `.pcrq`, offline Account Root независимо проверяет evidence,
  authority revision и replacement ticket и выпускает self-contained `.pcrp`.
  Tampered request/response и подпись чужого Root отклоняются.
- Только C применяет response вручную. Затем C -> A -> B endpoint-announcement
  propagation переносит exact Root resolution и replacement descriptor;
  все три Devices снимают quarantine, сохраняют один `.pcr` и сходятся на
  новом publication channel. Повторное применение остаётся идемпотентным.
- Desktop incident panel создаёт online request и применяет offline response,
  но не получает Account Root path или secret. IPC contract обновлён до v18,
  endpoint-announcement bundle до v4, acknowledgement до v2.
- Первый `cargo test --workspace --no-fail-fast -- --test-threads=1` дал
  213 passed / 1 failed из-за уже наблюдавшегося transient Windows race при
  создании temp transaction directory (`os error 3`). Точный повтор
  `runtime_outbox_delivers_and_automatic_sync_converges` успешен: 1 passed,
  0 failed за 109.02 s. Новый focused regression также успешен в debug и
  release profiles.
- `cargo fmt --all -- --check`, `git diff --check` и strict
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят. `cargo build --workspace --release`, справка CLI/new offline
  recovery commands/bootstrap/store и hidden GUI launch smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,480,512 bytes, SHA-256
  `84DB5B0B426A1C3F69D2B0E69BFC601BF138570B7590E983A2ECA93A535EE35E`;
  `target/release/kilogram-cli.exe` — 26,003,456 bytes, SHA-256
  `3058D030C9E136647F480322A4F08BDBCBCDE63B1FC7BB2D91483DBFBF745395`;
  `target/release/kilogram-windows.exe` — 8,055,808 bytes, SHA-256
  `2788F97D1926BEBBB6429B776C344E5D90E01F0D7D477673229423D5F7D3FC8D`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `4C90CDE6CEB527B1D02243A01EEFDA2DA043450A9127AE653F366B251CADAC7C`.

## M0.9.46 verification snapshot (2026-09-06)

- Long-lived publication regression вращает собственный channel через IPC v19,
  сохраняет тот же Iroh Endpoint ID, атомарно заменяет public ticket, оставляет
  runtime online и после shutdown reload-ит durable epoch 1.
- Three-Device regression создаёт `.pcrq` и применяет `.pcrp` через работающий
  actor: первый apply даёт `Inserted`, повторный — `Unchanged`, Root secret не
  загружается, IPC ping после обеих mutations успешен; дальнейшая C -> A -> B
  propagation остаётся неизменной.
- `cargo test --workspace --no-fail-fast -- --test-threads=1`: 214 tests,
  0 failed. `cargo test -p kilogram-windows`: 21 tests, 0 failed.
- Оба новых focused regressions также проходят в release profile. `cargo fmt
  --all -- --check`, `git diff --check` и strict `cargo clippy --workspace
  --all-targets --all-features -- -D warnings` проходят.
- `cargo build --workspace --release`, справка всех трёх IPC v19 команд,
  bootstrap/store help и bounded hidden GUI event-loop smoke успешны.
- Windows artifacts со стабильными именами:
  `target/release/kilogram-bootstrap.exe` — 20,480,512 bytes, SHA-256
  `84DB5B0B426A1C3F69D2B0E69BFC601BF138570B7590E983A2ECA93A535EE35E`;
  `target/release/kilogram-cli.exe` — 26,120,704 bytes, SHA-256
  `8A0857A209B572A69CC232FE52B25F2077C9455C612CADA973A4DF342C9AF806`;
  `target/release/kilogram-windows.exe` — 8,082,944 bytes, SHA-256
  `B00033395E08AD2BD3E1FFFF8BF7990381109912A730C57EE91E9E5B47668906`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `4C90CDE6CEB527B1D02243A01EEFDA2DA043450A9127AE653F366B251CADAC7C`.

## M0.9.47 verification snapshot (2026-09-06)

- Three-Device regression расширен на hardened offline ceremony: runtime IPC
  v20 возвращает request digest/KPC1 code, public inspector проверяет full
  signed request и exact ticket, QR round-trip даёт exact match, wrong code
  отклоняется до попытки загрузить намеренно отсутствующий Root, wrong Root
  отклоняется, matching Root выпускает response с тем же code, distinct response
  QR принимается, request QR вместо response — нет. Live apply и дальнейшая
  sibling convergence остаются успешными.
- Release focused regression
  `tests::endpoint_announcements_transfer_enrollments_and_high_water_between_own_devices`
  и release QR/confirmation unit test проходят.
- `cargo test --workspace --no-fail-fast -- --test-threads=1`: 215 passed,
  1 failed. Единственный старый
  `runtime_outbox_delivers_and_automatic_sync_converges` дал
  `endpoint state actor stopped` -> IPC timeout; точный изолированный повтор
  прошёл 1/1 за 96.40 s, обслужив delivery и семь runtime sessions. Эта
  transient lifecycle race не считается причинённой M0.9.47 и остаётся явно
  записанной, а не скрытой как полный зелёный suite.
- `cargo fmt --all -- --check`, `git diff --check`, `cargo check --workspace` и
  strict `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходят. `cargo build --workspace --release`, help трёх новых conflict
  ceremony commands, bootstrap/store help и bounded hidden GUI event-loop smoke
  успешны.
- Windows artifacts сохраняют стабильные имена:
  `target/release/kilogram-bootstrap.exe` — 20,480,512 bytes, SHA-256
  `84DB5B0B426A1C3F69D2B0E69BFC601BF138570B7590E983A2ECA93A535EE35E`;
  `target/release/kilogram-cli.exe` — 26,200,576 bytes, SHA-256
  `683546422D95F9393997E9321AD5A51120F3F67EF4E9AB1BE9C9117B5E31D3DC`;
  `target/release/kilogram-windows.exe` — 7,797,248 bytes, SHA-256
  `C50B4509127E588986742EEE844917B721DB9673707F9921D62C3B3160F8F19E`;
  `target/release/kilogram-ticket-store.exe` — 2,581,504 bytes, SHA-256
  `4C90CDE6CEB527B1D02243A01EEFDA2DA043450A9127AE653F366B251CADAC7C`.

## M0.9.48 verification snapshot (2026-09-06)

- Новый `kilogram-offline` проходит `cargo check`, focused unit regression и
  cross-component Three-Device lifecycle: online runtime создаёт `.pcrq`,
  purpose-built offline decoder проверяет его, wrong KPC1 блокирует Root load,
  offline Root выпускает `.pcrp`, а online runtime принимает response и
  продолжает sibling convergence.
- Strict workspace clippy проходит. Dependency gate насчитал 204 normal
  packages для offline binary против 480 для CLI и не обнаружил Iroh, Tokio,
  Reqwest или запрещённых Kilogram runtime/state/transport crates.
- Два development package одного source/toolchain/binary получили одинаковый
  ZIP SHA-256
  `5C6AD38064C434E802FF253DBC3D94504903F8A22DCB3DBCB2F4FEAD5E3FAF62`;
  `kilogram-offline.exe` имел SHA-256
  `C23B50A336EBA4CA0FC8417516B6F426395F226297974775381455B41762FC6F`.
- Полный direct `cargo test --workspace` был остановлен после нового Windows
  Firewall prompt от `kilogram_cli-<hash>.exe`. Предыдущая loopback-only мера
  ограничивает сеть, но не предотвращает application-path prompt. Добавлен
  `scripts/run-cargo-tests-stable.ps1`: default делает только locked `--no-run`
  и stable-name copy, а запуск требует explicit `-Run`. В этой сессии сетевой
  stable harness намеренно не запускался, чтобы не вызвать первый prompt без
  согласованного времени.

## M0.9.49 verification snapshot (2026-09-06)

- `kilogram-publication-conflict` стал общим владельцем signed publication,
  observation, proof/evidence, request, Root resolution/response и KPC1 claim;
  из CLI удалено около 1,200, из offline library около 800 строк дублированного
  wire/verification кода.
- `scripts/verify-publication-conflict-boundary.ps1` подтвердил 94 normal
  dependency records, отсутствие network/runtime/image dependencies и
  duplicate wire implementations. Offline gate подтвердил общий codec и 206
  normal records без network/runtime packages.
- `cargo check --workspace --all-targets`, strict workspace Clippy и formatting
  проходят. Общий unit test и три network-free CLI regressions запущены только
  через stable-name harness; все четыре прошли.
- Новый cross-component regression требует одинаковые online/offline request и
  response IDs, artifact digests, KPC1 и byte-exact response re-encode.
- Network-bearing regressions намеренно только компилируются: их запуск остаётся
  отдельным согласованным действием, чтобы Windows Firewall prompt не возникал
  во время другой работы пользователя.
- `cargo build --workspace --release --locked` и offline help smoke проходят.
  Два development package содержали один и тот же отдельно собранный
  `kilogram-offline.exe` — 2,458,112 bytes, SHA-256
  `B90AF46F5A10B1CB2CDDB521B4963B260E0D896B380575C3346200496264C095`
  — и получили одинаковый ZIP SHA-256
  `9308D12FCB3D8D2835E0C25F6FAAE013D5972E42330C3040B5A292A884D28D85`.
  Это проверяет deterministic packaging готового payload; отдельная повторная
  линковка пока не заявлена воспроизводимой и является целью M0.9.50.

## M0.9.50 verification snapshot (2026-09-06)

- Перед изменением cache policy exact workspace `target` содержал 145,230
  файлов и занимал 105.66 GiB; основная масса приходилась на
  `debug/incremental` (57.87 GiB) и `debug/deps` (42.68 GiB, включая 24.63 GiB
  PDB). По явному решению владельца весь `target` разово удалён.
- `[profile.verification]` наследует test, использует `line-tables-only` и
  `incremental=false`; `scripts/run-cargo-tests-stable.ps1` компилирует туда и
  по-прежнему не запускает harness без `-Run`.
- После clean `cargo check --workspace --all-targets --locked`, полного
  verification compile и первого offline release build target занимал 3.99
  GiB: debug total 1.14, debug incremental 0.39, verification 2.46, release
  0.40 GiB.
- `scripts/cargo-cache-maintenance.ps1` read-only по умолчанию, предупреждает
  после 40 GiB; `-PruneVerification` и `-VacuumAndWarm` являются explicit
  guarded destructive modes.
- После наблюдаемой 100% CPU загрузки на холодной release-сборке heavy scripts
  переведены на inherited BelowNormal priority и dynamic half-logical-CPU
  default: 12 Cargo jobs на текущих 24 logical processors. Для ad-hoc команд
  используется `scripts/invoke-cargo-friendly.ps1`; override — `-CargoJobs`
  либо `KILOGRAM_CARGO_JOBS`.
- Development reproducibility run из двух разных clean roots прошёл: оба
  `kilogram-offline.exe` по 2,458,112 bytes, SHA-256
  `C93BD111444649727CCAFCAEA01A953AAD65861B34D6459A48A490299FA76E56`.
  Это pre-commit evidence; clean exact-HEAD record создаётся после milestone
  commit.
- Scope machine-readable record — `same-host-separate-clean-roots`. Он не
  объявляется independent second-host reproduction или signed provenance.
- Сетевые test harnesses не запускались; дополнительного Windows Firewall
  prompt этот этап не создаёт.

## M0.9.51 verification snapshot (2026-09-06)

- Workspace lockfile расширен локальным `kilogram-mailbox`; все зависимости
  разрешились из существующего offline Cargo cache.
- Routine milestone policy изменена: release и ZIP не строятся без реальной
  потребности во внешнем artifact. Основные команды — resource-bounded debug
  `check`, strict Clippy и unoptimized `verification` harness.
- `scripts/run-cargo-tests-stable.ps1 -Package kilogram-mailbox -Run` запустил
  только network-free stable-name harness: 3 tests passed.
- `scripts/verify-kilogram-mailbox-boundary.ps1` подтвердил отсутствие
  network/runtime/application-ID dependencies; locked normal graph содержит 77
  records и включает только ожидаемые `kilogram-crypto`/Redb boundaries.
- После full compile-only verification target занимает 6.60 GiB, ниже 40 GiB
  warning threshold; release output 1.73 GiB является сохранённым cache
  предыдущего этапа и в M0.9.51 не пересобирался.
- Focused и full-workspace debug check/strict Clippy прошли. Network-free
  mailbox harness: 3/3 passed. Все 22 workspace harness artifacts собраны под
  stable names compile-only; network-bearing EXE не запускались.

## M0.9.52 verification snapshot (2026-09-06)

- `cargo check --workspace --all-targets --all-features --locked`, strict full
  workspace Clippy, formatting и `git diff --check` проходят с BelowNormal и 12
  Cargo jobs.
- `scripts/verify-kilogram-mailbox-boundary.ps1` повторно подтвердил 77 normal
  dependency records без network/runtime/application IDs;
  `verify-kilogram-mailbox-client-boundary.ps1` подтвердил 177 records,
  Reqwest+Redb expected boundary, отсутствие Iroh/runtime/application IDs и
  отсутствие нового binary.
- Network-free stable harnesses: `kilogram-mailbox` 4/4 и
  `kilogram-mailbox-client` 2/2 passed. Один filtered pure
  `kilogram-ticket-store` route test прошёл без bind/listener и проверил
  signed PUT/LIST/DELETE chain.
- Все 23 workspace test artifacts собраны под stable names только compile-only;
  ни один network-bearing harness не запущен. Новый Windows Firewall prompt не
  создавался.
- После focused rebuild exact `target` содержит 17,947 files / 8.14 GiB:
  debug 2.95 GiB, verification 3.46 GiB, retained old release cache 1.73 GiB.
  Это ниже 40 GiB warning threshold; release cache не пересобирался.
- Release build и ZIP package в M0.9.52 не выполнялись.

## M0.9.53 verification snapshot (2026-09-06)

- `kilogram-mailbox-provisioning` и `kilogram-cli --all-targets` проходят
  resource-bounded debug check и strict Clippy при BelowNormal/12 Cargo jobs.
- `verify-kilogram-mailbox-provisioning-boundary.ps1` подтвердил отсутствие
  direct network/runtime/storage dependencies и нового executable; normal
  dependency graph содержит 131 record, включая ожидаемые identity/crypto/
  mailbox/url boundaries.
- Network-free stable-name provisioning harness: 4/4 tests passed (local sealed
  binding, recipient/source/scope/expiry binding, tamper rejection, URL policy).
- Runtime CLI harness компилируется, но не запускается; listener/socket не
  стартовал и нового Windows Firewall prompt этап не создавал.
- Release build и ZIP package в M0.9.53 не выполнялись. После focused build и
  compile-only CLI harness exact `target` содержит 19,511 files / 10.23 GiB
  (debug 4.30, verification 4.20, retained old release cache 1.73 GiB), ниже
  40 GiB warning threshold; cache управляется прежним bounded maintenance
  policy.

## M0.9.54 verification snapshot (2026-09-06)

- `cargo check --workspace --all-targets --all-features --locked`, strict full
  workspace Clippy с `-D warnings`, formatting и `git diff --check` проходят с
  BelowNormal priority и 12 Cargo jobs.
- `verify-kilogram-runtime-mailbox-flow.ps1` подтвердил direct/relay-before-
  fallback, application-commit-before-delete, durable reverse ACK, signed
  payload/dispatch, orphan-dispatch repair, honest IPC states и отсутствие
  нового executable.
- Mailbox contract/client/provisioning boundary gates повторно прошли: normal
  dependency graphs 77/177/131 records соответственно, запрещённые runtime/
  application/network crossings отсутствуют.
- Network-free stable-name `kilogram-mailbox-client` harness: 2/2 tests passed,
  включая новый `Pending -> Stored -> cleanup` ledger state query. CLI harness
  полностью скомпилирован под stable name, но не запущен: socket/listener не
  стартовал и Windows Firewall prompt не создавался.
- Release build и ZIP package в M0.9.54 не выполнялись. Exact `target` после
  verification содержит 19,686 files / 11.32 GiB (debug 5.19, verification
  4.39, retained old release 1.73 GiB), ниже 40 GiB warning threshold.

## M0.9.55 verification snapshot (2026-09-06)

- `cargo check --workspace --all-targets --all-features --locked`, strict full
  workspace Clippy с `-D warnings`, formatting и `git diff --check` проходят с
  BelowNormal priority и 12 Cargo jobs.
- Новый `verify-kilogram-mailbox-capability-lifecycle.ps1` подтвердил
  contiguous Device-signed chain, explicit rotation/revocation, bounded
  authenticated push, durable-apply-before-session-ACK, current-head/rotation-
  overlap guards, отсутствие capability material в endpoint publication source
  и отсутствие нового executable.
- `verify-kilogram-mailbox-provisioning-boundary.ps1` повторно подтвердил 131
  normal dependency record без network/runtime/direct-storage crossing;
  `verify-kilogram-runtime-mailbox-flow.ps1` сохранил live-before-fallback,
  commit-before-delete и reverse-ACK invariants M0.9.54.
- Network-free stable-name harnesses: `kilogram-mailbox-provisioning` 5/5 и
  `kilogram-protocol` 15/15 passed, включая contiguous chain gap/fork/wrong-
  revoke rejection и bounded capability wire frames. Они не создают listener.
- Network-bearing CLI/runtime harness не запускался; online capability exchange
  пока compile-verified, а не field-verified. Windows Firewall prompt не
  инициировался. Release build и ZIP package не создавались.
- Exact `target` после проверки содержит 20,214 files / 11.63 GiB (debug 5.39,
  verification 4.50, retained old release 1.73 GiB), ниже 40 GiB warning
  threshold; автоматическая очистка не требовалась.

## M0.9.58 verification snapshot (2026-09-06)

- Изменение собирается только в обычном debug/test profile с BelowNormal и 12
  Cargo jobs; release build и ZIP package не выполняются.
- Focused network-free CLI test
  `mailbox_capability_ack_drop_fault_is_one_shot` прошёл 1/1; он проверяет
  одноразовое потребление fault flag и не открывает socket/listener.
- `kilogram-cli --all-targets --all-features --locked` проходит strict Clippy с
  `-D warnings`; rustfmt и `git diff --check` проходят.
- `verify-kilogram-mailbox-field-evidence.ps1 -SelfTest` принял valid synthetic
  lifecycle set и fail closed отклонил store output после добавления
  `conversation_id`.
- `verify-kilogram-mailbox-field-test-boundary.ps1` подтвердил debug-only fault,
  durable-apply-before-drop-before-sign ordering, runtime stop только после
  vault mirror, отсутствие build/release/ZIP в operator helpers и отсутствие
  нового executable.
- Все предыдущие blind-mailbox, runtime flow, capability lifecycle,
  convergence и desktop-control boundary gates повторно прошли.
- Реальный network field test не запускался, поэтому Windows Firewall surface
  не создавался. Переносимый debug test kit будет подготовлен только по явному
  запросу перед согласованным Alice/Bob окном.

## M0.9.59 verification snapshot (2026-09-06)

- Добавлен manual-only GitHub Actions second-builder contract; push/PR/schedule/
  release triggers отсутствуют, поэтому первоначальная публикация repository не
  запускает release build.
- `verify-kilogram-independent-builder.ps1 -SelfTest` прошёл coherent synthetic
  evidence и доказал fail-closed rejection после изменения external EXE.
- `verify-kilogram-independent-builder-boundary.ps1` подтвердил exact action
  pins, least job permissions, exact commit/hash inputs, attest-only-on-match,
  stable EXE name, отсутствие local ZIP/package и mandatory production
  attestation constraints.
- `git diff --check` проходит. Rust source не менялся, поэтому Cargo check/test/
  Clippy не запускались; release build, ZIP и Kilogram network process не
  запускались, Windows Firewall surface не создавался.
- Реальный GitHub workflow сознательно не запускался. Внешний hash/attestation
  появятся только после отдельного clean M0.9.50 run и ручного dispatch для
  exact commit.

## M0.9.59 external execution addendum (2026-09-08)

- Manual GitHub Actions run `34152872647` на exact commit
  `e84d557dd80e07721296777aebe8ebbc6a8af392` успешно собрал isolated
  `kilogram-offline.exe`, но strict byte gate завершился ожидаемым failure.
- Local artifact: SHA-256
  `c93bd111444649727ccafcaea01a953aad65861b34d6459a48a490299fa76e56`,
  2,458,112 bytes, PE linker 14.44. Hosted artifact: SHA-256
  `53bc307e9ab1aee35fb98332af2044177421613756dc94e8c22775de0a3c3bb2`,
  2,459,136 bytes, PE linker 14.51.
- GitHub runner `win25-vs2026`, image `20260824.214.3`; rustc/cargo 1.98.0 на
  обеих сторонах. Attestation step был skipped, divergent evidence сохранён
  bounded artifact-ом. Это independent compile evidence, не reproducibility.

## M0.9.60 verification snapshot (2026-09-08)

- `test-kilogram-mailbox-store-preflight.ps1 -SelfTest` network-free принял
  coherent trusted-HTTPS/startup-log fixture и отверг wrong pinned key и remote
  cleartext HTTP.
- Portable driver `-ListSteps` детерминированно вывел 23 шага от
  `store-preflight` до `verify`; source parse прошёл без PowerShell syntax
  errors.
- `verify-kilogram-mailbox-field-evidence.ps1 -SelfTest` повторно принял valid
  lifecycle evidence и отверг metadata leak.
- Все семь static gates прошли: blind mailbox, runtime flow, capability
  lifecycle, convergence, desktop control, field harness и новый M1 acceptance
  kit boundary.
- Новый gate подтвердил default Windows TLS trust без redirect/certificate
  bypass, stable debug EXE hashes, local-private profile/IPC/state boundary,
  complete prerequisite-ordered step map, отсутствие release/ZIP и нового EXE.
- Negative generator check с одним Cargo job fail closed остановился на dirty
  worktree до output/build; clean build path оставлен до test window.
- `git diff --check` прошёл. Rust source не менялся; Cargo check/Clippy не
  требовались. Debug EXE не пересобирались, network/Firewall surface и архив не
  создавались.

## M0.9.62 verification snapshot (2026-09-08)

- Affected mailbox/store/Iroh/runtime packages прошли offline `cargo check` и
  strict all-target Clippy с `-D warnings`, BelowNormal priority и четырьмя
  Cargo jobs.
- Четыре focused network-free tests прошли 1/1 каждый: signed offer
  tamper/expiry, request-digest peer frame, offer EndpointAddr round trip и
  durable peer-service PUT/idempotent replay/rate limit.
- Старый volunteer-storage gate повторно подтвердил default 200 MiB,
  500+500 MiB/30 days, mobile-off, durable quota и no-new-EXE boundary.
- Новый `verify-kilogram-volunteer-iroh-boundary.ps1` подтвердил dedicated ALPN,
  capability-authenticated PUT/LIST/DELETE, endpoint-bound expiring offer,
  peer-identity rate limit, bounded runtime tasks и manual-only discovery.
- `cargo fmt --check` и `git diff --check` проходят. Network-bearing test или
  runtime не запускался, поэтому новый Firewall prompt не инициировался;
  release build и ZIP не создавались.

## M0.9.63 verification snapshot (2026-09-08)

- Разработка и проверки используют offline debug/test profile, BelowNormal
  priority и максимум четыре Cargo jobs; release build и ZIP не выполняются.
- `kilogram-mailbox-client` 5/5 и `kilogram-runtime-ipc` 15/15 network-free
  tests прошли; focused CLI provider import/selection test прошёл 1/1 без
  bind/listen/connect.
- Runtime integration compile-проверен; provider registry import тестирует
  real signed Iroh `EndpointAddr`, но endpoint не запускает.
- `verify-kilogram-volunteer-provider-selection-boundary.ps1` проверяет
  durable bounded registry, monotonic/canonical import, deterministic
  transport-distinct selection, capability-free IPC и no-new-EXE boundary.
- Предыдущие volunteer storage и Iroh gates обновлены только на честный current
  discovery status (`verified-expiring-offer-registry`) и повторно проходят.
- Full workspace offline check и strict Clippy `-D warnings`, rustfmt и diff
  check прошли. Network-bearing runtime не запускался, поэтому Windows Firewall
  surface не создавался.

## M0.9.64 verification snapshot (2026-09-08)

- Реализован automatic bidirectional provider-offer gossip поверх existing
  Root-authorized Device/Iroh session: максимум 8 offers, 2 KiB/offer, 2 hops,
  15-minute age, 60-second frame, 24 KiB protocol wire и один exchange максимум
  раз в пять минут; global directory отсутствует.
- Registry сохраняет hop provenance в отдельной Redb table, поэтому прежний
  M0.9.63 offer record остаётся совместим; direct observation может сбросить hop
  в 0, но обычный gossip replay сохранённый hop не уменьшает.
- Runtime импортирует собственный store-signed offer, обновляет его раз в пять
  минут и не чаще раза в пять минут выбирает authority-current contact для
  bounded exchange;
  actual mailbox replication пока отключена.
- `kilogram-mailbox-client` 7/7 и `kilogram-protocol` 16/16 network-free tests,
  focused CLI reply-binding/endpoint-import test 1/1 прошли в debug profile с
  BelowNormal priority и четырьмя Cargo jobs.
- Новый provider-gossip gate и три предыдущих volunteer gates проходят;
  release/ZIP не создавались, network endpoint/EXE не запускался, поэтому новый
  Windows Firewall prompt не инициировался.

## M0.9.65 verification snapshot (2026-09-08)

- `kilogram-mailbox-client::replication` сохраняет в отдельном Redb exact
  encrypted PUT, dispatch binding, immutable random salt, 3/2 policy, attempts
  и transport-distinct store receipts; retry остаётся возможен после завершения
  primary HTTPS upload.
- Runtime после direct failure выбирает remote offers, исключает own endpoint,
  уже использованные transport identities и недостаточный max-record, затем
  выполняет capability-authenticated PUT через existing mailbox ALPN.
- Volunteer failure не блокирует existing HTTPS delivery; Iroh LIST/DELETE
  retrieval остаётся M0.9.66, поэтому runtime честно печатает
  `https-compatible-pending-iroh-read`.
- Проверки выполняются debug/test/verification profile с BelowNormal priority и
  максимум четырьмя Cargo jobs. Network endpoint/EXE, release и ZIP не должны
  запускаться в локальной automated verification.

## M0.9.66 verification snapshot (2026-09-08)

- `kilogram-mailbox-client` получил отдельные Immediate-durable inbound commit
  и deletion records по exact mailbox/item/store/transport/stored receipt;
  delete authorization недоступна до application commit, а signed delete
  receipt сохраняется до bounded expiry.
- Runtime concurrently выполняет capability-authenticated Iroh LIST максимум к
  трём verified providers, запрашивает один item с каждого, исключает own
  endpoint и переиспользует общий HTTPS/volunteer application commit path.
- Незавершённые DELETE возобновляются по fresh matching offer после restart;
  одинаковый item на разных stores не схлопывается в одну запись.
- `kilogram-mailbox-client` 9/9 network-free tests и targeted CLI compile check
  прошли в debug/test profile с BelowNormal priority и четырьмя Cargo jobs.
- Старый replication gate и новый volunteer retrieval gate проходят; rustfmt и
  diff check проходят. Network runtime/EXE, release и ZIP не запускались,
  поэтому Windows Firewall surface не создавался.
- HTTPS остаётся compatibility fallback до M0.9.67 field evidence. Random
  bounded provider sampling не считается scalable exact replica discovery.

## M0.9.67 verification snapshot (2026-09-08)

- Добавлен clean-HEAD no-archive generator обычной debug-папки с одним
  stable-name `kilogram-cli.exe`; release profile, ZIP и автоматический network
  launch запрещены static gate-ом.
- Provider bootstrap реально проверен network-free: созданы отдельные Account
  Root, Device enrollment, runtime profile и volunteer policy, после чего
  identity успешно прочитана; временный self-test state удалён.
- Все field PowerShell scripts проходят AST parse; fail-closed evidence verifier
  проходит synthetic self-test, а field-kit boundary gate подтверждает private
  secret boundary, no-clobber evidence и отсутствие нового EXE.
- Два provider-only runtime могут работать на одном Alice host с отдельными
  identity/state/store key. Это mechanism evidence, не operator-independent
  durability.
- Первый clean generator run обнаружил несовместимость UTF-8-without-BOM с
  вложенным Windows PowerShell 5; проверки переведены на текущий PowerShell host,
  после чего clean-HEAD generator успешно создал debug kit.
- Собранный kit проверен отдельно: exact source revision, один stable-name EXE,
  8 parseable operator scripts, совпадающий SHA-256, ноль архивов и успешный
  copied-verifier self-test.
- Реальный Iroh/HTTPS runtime здесь не запускался, поэтому Firewall surface не
  создавался; внешний field result пока pending.
- На 2026-09-18 добавлен three-folder operator harness с шестью запускаемыми
  scripts, одним stable-path `kilogram-cli.exe` и loopback-only compatibility
  store. Он автоматически создаёт fresh Alice/Bob/provider identities и не
  требует ручных ID, config или абсолютных путей.
- Девять новых PowerShell files проходят AST parse; static simple-kit gate
  подтверждает relative path bootstrap, `%LOCALAPPDATA%` live-state boundary,
  two-provider process, Alice 2/2/offline evidence и Bob receive/restart/history.
  Network runtime при этой проверке не запускался; внешний run pending.
- Clean commit `cbca23c` собран прямо в requested
  `C:\Users\Paul\YandexDisk\!M\M0.9.67`: exact folders `1/2/3`, один общий
  stable-path CLI, loopback store, 8 scripts, два hash-verified debug artifacts
  и ноль архивов. Copied evidence verifier self-test и AST parse проходят как
  в PowerShell 7, так и в штатном Windows PowerShell 5.
- Background-process helper отдельно проверен без сети через real `identity`
  child process, redirected log и pattern wait. Ни runtime endpoint, ни store
  listener при генерации/verification не запускались; Firewall prompt не
  инициировался.
- Первый внешний запуск simple harness дошёл до Alice send и выявил stale IPC
  descriptor от bootstrap: file wait завершался до перезаписи descriptor новым
  runtime, после чего `runtime-ipc-ping` попадал в уже остановленный процесс.
  Исправление удаляет descriptor перед launch и ждёт успешный ping; synthetic
  probe с фактическим stale descriptor подтвердил fail-closed ожидание.
- Повтор Alice send разрешён только если отсутствуют queue/offline/marker и send
  log ещё не содержит outbound result. Pre-queue logs сохраняются с
  `.failed-pre-queue-<UTC>` suffix, а два текущих provider offers перед import
  атомарно обновляются последними complete значениями из live logs.
- Второй Alice send создал queue item, успешно записал compatibility copy и
  дошёл до двух реальных direct Iroh provider sessions. Оба providers завершили
  request, но requester получил `connection lost: closed by peer ... code 0`:
  provider закрывал connection сразу после `SendStream::finish()`, не дожидаясь
  подтверждения response bytes. Одновременно offline Bob automatic sync через
  early `ensure!` завершал весь runtime.
- Исправление ждёт `send.stopped()` после response FIN, удерживает requester до
  graceful close и превращает failed automatic sync в logged non-fatal attempt.
  Existing Alice queue/replication ledger сохраняются; harness умеет resume
  exact queued item и перезапуск тех же provider identities после замены CLI.
- Актуальный recovery kit содержит девять parseable PowerShell files: стандартный
  six-launch flow неизменен, а `2\02_RESTART_PROVIDERS_AFTER_FIX.ps1` является
  одноразовым recovery action только для уже начатого external run.
- Первый recovery resume был безопасно остановлен до runtime launch из-за
  CRLF-sensitive regex над `03-alice-queue.log`: evidence содержал exact queued
  status, но multiline `$` видел остающийся `\r`. Resume guard переведён на
  line-wise `Get-M0967ExactValue` и проверяет совпадение request/queue ID,
  durable store outcome, encrypted-at-rest marker и exact queued status.
- Следующий recovery run успешно получил две независимые volunteer receipts и
  достиг `runtime_mailbox_replication_status=satisfied`, но Windows PowerShell 5
  превратил ожидаемый stderr отрицательного post-stop `runtime-ipc-ping` в
  terminating `NativeCommandError`. Alice send теперь умеет отдельно завершить
  уже satisfied attempt без network retry, подавляет stderr только у ожидаемого
  отрицательного probe и разбирает receipt keys построчно, независимо от CRLF.
  Та же безопасная probe-семантика применена к standalone sender-offline helper.
- Первый Bob receive завершился до provider import: удалённый harness пытался
  извлечь offer из активно синхронизируемого provider log и увидел его временно
  неполную Yandex Disk версию. Provider host теперь атомарно refresh-ит короткие
  `.offer` files каждые 10 секунд; consumers читают только их. Bob receive может
  архивировать и повторить attempt лишь до первого inbound commit/delete.
- Первый recovery wrapper проверял semantic markers и мог стартовать на
  промежуточной Yandex Disk версии `common.ps1`, пока atomic replacement ещё не
  дошёл до ноутбука. Для текущего external run добавлен exact-version wrapper с
  SHA-256 пары `common.ps1` + `02_RECEIVE_BOB.ps1`; deployment этих файлов и
  wrapper выполняется atomic rename в sync-root.
- Повтор с exact script hashes показал, что этого недостаточно: Yandex Disk
  способен доставить два `.offer` и publication state разными поколениями.
  Current-run publisher теперь атомарно пишет JSON manifest с SHA-256/expiry,
  а Bob recovery ждёт согласованный набор с запасом срока, копирует его в
  immutable local snapshot и только затем запускает importer. Native CLI stderr
  больше не теряется из-за Windows PowerShell 5: failure сохраняется отдельным
  timestamped evidence-файлом. Предыдущие Bob attempts подтверждены как
  pre-inbound (`0` commit, `0` delete), поэтому bounded retry остаётся безопасным.
- Следующая recovery attempt приняла exact scripts и fresh offer snapshot, затем
  атомарно архивировала прежний `04-receive-bob.log`, но не смогла создать новый
  shared redirect log; provider import/failure evidence не начались, inbound и
  delete по всем attempts остались `0/0`. Bob runtime/restart logs перенесены в
  unique local attempt directory и публикуются в evidence только после process
  close; outer catch теперь всегда публикует exact stage/exception/script stack.
- Local-first retry подтвердил корректный fresh snapshot и runtime IPC. Первая
  попытка импортировала оба provider, но немедленный select завершился code 1;
  следующая заменила provider1, после чего provider2 import завершился code 1.
  Inbound/delete остались `0/0`. Recovery теперь повторяет monotonic/idempotent
  import и read-only selection bounded 12 x 750 ms, выбирает два существующих
  provider и пишет native stderr через temporary file, обходя PowerShell 5
  stderr conversion.
- Последующий Bob attempt подтвердил network и storage path: event
  `6a868866...` был локально `Inserted`, acknowledgement `46aee504...` также
  записан, а повторные polls видели `AlreadyPresent`. STDERR показал точную
  причину отсутствия DELETE: `reverse acknowledgement has no current peer
  mailbox binding`. Это не transport failure и не потеря сообщения, а неверная
  связь optional reverse upload с mandatory post-commit replica lifecycle.
- Исправление делает reverse acknowledgement preparation best-effort после
  durable application commit. Targeted regression test проходит; static
  volunteer-retrieval и simple-kit gates проходят. Полный CLI suite: 73/74,
  один отдельный existing live-runtime test стабильно падает на DB-primary
  typed-shadow mismatch в background mailbox upload и не проходит даже при
  отдельном запуске; этот результат не маскируется как успех текущего fix.
- Fix commit `b095d7a` собран debug с `--jobs 2`/below-normal. В existing
  `C:\Users\Paul\YandexDisk\!M\M0.9.67` атомарно установлен stable-path CLI
  SHA-256 `07f61075...`, обновлены `BUILD-INFO.json`, `common.ps1` и Bob receive.
  Те же provider identities перезапущены на новом binary; store keys сохранились
  (`ead890...`, `c524c6...`), endpoints новые, fresh offer publication активна.
- На Bob добавлен exact recovery entrypoint
  `3\06_RESUME_BOB_AFTER_POST_COMMIT_FIX.ps1`: он ждёт exact CLI/build/script
  hashes и согласованный fresh two-offer snapshot, затем повторно обрабатывает
  уже durable event idempotently и не создаёт новое сообщение. Deployed script
  проходит Windows PowerShell 5 AST parse; provider processes остаются запущены
  до финального `1\03_VERIFY.ps1`.
- Bob recovery завершён успешно. Shared evidence содержит по две строки
  `runtime_mailbox_inbound_source=volunteer-iroh` и
  `runtime_mailbox_replica_delete_status=deleted-after-commit`; restart log не
  содержит повторной delivery, history содержит marker ровно один раз.
  Финальный verifier вернул `result=verified`,
  `sender_evidence=recovered-log-chain` и
  `recipient_retrieval=two-volunteer-replicas-committed-and-deleted`.
- Первоначально `06` выглядел зависшим, потому что 15-second heartbeat начинался
  только после runtime startup/IPC/provider import. Common harness теперь
  печатает Bob stage, IPC/log wait, provider import, каждый bounded retry и
  restart observation. Final `03_VERIFY.ps1` разрешает безопасный повтор после
  уже скопированного identical `06-boundaries.log` и не читает stale/unset
  `$LASTEXITCODE` после успешного вложенного PowerShell script.

## M0.9.68 verification snapshot (2026-09-19)

- Добавлен recipient-selected Device-signed `MailboxReplicaSetCommitment`:
  canonical 2–8 store keys, current runtime fan-out 3, domain-separated
  commitment ID и новый append-only `ActivateWithReplicaSet` action без
  Account/Device/conversation/mailbox IDs или capability secrets.
- Sender сохраняет exact commitment рядом с per-item replication plan в
  Immediate-durable `mailbox-replica-set-locators-v1`; conflict replay fail
  closed, restart удерживает set, expiry удаляет locator вместе с plan.
- Sender и recipient используют exact store-key lookup по fresh signed provider
  offers. Старый `Activate` остаётся совместимым и явно маркируется
  `legacy-random-fallback`; global directory, server/listener и новый EXE не
  добавлены.
- Во время полного CLI regression найден delayed state-vault mismatch: writable
  Redb open/close менял служебные байты client/replication DB без логического
  изменения. Idle queue, provider и replication inspection переведены на
  `ReadOnlyDatabase`; cleanup выполняется только как явная vault-mirrored write.
- Отдельные unit tests сравнивают BLAKE3 DB до/после read-only inspection и
  подтверждают, что отсутствующая DB не создаётся.
- Проверки: `kilogram-mailbox-provisioning` 7/7,
  `kilogram-mailbox-client` 13/13, полный `kilogram-cli` 74/74; targeted
  long-lived revocation runtime regression прошёл отдельно. `cargo fmt --check`,
  Clippy `-D warnings` для трёх затронутых packages и
  `verify-kilogram-volunteer-replica-locator-boundary.ps1` проходят.
- Сборки и тесты выполнены только в debug/dev с `CARGO_BUILD_JOBS=2`; release и
  ZIP не создавались, network listener не запускался.

## M0.9.69 verification snapshot (2026-09-19)

- Добавлен `scripts/m0969-exact` — отдельный clean-run three-folder harness без
  repair wrappers. Порядок шести запусков: Alice prepare, providers start, Bob
  prepare, Alice send, Bob receive, Alice verify.
- Provider script больше не ждёт готовности клиентов: один publication manifest
  с SHA-256/expiry двух offers появляется до Bob mailbox activation. Consumers
  копируют только согласованный fresh set в local immutable snapshot.
- Bob pre-activation import сохраняется отдельным evidence; первая capability
  обязана иметь `mailbox_replica_set_store_count=2` и
  `mailbox_replica_set_discovery=exact-authenticated`.
- Prepare phase не заканчивается до bidirectional convergence: Alice durable
  пишет exact capability update, Bob durable пишет session-bound ACK. Update ID
  проверяется на обеих сторонах.
- Bootstrap runtimes больше не оставляют двусмысленный endpoint race: каждый
  final runtime атомарно заменяет свой ticket, peer ждёт отличный SHA-256 и
  валидирует fresh signed descriptor через новый CLI surface
  `runtime-ipc-contact-add` поверх уже существующего actor `AddContact`. Account,
  Device и исходный controlled descriptor path должны остаться теми же.
- Sender/recipient scripts требуют один activation commitment ID, exact resolved
  `2/2` и равенство provider keys с attempts/receipts/polls/committed sources;
  любая legacy fallback строка завершает run ошибкой.
- Evidence verifier self-test проходит positive fixture и доказывает fail-closed
  отклонение legacy marker, substituted poll key и stale bootstrap ticket.
  Static kit gate также проходит; PowerShell AST parse успешен для всех 10
  новых scripts. Полный `kilogram-cli` regression: 74/74.
- Generator использует debug profile, stable EXE names и `CargoJobs=2`, а
  `BUILD-INFO.json` связывает SHA-256/length обоих EXE, шести operator scripts,
  common helper, verifier и boundary log. ZIP/release/network launch отсутствуют.
  Реальный внешний run и итоговый `result=verified` ещё ожидаются.

### External retry observation

- Первый clean external retry без VPN прошёл prepare и Alice send, но Bob
  receive увидел маленький `alice-sent.marker` раньше, чем Yandex Disk доставил
  большие `01-provider1.log`/`01-provider2.log`. Сбой произошёл до запуска Bob
  runtime и до inbound commit, поэтому разрешён точечный Bob-only retry без
  повторной отправки Alice.
- Причина уточнена: provider runtime logs остаются открытыми и меняются всё
  время обслуживания, поэтому Yandex Disk может не публиковать их до остановки
  providers. `Get-M0969ProviderStoreKeys` теперь вообще не использует live logs:
  он bounded читает два exact store key из закрытого
  `02-bob-providers-before-activation.log`, который причинно предшествует
  capability activation. Progress печатается каждые 15 секунд, timeout — 30
  минут. Static kit gate и evidence verifier self-test проходят.
- Следующий Bob-only retry обнаружил отдельный продуктовый дефект после
  возможного принудительного завершения runtime: read-only mailbox-client Redb
  open возвращал typed `DatabaseError::RepairAborted`, поэтому runtime не
  доходил до inbound polling, хотя sender уже имел durable exact `2/2`
  replication receipts. Runtime теперь выполняет ровно один writable recovery
  под state lock и `VaultDualWriteGuard`, повторяет read-only inspection и
  оставляет все остальные DB ошибки fail closed.
- Проверки автоматического восстановления: targeted unit test, полный
  `kilogram-cli` regression 75/75, `cargo fmt --check`, Clippy `-D warnings`,
  static M0.9.69 kit gate и `git diff --check` проходят. Сборка и тесты — только
  debug/dev с `CARGO_BUILD_JOBS=2`; release/ZIP/network launch отсутствуют.
- Полевой `05_REPAIR_AND_RETRY_BOB.ps1` успешно открыл mailbox-client ledger
  writable (`status=runtime-mailbox-inspected`), но Yandex Disk немедленно
  восстановил удалённые canonical `06-receive-bob.log*` с другого хоста. Их
  SHA-256 побайтово совпал с сохранённым первым failed attempt, поэтому `04`
  отказался создавать новую попытку до runtime launch. Для функционального
  recovery добавлен внешний `06_RETRY_BOB_WITH_UNIQUE_EVIDENCE.ps1`: он не
  трогает эти конфликтующие файлы, использует timestamped local/shared paths и
  повторяет исходные exact-locator проверки. Такой retry остаётся recovery
  evidence и не превращает run в clean M0.9.69 attestation.
- Первая ревизия `06` завершалась до runtime из-за CRLF-sensitive regex на
  последней строке repair evidence. Проверка заменена на exact line membership
  через `Get-Content`; локально подтверждены PowerShell AST и совпадение
  `status=runtime-mailbox-inspected` с реальным Windows-журналом.
- Поскольку повторная ранняя ошибка `06` не создала runtime evidence, добавлен
  `07_RUN_AND_CAPTURE_BOB_RECOVERY.ps1`: он ждёт exact SHA-256 исправленного
  `06`, запускает его отдельным PowerShell и с первой строки перенаправляет
  stdout/stderr/result в timestamped shared evidence. Следующая ошибка больше
  не требует фотографии консоли.
- Captured stderr показал второй wrapper-only дефект: `Invoke-Expression`
  выполнял исходные bootstrap-строки `04`, но динамический script block не имел
  file-backed `$PSScriptRoot`, из-за чего `Join-Path` получил пустой `Path`.
  `06` теперь заранее загружает/проверяет `common.ps1`, валидирует первые две
  строки `04` и исполняет только остальное тело. AST исходного wrapper и
  получаемого dynamic body проверены; `07` ожидает новый exact SHA-256.
- Изолированный recovery attempt `20260919T005959Z` завершился успешно. Два
  pre-activation store key точно совпали с unique poll/source sets; получены две
  строки `runtime_mailbox_inbound_source=volunteer-iroh`, обе реплики удалены
  `deleted-after-commit`, exact discovery присутствует, legacy fallback — 0.
  После restart redelivery — 0; Bob history содержит message marker ровно один
  раз и acknowledgement. Loopback HTTPS fixture оставался offline (ожидаемые
  connection-refused строки), поэтому доставка фактически выполнена volunteer
  Iroh providers. Это подтверждает repair/recovery semantics, но из-за цепочки
  retry не является clean single-revision M0.9.69 field attestation.
- После публикации commits `60b5a8b` и `2cd9cb6` с clean HEAD создан новый
  одноразовый комплект
  `C:\Users\Paul\YandexDisk\!M\M0.9.69-clean2` из revision
  `2cd9cb63075d8cbe84b2b8b05e93d25a4a2220c2`. Generator и все static gates
  прошли; debug build использовал два Cargo jobs. Проверка комплекта: 11/11
  artifact SHA-256/length совпадают, PowerShell parse errors — 0, ZIP — 0,
  release/network execution — false, `1\shared` до полевого запуска отсутствует.
  Clean external run ещё ожидается.
- Clean2 field run `20260919-141905` при заранее отключённых VPN и mobile Wi-Fi
  успешно создал providers, membership, fresh Alice/Bob tickets и exact
  commitment `2/2`, но оба capability-convergence направления повторяли
  30-секундный timeout через автоматически выбранный
  `https://euc1-1.relay.n0.iroh.link./`. Это совпадает с ранним M0.3 failure
  signature; `aps1` в той же home-to-cellular topology ранее успешно перенёс
  strict relay delivery и restart sync. Provider processes остановлены через
  `STOP-PROVIDERS.marker`; три постоянно синхронизировавшихся файла были live
  logs/refreshing publication, а не causal inputs.
- Следующий clean kit pin-ит все Alice/Bob/provider profiles к
  `https://aps1-1.relay.n0.iroh.link./`, сохраняя `route_policy=auto` и direct
  upgrade. Manifest/build info фиксируют выбор, а fail-closed evidence verifier
  требует совпавший relay во всех retained phases и synthetic negative test
  отвергает `euc1` divergence. Это детерминизация field fixture, не production
  relay default.
- После прохождения static gate и verifier self-test из clean revision
  `6cea6074fbb4116639db3f79dddc0bb4bf373fe9` создан комплект
  `C:\Users\Paul\YandexDisk\!M\M0.9.69-clean3`. Debug build занял два Cargo
  jobs; 11/11 artifacts прошли SHA-256/length, PowerShell parse errors — 0,
  размер 61.46 MiB, ZIP — 0, `1\shared` до запуска отсутствует. BUILD-INFO
  фиксирует `field_route_policy=auto` и
  `field_relay_url=https://aps1-1.relay.n0.iroh.link./`.
- Clean3 run `20260919-144857` завершён успешно и независимо перепроверен
  workspace verifier-ом по синхронизированному evidence. Итог:
  `result=verified`, activation=`providers-before-capability`, convergence=
  `recipient-applied-owner-acknowledged`, sender resolution/receipts=`2/2`,
  recipient resolution/commit-delete=`2/2`, Alice offline before receive,
  legacy fallback=false, restart redelivery=absent, history occurrence=1.
  `alice-sent.marker`, `bob-complete.marker` и `providers-stopped.marker`
  присутствуют. Все retained phases показывают `route_policy=auto` и
  `relay_home_url=https://aps1-1.relay.n0.iroh.link./`. Это закрывает clean
  external M0.9.69 evidence; HTTPS compatibility retirement остаётся отдельным
  следующим архитектурным этапом.
- M0.9.70 проверен только verification/debug toolchain с ограничением
  `--jobs 2`; release/ZIP/network launch не выполнялись. Targeted automatic
  legacy-upgrade test прошёл, затем полные затронутые suites дали CLI 76/76,
  runtime IPC 15/15 и Windows 22/22. Clippy `-D warnings` для трёх пакетов,
  `cargo fmt --check`, `git diff --check` и static gates legacy-upgrade,
  desktop-control IPC v25, provider-selection, runtime-mailbox и M1 acceptance
  прошли.

## M0.9.71 verification snapshot (2026-09-19)

- Exact authenticated outbound delivery теперь выполняет volunteer replication
  до создания `MailboxHttpClient`. Suppression HTTPS фиксируется только после
  durable `ReplicatedOutboundCommit`; ошибка проверки или локального commit
  сохраняет pending request и переходит к compatibility HTTP.
- Новый replicated record хранится во прежней stored-outbound таблице с
  однозначным `0xffKRP1` prefix; старый unprefixed HTTPS receipt decoder не
  изменён. Insert replicated record и remove pending выполняются одной Redb
  Immediate transaction, поэтому restart не создаёт ambiguous queue state.
- Exact threshold считает только receipts, чьи store keys входят в durable
  locator. Transport identities остаются distinct; legacy/random receipt после
  rotation не может сам увеличить exact count.
- Mailbox-client regression прошёл 14/14, полный CLI regression — 77/77.
  Clippy `-D warnings` для `kilogram-mailbox-client` и `kilogram-cli`, новый
  HTTPS-retirement gate, legacy-upgrade gate и M1 acceptance-kit gate прошли.
- Первый CLI-прогон с неограниченным числом test-harness threads один раз
  поймал Windows race `os error 183` при параллельном создании backup ratchet
  directory. Тот же тест отдельно прошёл, а полный повтор с
  `-- --test-threads=2` прошёл 77/77. Поэтому на Windows ограничивать нужно не
  только Cargo `--jobs 2`, но и число одновременно исполняемых Rust-тестов.
- Все проверки выполнялись debug/dev toolchain с `--jobs 2`; release/ZIP и
  network listener не запускались. Реальный clean external no-HTTPS run остаётся
  следующим M0.9.72 evidence этапом.

## M0.9.72 verification snapshot (2026-09-19)

- M0.9.69 orchestration templates сделаны milestone-aware без дублирования
  provider/Bob logic: старый kit по-прежнему требует ticket-store, новый
  M0.9.72 fail closed запрещает присутствие этого EXE.
- No-HTTPS mode использует inert RFC 8032 store key и unreachable
  `http://127.0.0.1:18787`, проверяет отсутствие binary/listener до identity
  setup, непосредственно перед send и после остановки Alice.
- Новый verifier наследует полный exact-locator evidence contract и дополнительно
  требует suppression, `http_put=not-attempted` и exact volunteer durability;
  self-test отклоняет attempted PUT и reachable endpoint.
- PowerShell AST parse всех изменённых/new scripts — без ошибок. Старый M0.9.69
  boundary/self-test, новый M0.9.72 boundary/self-test, HTTPS-retirement gate и
  M1 acceptance-kit boundary проходят.
- Первый clean-HEAD generator preflight обнаружил устаревшее утверждение
  M0.9.65 replication gate о безусловной HTTPS compatibility copy. Gate приведён
  к M0.9.71: exact `2/2` требует no-PUT, а legacy/incomplete/reverse-ACK всё ещё
  требуют compatibility path.
- Из clean revision `36a4a6c76a02d9f8d6ff70adc5a259951b8b2885` создан
  `.tmp\m0972-no-https-36a4a6c`: debug, Cargo jobs=2, 11/11 payload artifacts
  совпадают по SHA-256/length, PowerShell parse errors=0, 13 файлов/57,690,277
  bytes, ZIP=0, ticket-store EXE отсутствует, `1\shared` до запуска отсутствует.
  Generator network execution=false.
- Первый external run `20260919-170816` не дошёл до send. Alice и Bob были
  online на pinned `aps1`; Bob разрешил exact providers `2/2` и соединялся с
  ними direct, но взаимные runtime automatic-sync/capability sessions попадали
  в повторяющиеся 30-second outbound и incoming Initial/handshake timeout.
  Provider publication, Yandex synchronization и порядок шагов корректны.
- Runtime event loop сохраняет accept future между ticks, но сам tick handler
  последовательно await-ит capability push, delivery, mailbox poll/gossip и
  automatic sync. Пока выполняется один такой outbound await, accept future не
  poll-ится; одинаковый startup schedule двух peers может создать symmetric
  lockstep. Это рабочая гипотеза для M0.9.73 и должна быть закреплена локальным
  deterministic regression до следующего внешнего запуска.

## M0.9.73 verification snapshot (2026-09-19)

- Детерминированный mutual-auto-sync regression сначала воспроизвёл прежний
  symmetric timeout примерно за 90 секунд: два runtime одновременно начали
  outbound sync и не обслужили входящие application sessions.
- Runtime actor теперь запускает не более одной owned outbound-cycle task и во
  время её network waits продолжает poll постоянного Iroh accept. IPC mutation
  принимается между cycles; существующий state-directory lock сериализует
  background mutation и accepted application session.
- Reciprocal automatic sync выбирает одного initiator детерминированно по
  Account ID, а для устройств одного аккаунта — по Device ID. Passive peer
  остаётся доступным для elected inbound session; wire format и authority не
  менялись.
- Новый mutual-runtime regression и прежний outbox/sync regression проходят;
  полный `kilogram-cli` suite: 79 passed, 0 failed, 291.41s, `--jobs 2` и
  `--test-threads=2`.
- Контракт описан в RFC-0096 и закреплён статическим fail-closed verifier,
  включённым в M1 acceptance-kit builder/boundary. Fresh двухсетевой no-HTTPS
  field run ещё обязателен: локальный green не превращает run
  `20260919-170816` в принятое evidence.
- Existing exact-locator generator расширен отдельным `M0.9.73` mode без
  копирования orchestration: wrapper, `m0973` conversation/message label,
  `%LOCALAPPDATA%\Kilogram\M0973` private root и milestone-aware no-HTTPS
  evidence verifier. Mismatch `M0.9.72`/`M0.9.73` отклоняется self-test.
- Новый `verify-kilogram-m0973-no-https-kit-boundary.ps1` наследует весь
  M0.9.72 contract и дополнительно требует cooperative scheduling. M1
  acceptance-kit включает эту границу; профиль остаётся debug, Cargo jobs=2,
  ZIP/release/new EXE/network launch отсутствуют.
- Из clean revision `fe7f46df25c7389291dfb52d06d897d933b7ee15` создан
  `.tmp\m0973-no-https-fe7f46df25c7` и byte-identical копия
  `C:\Users\Paul\YandexDisk\!M\M0.9.73`. Проверено: 11/11 manifest artifacts,
  0 PowerShell parse errors, 13 файлов/57,741,367 bytes, ZIP=0,
  `kilogram-ticket-store.exe`=0, precreated `1\shared`=false. `BOUNDARIES.log`
  содержит M0.9.72 inherited no-HTTPS, cooperative scheduling и M0.9.73 gates;
  generator network execution=false.

## M0.9.74 verification snapshot (2026-09-19)

- M0.9.73 external run `20260919-192354` дошёл дальше прежнего: обе peer
  стороны завершили convergence, Alice получила exact volunteer receipts `2/2`,
  записала `runtime_mailbox_delivery_durability=exact-volunteer-replication`,
  `runtime_mailbox_http_put=not-attempted` и остановилась. Это подтверждает
  исправление cooperative scheduling, но не завершает field acceptance.
- Bob не выполнил retrieval: runtime повторял
  `open mailbox replication ledger read-only: Database repair aborted` и не
  зарегистрировал inbound replica. Причина — unclean runtime stop оставил Redb
  recovery marker; прежний read-only inspection не мог выполнить recovery.
- Новый helper сохраняет read-only fast path. Только typed `RepairAborted`
  выполняет один writable open под state lock и `VaultDualWriteGuard`, затем
  повторяет read-only inspection. Все другие ошибки остаются fail closed.
- Regression создаёт настоящий interrupted Redb через child `process::exit(73)`;
  targeted test и полный CLI suite прошли. Итог полного прогона: 81 passed,
  0 failed, 299.75s, Cargo jobs=2, test threads=2. Strict clippy также clean.
- RFC-0097 и новый static gate включены в M1 acceptance builder/boundary.
  Release/ZIP/network listener не запускались; VPN не влияет на эти локальные
  проверки. Следующая внешняя попытка обязана использовать fresh M0.9.74 kit.
- Generator/common/Alice/verifiers расширены строгими
  `M0.9.74`/`m0974`/`M0974` branches. Новый M0.9.74 boundary наследует M0.9.73,
  требует recovery gate, debug/two jobs/six launches/no ZIP/no release/no
  generator network. Все M0.9.72/73/74 gates, verifier self-tests, M1 boundary
  и PowerShell AST parse проходят.
- Clean harness revision `aa295dfd183cc4edaf1347f0555916285a9a9cb0`
  создан отдельным коммитом. Генератор с below-normal Cargo priority/jobs=2
  собрал `.tmp\m0974-no-https-aa295dfd183c` за 24.50s; рабочее дерево осталось
  clean. Проверка комплекта: 11/11 manifest artifacts, 13 files/57,748,745
  bytes, PowerShell parse errors=0, ZIP=0, ticket-store EXE=0, precreated
  `1\shared`=false, profile=debug, archive/network execution=false.
- Комплект без перезаписи скопирован в
  `C:\Users\Paul\YandexDisk\!M\M0.9.74`; SHA-256 всех 13 source/copy файлов
  совпал. Перед запуском на втором хосте нужно дождаться полной синхронизации
  новой папки; M0.9.73 state/evidence не переносить.
- External run `20260919-203722` завершён и независимо проверен kit verifier-ом:
  `result=verified`, `build_commit=aa295dfd183cc4edaf1347f0555916285a9a9cb0`,
  conversation=`m0974-20260919-203722`. Sender: exact resolve/receipts `2/2`,
  exact volunteer durability, no HTTP PUT. Recipient: два distinct store keys,
  два `volunteer-iroh`, два `deleted-after-commit`; restart без inbound
  redelivery, history `event_count=2` и verifier подтвердил один message marker.
  Все completion/provider-stop markers присутствуют. Fresh state не потребовал
  runtime repair marker; deterministic crash test остаётся прямым recovery
  proof, field run — полным lifecycle proof.

## M0.9.75 verification snapshot (2026-09-19)

- Новый exact-volunteer provisioning wire v2 не содержит
  `MailboxServiceDescriptor`, HTTPS URL или central store key; v2 binding/offer
  используют отдельные signature, HPKE и content-ID domains.
- Ordered capability update получил append-only action `ActivateExactVolunteer`;
  v1 decode/read сохранён, `v1 -> v2` migration разрешена, `v2 -> v1`
  fail-closed запрещена, v2 revocation сохраняет версию chain.
- Automatic upgrade теперь переводит любой acknowledged active v1 head в
  service-free v2 при наличии двух transport-distinct providers. Targeted
  regression подтверждает отсутствие service descriptor после persisted
  rotation и restart-idempotence.
- Runtime upload/poll и reverse ACK для v2 используют exact provider locator;
  неполная репликация остаётся pending и не вызывает HTTPS fallback.
- Runtime IPC v26 и Windows desktop используют exact create/rotate без URL/key;
  status явно показывает `v2-exact-volunteer` или `v1-legacy-https`.
- Новый static gate включён в M1 acceptance builder/boundary. Проверки этапа
  выполняются только debug/dev toolchain с Cargo jobs=2; release/ZIP/network
  listener не запускаются.
- Mailbox provisioning 8/8, runtime IPC 15/15 и Windows 22/22 прошли. Первый
  полный CLI run выявил старый test-only race: multi-endpoint regression открыл
  direct state transaction рядом с работающим actor без state lock. Regression
  переведён на штатный `acquire_runtime_state_lock`; отдельный повтор прошёл,
  затем полный CLI suite завершился 81/81 за 313.65s с `--jobs 2` и
  `--test-threads=2`.
- Strict Clippy для provisioning/runtime-IPC/CLI/Windows прошёл с
  `-D warnings`; все mailbox compatibility/lifecycle/runtime/v2 static gates,
  M1 acceptance boundary, `cargo fmt --check` и `git diff --check` зелёные.

## M0.9.76 verification snapshot (2026-09-19)

- Новый mode переиспользует существующий three-folder/six-launch harness и
  добавляет только `M0.9.76`/`m0976`/`M0976` branches, отдельный wrapper и
  service-free evidence verifier.
- Bob activation использует `runtime-mailbox-exact-offer-create` без
  `--service-base-url`/`--store-key`; public coordination, manifest и build info
  также не сериализуют compatibility endpoint.
- Успешный v2 exact commit печатает
  `runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer`;
  legacy v1 exact path сохраняет прежнюю метку `suppressed`.
- M0.9.72, M0.9.73, M0.9.74, M0.9.76, service-free v2 и M1 acceptance static
  gates прошли. Negative verifier self-test отверг central URL и legacy
  suppressed-copy marker.
- `cargo fmt --check`, targeted CLI unit test и strict CLI Clippy прошли при
  `--jobs 2`; release/ZIP и сетевые процессы не запускались.
- Fresh debug kit должен генерироваться только из clean committed revision;
  внешний two-network lifecycle пока не выполнялся.
- После обновления устаревшего provider-selection static gate с IPC v25 на
  фактический v26 clean revision `c39f5ce9ec6effd49cf355036119e28d830c9d28`
  успешно создал `.tmp\m0976-service-free-v2-c39f5ce9ec6e`.
- Generated kit verification: 11/11 SHA-256+length manifest artifacts, 9/9
  PowerShell parse, bundled negative evidence self-test green, 13 files,
  57,863,721 bytes, ZIP=0, ticket-store EXE=0, compatibility endpoint property
  absent, profile=debug, Cargo jobs=2, generator network execution=false.
- Первый внешний run остановился в `02_SEND_ALICE.ps1` до runtime/queue/message:
  service-free v2 попал в общий `else`, предназначенный для запуска legacy
  HTTPS fixture. `04-send`, queue и offline evidence не появились; локальные
  provider процессы после ошибки остановлены.
- Исправленный send запускает store только под явным
  `if (-not $noHttpsCompatibility)`. M0.9.72 gate дополнен AST-проверкой
  structural ownership store-start command; inherited M0.9.72/73/74, M0.9.76
  и M1 boundaries проходят. Нужен новый clean committed retry kit.
- Revision `ff38e1b89dc5832abdb2a5d81f7ab3af06e0ceae` создал исправленный
  `.tmp\m0976-service-free-v2-ff38e1b89dc5`: 11/11 hashes, ровно один
  store-start под exact AST guard, bundled verifier green, 13 files и
  57,863,913 bytes. Byte-identical external copy:
  `C:\Users\Paul\YandexDisk\!M\M0.9.76-retry1`.
- External retry run `20260920-001614` прошёл bundled evidence verifier:
  `result=verified`, build commit `ff38e1b89dc5832abdb2a5d81f7ab3af06e0ceae`,
  v2 descriptor/compatibility endpoint absent, exact resolution/receipts 2/2,
  HTTP PUT not attempted, Alice offline, Bob volunteer-Iroh commit/delete 2/2,
  restart inbound count 0, history event count 2 и marker occurrence 1.

## M0.9.77 verification snapshot (2026-09-20)

- Pinned Rust: `rustc 1.98.0`, target `x86_64-pc-windows-msvc`; bundled
  `rust-lld.exe` расположен в target-specific sysroot `bin`.
- Local bundled linker identity на этой машине: SHA-256
  `f436ddda2f519c5ad18b7c7b09a3eadc2d4fc1be88cd9280cc472740fc3b7389`,
  113,421,312 bytes.
- Stable explicit flags `-C linker=<rust-lld.exe> -C linker-flavor=lld-link
  -C link-arg=/Brepro` успешно собрали и запустили tiny smoke и настоящий debug
  `kilogram-offline.exe`; `dumpbin /headers` показывает linker version 14.00.
- Reverse build-dependency tree offline target содержит `cc` только через
  BLAKE3. Direct `kilogram-offline` dependency включает официальный feature
  `pure`; resolved graph подтверждает `blake3 feature "pure"`, поэтому C/ASM
  implementations не попадают в итоговый artifact.
- `-C linker-features=+lld` в Rust 1.98.0 всё ещё unstable и поэтому явно не
  используется; static boundary запрещает его появление.
- Первый clean release pair с LLD имел одинаковые 2,433,536 bytes и отличался
  только 20 байтами: COFF timestamp, два debug timestamps и первые восемь байт
  CodeView GUID. Parser-bounded normalization v1 обнуляет все эти timestamp
  fields и полный 16-byte GUID; retained copies после неё имеют одинаковый
  SHA-256 `25b24880f3f7f8ee34da48b4239b0dd730f5799e22275ef508640747cb9b6540`.
- Format-v3 local/external records фиксируют exact linker identity и exact
  PE-normalization policy; independent
  verifier self-test принял coherent evidence и отверг mismatched linker hash и
  tampered artifact.
- `verify-kilogram-independent-builder-boundary.ps1`, PowerShell parse всех
  изменённых scripts и real debug Cargo build прошли. Сборка использовала
  BelowNormal и `--jobs 2`; ZIP и network process не создавались. Первый
  release pair сохранён как bounded divergent diagnostic fixture; clean
  format-v3 pair для commit
  `788a3c4ff0152c78a43e8875315a93eba51925a8` затем завершился успешно:
  artifact SHA-256
  `25b24880f3f7f8ee34da48b4239b0dd730f5799e22275ef508640747cb9b6540`,
  2,433,536 bytes; source-manifest SHA-256
  `d3ccdbfb56741f1d5eed74ad2856fea4ccc1b5b124d46aaf13b0a11a3147c46b`.
  Record повторно verified, offline EXE `--help` запускается, temporary
  source/target roots удалены; evidence directory:
  `.tmp/repro/m0977-788a3c4-v3`.

## M0.9.78 verification snapshot (2026-09-20)

- GitHub run `35469391445` для `4711dd337dce4ba82c19d50970dc29f75b6bd260`
  прошёл source/toolchain/offline build с exact local LLD hash, но fail closed:
  external normalized artifact SHA-256
  `3dd8e4b78e4326663393c126a3a1533dc1fdd7f9a5dcf45751e80034b96253ff`,
  2,434,560 bytes против local
  `25b24880f3f7f8ee34da48b4239b0dd730f5799e22275ef508640747cb9b6540`,
  2,433,536 bytes. `.rdata` и следующие sections сдвинуты; это не metadata-only
  difference, attestations ожидаемо не созданы.
- Real `cargo rustc --bin kilogram-offline` с LLD `/reproduce` создал
  157,723,136-byte/175-entry diagnostic TAR. В нём ровно 10 `.lib`: MSVC
  `14.44.35207` `msvcrt`/`vcruntime`, Windows SDK `10.0.28000.0` UCRT и семь
  UM import libraries.
- Новый helper классифицирует только Windows SDK/MSVC x64 paths, хэширует exact
  bytes, пишет sorted path-independent `NATIVE-LINK-INPUTS.sha256`, требует
  UM+UCRT+MSVC coverage и отвергает unsafe/unclassified/duplicate/empty inputs.
- Local manifest SHA-256
  `182c33c504ac2bce811459acd1a9f3fcd35fcb414be1b710b32651c9c794d61c`,
  count=10. Probe TAR/extraction root удалены; normalized artifact не изменился.
- Format-v4 local/GitHub records связывают manifest hash/count/format и
  `archive_retained=false`; same-host roots требуют identical manifests,
  external verifier требует local/GitHub equality и attests manifest вместе с
  EXE/record только при exact artifact match.
- Static independent-builder boundary и network-free verifier self-test прошли;
  self-test отдельно отверг `mismatched_native_link_inputs`.
- Dirty development two-clean-root run
  `.tmp/repro/m0978-dev-native-inputs-v4` прошёл с Cargo jobs=2/BelowNormal:
  оба EXE имеют прежний SHA-256
  `25b24880f3f7f8ee34da48b4239b0dd730f5799e22275ef508640747cb9b6540`,
  2,433,536 bytes, а оба native manifests — exact SHA-256 выше/count=10.
  `source-a/b`, `target-a/b` и оба TAR удалены; retained TAR count=0.

## M0.9.79 verification snapshot (2026-09-20)

- Commit `c92d25fce192be1ed5094098581ccb082ffa7a53` push-нут в `origin/master`;
  manual workflow run `35471719378` завершился ожидаемым fail-closed после
  успешной independent compile и сохранил bounded evidence artifact.
- External record: `windows-2025`/`win25-vs2026`, MSVC `14.51.36231`, Windows
  SDK `10.0.26100.0`, native manifest SHA-256
  `08807e748ddb05aee3278621b17b9024f5278b2de754b2a0868ab86c54f97fbf`;
  EXE SHA-256
  `3dd8e4b78e4326663393c126a3a1533dc1fdd7f9a5dcf45751e80034b96253ff`,
  2,434,560 bytes.
- Local exact M0.9.78 record использовал MSVC `14.44.35207`, SDK
  `10.0.28000.0`, native manifest SHA-256
  `182c33c504ac2bce811459acd1a9f3fcd35fcb414be1b710b32651c9c794d61c`;
  одинаковым между manifests был только exact `ucrt.lib` payload.
- Для M0.9.79 выбран installed/non-vendored set MSVC `14.44.35207` + Windows
  SDK `10.0.19041.0`. `WINDOWS-NATIVE-LINK-INPUTS.lock` содержит exact ten
  hashes/lengths, canonical identity
  `e478c6daf61551607f8502c7ef97537403f4da24ddbab0b7a6a8fcb94d835027`.
- Probe с одним `LIB` всё ещё выбрал SDK 28000. Probe с explicit final
  `rustc -L native=<MSVC> -L native=<UCRT> -L native=<UM>` выбрал exact SDK
  19041 manifest; обе проверки использовали BelowNormal/jobs=2, каждая заняла
  около двух секунд благодаря существующему cache, transient TAR удалён.
- Format-v5 local/external records связывают repository lock и observed
  manifest. Static boundary, parse шести PowerShell scripts и independent
  verifier self-test прошли; negative cases включают mismatched lock, tampered
  lock file, mismatched observed inputs, malformed manifest, mismatched LLD и
  PE tamper.
- Dirty development two-clean-root record `.tmp/repro/m0979-dev-v5` verified:
  оба normalized EXE byte-identical, 2,433,536 bytes, SHA-256
  `3e5419e14fadeca66068ffdccda7a4cde7584cae03e0430b745e2c3ca700e878`;
  observed manifest exact lock SHA-256
  `e478c6daf61551607f8502c7ef97537403f4da24ddbab0b7a6a8fcb94d835027`,
  count=10. Source/target roots и reproduction TAR удалены.
- Workflow переведён на `windows-2022`, но exact hashes остаются обязательными;
  официальный current runner-image inventory перечисляет SDK 10.0.19041.0 и
  VC x86/x64 tools, но не доказывает exact file payloads. Наличие pinned set и
  cross-host byte equality ещё требуют fresh committed external run.

## M0.9.80 verification snapshot (2026-09-20)

- Exact M0.9.79 commit `a7c5fe0ab7372958959985b54fc2b813c5c4f620`
  push-нут; manual workflow run `35473329174` прошёл Rust/LLD/native-toolchain
  gates, но корректно fail closed на final SHA-256 mismatch без attestation.
- Native boundary действительно совпал: lock/observed SHA-256
  `e478c6daf61551607f8502c7ef97537403f4da24ddbab0b7a6a8fcb94d835027`,
  MSVC `14.44.35207`, Windows SDK `10.0.19041.0`, count=10.
- Local artifact: 2,433,536 bytes,
  `3e5419e14fadeca66068ffdccda7a4cde7584cae03e0430b745e2c3ca700e878`;
  GitHub artifact: 2,434,560 bytes,
  `bea3fd37a9190f33e30fe31c3ea1c92416d47367cb739b7c5c6ecaf65c85bd78`.
- `rg -a` по retained executables показал exact cause: local embedded path
  `C:\Users\Paul\.cargo\registry\src`, hosted path
  `C:\Users\runneradmin\.cargo\registry\src`. Source-root remap не покрывал
  registry dependencies вне checkout.
- M0.9.80 добавляет canonical `Z:/cargo-registry-src`, explicit resolved-root
  validation и PE leak check. Records/verifiers подняты до format v6.
- Static boundary и network-free independent verifier self-test проходят;
  self-test отдельно отвергает raw hosted-user Cargo registry path.
- На момент local snapshot clean exact-commit two-root build и fresh GitHub
  dispatch ещё предстояли; принятый результат записан ниже.

### M0.9.80 accepted external result (2026-09-20)

- Clean committed revision `4e9054ab2ccc6a4c542fb37d486b70e53027dd08`
  independently reproduced locally and on GitHub.
- Local format-v6 pair and GitHub run `35474774356` produced the same normalized
  `kilogram-offline.exe`: 2,432,000 bytes, SHA-256
  `d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`.
- GitHub run succeeded in 3m49s, created artifact attestation `48691093`, and
  retained one bounded artifact. No artifact ZIP was downloaded during the
  accepted run.
- Repository branch `master` and `origin/master` both point to the accepted
  revision. The next bounded stage is an M1 candidate record that composes the
  accepted M0.9.76 network evidence with this supply-chain evidence.

## M0.9.81 accepted candidate verification (2026-09-20)

- Added `M1-CANDIDATE.json`, production/static verifiers and RFC-0104.
- Accepted field baseline surface: 83 Git entries, SHA-256
  `a67580165a166e22d6c52b52e93d29c562f1bb6289e2497e220fe3ff1e788a3d`.
- Accepted independent-artifact surface: 63 Git entries, SHA-256
  `4ada929ce28d5e152923f746d339d168c5620767f8270443e2a63baf4553e294`.
- PowerShell parse, JSON parse, verifier negative self-test and static boundary
  pass. Self-test rejects field revision/hash/path/risk weakening.
- Candidate commit `485857ec29c838ddcea7cc61fc74ecae629d2d84` passed the
  production verifier: both evidence revisions are ancestors,
  `runtime_protocol_surface=unchanged`,
  `independent_artifact_surface=unchanged` and
  `m1_candidate_status=verified`.
- No network process, Cargo build, release target, ZIP, service or Git tag was
  created during implementation or acceptance.

## M0.9.82 local admission-work verification (2026-09-20)

- New embedded volunteer offers use 18-bit domain-separated admission work;
  generation permits 1..20 bits and is bounded by `2^24` attempts.
- New exact capability selection, automatic legacy upgrade, diagnostic
  selection and authenticated gossip exclude low-work offers. Existing exact
  store-key lookup remains compatible with retained low-work offers.
- Authenticated IPC remains v26; no executable target was added.
- Debug validation used at most two Cargo jobs: 30 mailbox/client/store tests,
  three focused runtime tests and Clippy with `-D warnings` passed. All 11
  mailbox/provider/service-free static boundaries passed together. Initial
  measured generation remained below one second for a two-offer qualified
  fixture.
- No release build, ZIP, network field process or background service was used.

## M0.9.83 local provider-observation verification (2026-09-20)

- Added a bounded local-only authenticated observation table with at most eight
  pseudonymous peer tags per exact signed provider offer.
- Tags use a BLAKE3 subkey derived from the local Device secret and keyed input
  from the already authenticated peer Account/Device identity.
- Wire and IPC remain unchanged; observation tags/social IDs are not
  transmitted or logged, and provider selection does not yet consume counts.
- Unit regressions cover deduplication, capacity, replacement reset,
  persistence, legacy read compatibility and observation-independent gossip
  bytes. Runtime regression covers deterministic local tag derivation and
  authenticated inbound/reply-bound import integration.
- Accepted locally after 16/16 mailbox-client tests, 3/3 focused CLI runtime
  tests, all 12 mailbox/provider/service-free fail-closed scripts, formatting
  and Clippy with warnings denied passed under Cargo `-j 2`.
- No release build, ZIP, network field process, background service, external
  publication or executable target was created.

## M0.9.84 bootstrap-safe provider-selection verification (2026-09-20)

- Added binary observed-first selection after the existing 18-bit admission
  gate; two through eight observer tags provide no additional rank.
- Admission-qualified unobserved offers fill every missing slot, retaining a
  fresh-install bootstrap path while exact transport identities stay distinct.
- Replacement/expiry clears the exact-offer preference; committed replica-set
  lookup remains observation-independent and IPC remains v26 without local
  provenance fields.
- Debug validation passed 17/17 mailbox-client tests, 7/7 mailbox tests and 3/3
  focused CLI runtime tests under Cargo `-j 2`.
- All 13 mailbox/provider/service-free fail-closed scripts, formatting and
  Clippy with warnings denied passed together.
- No release build, ZIP, network field process, background service, external
  publication or executable target was created.

## M0.9.85 local provider path-domain provenance verification (2026-09-20)

- `kilogram-transport-iroh` now canonicalizes selected direct paths to the
  exact remote IP without port and relay paths to URL origin, then exposes a
  caller-keyed redacted local-tag derivation alongside existing diagnostics.
- `kilogram-mailbox-client` persists one exact-offer-bound latest tag plus
  derivation epoch and kind in a separate redb table; replacement/expiry clears
  it and legacy missing-table reads remain valid.
- Runtime captures the selected path after authenticated peer response and
  persists it only after typed PUT/LIST/DELETE verification. Evidence failure
  cannot discard a verified receipt/application commit.
- Raw IP, relay URL, tag and epoch are absent from provider offer/gossip,
  protocol and IPC v26. Selection is intentionally unchanged.
- RFC-0108 and
  `scripts/verify-kilogram-provider-path-domain-boundary.ps1` define the
  fail-closed privacy/ordering boundary.
- Debug-only acceptance passed 18/18 mailbox-client tests, 6/6 transport tests,
  7/7 mailbox tests, 2/2 focused CLI runtime tests, all 16
  mailbox/provider/service-free fail-closed boundaries, formatting and Clippy
  with warnings denied under Cargo `-j 2`.
- No release build, ZIP, new EXE, background process, network field run or
  external publication was created.

## M0.9.86 positive provider path co-location avoidance verification (2026-09-20)

- `select_bootstrap_safe_active_offers` retains admission work, binary local
  corroboration, rendezvous order and transport deduplication, but defers a
  candidate positively known to share a selected path domain.
- Path-unknown candidates remain eligible in the first pass. A second pass
  restores deferred co-located candidates in ranked order when alternatives
  cannot fill the request; unequal tags receive no independence weight.
- New exact provisioning, automatic legacy upgrade and diagnostic selection
  consume the policy. Existing exact commitments, gossip/protocol wire and IPC
  v26 remain unchanged.
- RFC-0109 and
  `scripts/verify-kilogram-provider-colocation-avoidance-boundary.ps1` define
  the fail-closed preference/availability/privacy boundary.
- Debug-only acceptance passed 19/19 mailbox-client tests, 6/6 transport tests,
  7/7 mailbox tests, 2/2 focused CLI runtime tests, all 17
  mailbox/provider/service-free fail-closed boundaries, formatting and Clippy
  with warnings denied under Cargo `-j 2`.
- No release build, ZIP, new EXE, background process, network field run or
  external publication was created.

## M0.9.87 independent-provider field-contract verification (2026-09-20)

- Added a four-folder Windows kit generator: Alice, Provider 1, Provider 2 and
  Bob. Minimum physical topology is three hosts; four are ideal.
- The provider pair must have distinct run-scoped machine, operator-claim and
  network-claim SHA-256 values. Raw MachineGuid and entered labels are not
  serialized; exact evidence schema rejects added raw fields.
- Provider private state and open logs remain in `%LOCALAPPDATA%`. Shared offer
  and attestation files are atomic; final provider logs are copied only after
  the runtime closes them. Both providers wait for the peer stop marker.
- The evidence verifier inherits the M0.9.69 exact-locator and M0.9.76
  service-free lifecycle, and rejects same-machine/operator/network fixtures.
- Stage acceptance is PowerShell/static and network-free because Rust runtime
  code did not change. External three-/four-host evidence remains pending.
- No release build, ZIP, HTTPS mailbox process, new EXE, background service,
  field connection or external publication was created.

## Удалённый Windows test node (2026-10-14)

- Выделенный ноутбук Windows 11 доступен только в текущей LAN как
  `test@192.168.0.152`; hostname `PAUL_HP_OMEN`, профиль
  `C:\Users\test`, shell Windows PowerShell 5.1. Адрес DHCP и поэтому перед
  будущим прогоном его следует считать проверяемым, а не постоянным.
- `test` — обычный локальный пользователь, состоящий только в группе
  `BUILTIN\Users`. У него нет доступа к управлению `sshd`, системной CIM
  информации и server host private keys; это желаемая least-privilege граница.
- Рабочая беспарольная авторизация использует отдельный Ed25519 key
  `C:\Users\Paul\.ssh\kilogram-test-laptop-final-ed25519`. На ноутбуке в
  `C:\Users\test\.ssh\authorized_keys` оставлена только его публичная часть
  с comment `kilogram-test-laptop-final-2026-10-14`; пароль и private key в
  проектной памяти не записываются.
- Проверка выполнялась с `BatchMode=yes`, `IdentitiesOnly=yes`, pinned host key
  из ignored файла `.tmp\remote-access\known_hosts` и завершилась как
  `key_auth_ok`. Старый интерактивный парольный session закрыт, два пробных
  неработающих ключа удалены локально и из `authorized_keys`.
- Remote workspace `C:\Users\test\KilogramLab` создан. SCP transfer
  `docs\M0.9.87-TWO-HOST-REDUCED-TEST-RU.md` в `access-test.md` проверен
  одинаковым SHA-256
  `3AD88947A1E5E19FBAE62EA0882CD5A2759F0B234FEF023579EE3549D1FC6D94`.
- На test node не обнаружены `cargo`, `git`, `syncthing` и PowerShell 7; для
  текущих Windows field kits достаточно SCP-доставки готовых EXE/scripts и
  удалённого запуска через встроенный Windows PowerShell. Устанавливать
  toolchain следует только при отдельной необходимости.
- Обычный процесс, запущенный непосредственно из OpenSSH network logon, не
  может вызвать current-user DPAPI (`CryptProtectData`, Windows error 5).
  Для field roles удалённый controller создаёт новый logon того же test user
  через `Start-Process -Credential -LoadUserProfile`; проверочный
  `account-create` после этого сообщил
  `root_key_protection=windows-dpapi-current-user`. Пароль не сохранять в
  workspace/logs/проектной памяти.
- При активном `happ-xray` обычные локальные SSH probes могут перехватываться
  виртуальным интерфейсом. SSH/SCP controller должен использовать
  `BindAddress=192.168.0.140` (актуальный физический адрес сначала проверять),
  а доступность ноутбука — подтверждаться реальным key-auth и hostname, не
  широким TCP scan. Закрытая крышка усыпляет ноутбук и делает SSH недоступным.
- Учётная запись и ключ предназначены только для временных Kilogram tests.
  После завершения field testing удалить пользователя `test`, его профиль и
  локальный private key; до этого не использовать эту учётную запись для
  личных данных.

## M0.9.87 two-host run firewall diagnostic (2026-10-14)

- Fresh reduced run `20261014-072429` из revision
  `fce270c1ad23d39b00da6affbf1175207e1ef03d` подтвердил Alice/Bob direct LAN
  path, но получил только Provider 1 receipt. Provider 2 на ноутбуке видел
  incoming attempt, после чего server side завершал handshake timeout, а Alice
  получала Iroh TLS error `no server certificate chain resolved`.
- Повтор exact locator после первого частичного PUT теперь действительно
  idempotent: `recorded_at_unix_seconds` не участвует в immutable locator replay
  equality, при этом signed replica-set fields и cryptographic commitments
  продолжают проверяться exact. Mailbox-client regression: 19/19.
- Read-only `netsh` audit обнаружил 30 старых allow blocks для
  `kilogram-cli.exe`, но ни одного для текущего exact path
  `C:\Users\test\KilogramLab\field-kit-current\kilogram-cli.exe`. Windows
  Firewall rules path-specific; все profiles enabled. Поэтому firewall остаётся
  правдоподобной причиной именно Provider 2 failure, несмотря на успешный
  Alice/Bob hole-punched direct path.
- Least-privilege user `test` не может добавить правило: `netsh` корректно
  вернул elevation required. Следующий diagnostic обязан сначала создать от
  администратора узкое временное inbound UDP allow rule для текущего EXE и
  Alice physical IP `192.168.0.140`, затем провести fresh run и после проверки
  удалить правило. Run `20261014-072429` отклонён и не возобновляется.

## M0.9.87 two-host run success (2026-10-14)

- Администраторский firewall rule для exact path
  `C:\Users\test\KilogramLab\field-kit-current\kilogram-cli.exe`, inbound UDP,
  all profiles, remote `192.168.0.140/32` устранил Provider 2 handshake failure.
- Fresh run `20261014-075145` завершил Alice prepare/send, Bob
  prepare/receive/delete/restart и остановку обоих providers. Основной результат
  — `verified-reduced-two-host`; это не strict independent-provider acceptance.
- Windows PowerShell 5.1 `-File` не передаёт `string[]` из controller helper как
  два надёжных positional arguments: `paul` и `home-lan` стали одной строкой
  `paul,home-lan`, а Provider 2 ждал `Read-Host`. Для automation использовать
  remote wrapper с именованными `-OperatorLabel` и `-NetworkLabel`.
- Codex sandbox не должен владеть shared field directories одновременно с
  elevated SSH bridge; локальные field roles запускаются outside sandbox, а
  директории создаются через idempotent `Directory.CreateDirectory`.
- Firewall rule является временным тестовым исключением. После завершения
  сетевых прогонов его нужно удалить от администратора вместе с test account и
  SSH key; обычный пользователь `test` этого сделать не может.
- Clean rerun `20261014-080905` из commit `162836b` прошёл встроенный
  `03_VERIFY` со статусом `verified-reduced-two-host`. Он является текущим
  canonical двухмашинным field result; первый successful run `075145` полезен
  как regression fixture verifier false negative, но не нужен для повторного
  запуска.

## M0.9.88 independent-verifier Cargo identity diagnostic (2026-10-14)

- Same-host clean-root pair для revision
  `bb4b7a08184301c2f6b56eed658fdae483cd8f55` совпал: 2,432,000 bytes,
  SHA-256 `d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`.
- GitHub run `38506048297` повторил exact artifact и создал attestation
  `55680587`; bounded artifact ID `11868131068` скачан распакованным в ignored
  `.tmp` только для production verification, ZIP локально не сохранялся.
- Старый production verifier корректно дошёл до toolchain comparison, но ложно
  отверг evidence: единственная разница `cargo -vV` — local Windows 11
  `10.0.26300` против hosted Windows Server 2022 `10.0.20348`.
- M0.9.88 parser требует closed eight-line Cargo identity, сравнивает exact
  version/commit/date/target/libgit2/libcurl, проверяет `os:` как 64-bit
  Windows и исключает только release text из equality. Реальный run после
  исправления прошёл все четыре `gh attestation verify`; fresh run exact
  committed fix оставался acceptance gate.
- Exact fix revision `68804c93f43b405008cfecf35dc8660db360a3a3`
  прошла новый local clean-root pair и GitHub run `38507114425`. Attestation
  `55683374`, artifact ID `11868441712`; downloaded evidence прошёл production
  verifier с `attestation_verified=true`. M0.9.88 принят.

## M0.9.89 current M1 candidate refresh (2026-10-14)

- Field baseline: M0.9.87 run `20261014-080905`, revision
  `162836bb7a54180c3a00915749cf17fe238019e2`, runtime surface 83 entries /
  `1aacaca665c9a2ab4dbc43107c2556128274d7fb9f8727d2dcc78681026bb320`.
- Independent baseline: M0.9.88 revision
  `68804c93f43b405008cfecf35dc8660db360a3a3`, run `38507114425`, attestation
  `55683374`, protected surface 63 entries /
  `2a7db2c5cacd0419a8ece61b0dc8386075a558d4f052b44c0eedd81cd9354d68`.
- Candidate format v2 сохраняет reduced result и false strict-acceptance bit;
  production verification возможна только после commit, потому что control
  surface обязана быть tracked и clean.
- Candidate revision `d9142003eeb33f4e9ff44f100d88edd556c058c1` прошла
  production verifier: обе selected-tree surfaces unchanged,
  `network_retest_required=false`, network/build/archive/tag activity false.

## M0.9.90 complete M1 readiness gate (2026-10-14)

- Full manual boundary audit: первый проход 30/31, единственный failure — parser
  error Windows PowerShell 5.1 в старом volunteer field-kit boundary из-за
  UTF-8 Cyrillic literals без BOM.
- Boundary исправлен explicit UTF-8 read и ASCII-safe Base64 literals; повторный
  проход 31/31 successful.
- `cargo fmt --check` и workspace/all-target/all-feature strict Clippy прошли с
  двумя jobs и BelowNormal priority. Все 24 Rust test harnesses compile-only
  собраны в verification profile; test EXE не запускались.
- Одноразовый verification cache 6.52 GiB удалён штатным `-PruneVerification`;
  target уменьшен с 43.87 до 37.35 GiB, debug/release cache сохранён.
- Новый readiness gate pin-ит closed inventory уже 32 boundaries (включая свой),
  exact markers, production candidate, format и offline strict Clippy.
- Первый production run выявил PowerShell 5.1 stderr-redirection quirk после
  successful Clippy; capture удалён, реальный Cargo exit code сохранён.
- Revision `56c93aba38648fa5dbe57b22bbf2640c26590c2f` прошла clean-tree
  production readiness: `boundary_gates=32/32`, candidate/format/offline strict
  Clippy verified, все stage mutation flags false.

## M0.9.91 continuous Windows CI (2026-10-14)

- Added `.github/workflows/windows-ci.yml` for master push, PR and manual runs.
- Permissions are `contents: read`; checkout action is full-SHA pinned, does not
  persist credentials and fetches complete history for candidate continuity.
- One locked dependency fetch is followed by Cargo offline mode, the now
  33-boundary readiness gate and `run-cargo-tests-stable.ps1 -Run -NoFailFast`
  with two jobs.
- Hosted CI creates no release build, uploaded artifact, attestation or Git tag.
- First hosted run `38509447420` passed checkout/toolchain/fetch and complete
  33-gate readiness. Test compilation took 4m32s; all non-CLI harnesses passed.
  The aggregate CLI process passed 79/81 tests and failed only
  `live_runtime_applies_revocation_republishes_ticket_and_retires_ratchet`
  (10-second restart deadline) and
  `runtime_pairwise_store_discovers_ticket_and_pushes_own_device_announcements`
  (25-second discovery deadline) late in a 770.71-second process.
- Remediation does not edit `apps/` or `crates/`: a strict format-v1 JSON
  manifest resolves each exact test against `--list`, skips it only in the
  aggregate CLI invocation and executes it once in a fresh process. Zero-match,
  duplicate/malformed selector or isolated failure fails closed.
- Hosted runner now uses unoptimized Cargo `test` profile to reuse preceding
  Clippy artifacts and has a 38-minute test-step bound. Local compile-only
  validation resolved both selectors; no Rust test executable was run locally.
- Hosted run `38513111049` confirmed 79 aggregate CLI tests and the isolated
  revocation/restart test. The pairwise test failed alone with its first cause
  `persist runtime record ... os error 3`; repeated HTTP 404 followed because
  the first locator was never locally journaled/uploaded.

## M0.9.92 Windows extended-length runtime journal (2026-10-14)

- Root cause: runtime ticket-publication file names plus a normal Windows state
  root can exceed 260 UTF-16 code units. `NamedTempFile::persist_noclobber`
  received the lexical destination instead of an extended-length path.
- Fix: create the parent, canonicalize that existing directory, append the
  not-yet-existing file name, and use the resolved path for all append-only I/O.
- Local bounded regression passed with two Cargo jobs: a >260 path inserts,
  repeats as `AlreadyPresent`, and reads exact bytes. No socket/test endpoint
  was opened locally, avoiding a new Windows Firewall identity.
- CI manifest now isolates one exact test. Pairwise locator runs in aggregate.
  Tests are conditioned on successful locked fetch and use `always()` so stale
  M1 evidence cannot suppress product regression feedback; readiness failure
  still fails the job.
- `apps/kilogram-cli` changed. M0.9.87 field surface continuity is therefore
  intentionally invalid until a fresh physical field run is accepted.

## M0.9.93 fresh field evidence and candidate refresh (2026-10-14)

- Fresh kit was generated from clean revision
  `d33dd0950484665ee397d161e37ce74743ee8f93` with debug profile and two Cargo
  jobs; no ZIP or release build was created.
- Desktop ran Alice + Provider 1; the SSH-controlled Windows laptop ran
  Provider 2 + Bob. Only role-owned public files and closed logs were copied;
  private roots, Device secrets, runtime state and live logs stayed local.
- Diagnostic run `20261014-103458` completed message delivery and restart but
  was rejected because Provider 1 reported `relay_status=timeout after 30s`.
- Clean retry `20261014-104506` proved APS1 relay registration on both
  providers, exact volunteer receipts `2/2`, sender shutdown before receive,
  two delete-after-commit results and no restart redelivery.
- Final evidence verifier reported `result=verified-reduced-two-host` and
  `independent_provider_field_acceptance=False`.
- The refreshed 83-file runtime surface is
  `95c1771de3ca71e3912ab028d50fde3facaa8d05cab81873bdefbb95804f2d34`;
  independent offline surface remains unchanged and keeps M0.9.88 evidence.
- Candidate commit `d8252c660dcde49d93ad32c16104c46e91ac8c84` passed
  production verification on a clean tree. Full local readiness then passed
  `33/33`, format and locked/offline strict Clippy with two jobs; no test EXE,
  release/archive/tag or background service was created.

## M0.9.94 hosted completion diagnostic (2026-10-14)

- Windows CI run `38519726403` на exact revision `3887e06...` прошёл
  33-boundary readiness, aggregate test executables и единственный isolated
  lifecycle test; isolated result: 1 passed за 69.84s.
- Runner напечатал `status=passed` и `isolated_test_count=1`, после чего
  `cargo-cache-maintenance.ps1` упал на `$measure.Sum` для пустой существующей
  profile directory. Это первая реальная причина красного job.
- Исправление не удаляет cache и не запускает новую сборку: empty/missing tree
  становится `0/0`, non-empty tree сохраняет exact counts, multiple summary
  остаётся ошибкой.
- Regression `-SelfTest` успешно пройден локально в Windows PowerShell 5.1 и
  PowerShell 7; он использует отдельный GUID temp root и удаляет только его.
- До commit production readiness запускать нельзя: candidate verifier требует
  clean tree. После commit нужен clean readiness и новый hosted run.
- Implementation revision `520793ff17d35d22323f984765b43f4ad8e1c5d3`
  прошла normal cache report (`46103` files / `37.48 GiB`), Windows CI boundary
  и clean production readiness `33/33`; candidate, format и offline strict
  Clippy verified. Test EXE/network/release/archive/tag flags остались false.

## M0.9.95 hosted isolated-test scheduling (2026-10-14)

- Run `38523647852` прошёл 33-gate readiness и все 276 aggregate tests; exact
  isolated revocation/restart test failed only on its 10-second IPC-ready wait
  after the recovered endpoint had already reached `relay_status=skipped`.
- Stable runner inherited `BelowNormal` from the interactive Cargo policy even
  on the disposable hosted runner and ran isolated work after the full suite.
- CI now passes `-NormalProcessPriority -IsolatedTestOrder BeforeAggregate`.
  Defaults remain `BelowNormal`/`AfterAggregate` for local interactive use.
- Windows CI boundary statically closes both explicit order branches, exact
  workflow flags and observable priority/order markers.
- Focused local exact tests passed twice at normal priority (59.82s/59.85s).
  Full runner validation then passed isolated first (60.64s), aggregate CLI
  81/81 (530.22s), final cache report and `status=passed`.
- Implementation revision `c6793136ef0839c6c854227929fa9dfe32cc3f74`
  прошла clean M1 readiness: boundaries `33/33`, candidate, format и
  locked/offline strict Clippy verified; readiness mutation flags false.
- No Rust source, runtime/network action, release/archive/service/tag mutation
  occurred; accepted field and independent surfaces remain unchanged.

Hosted follow-up:

- run `38531172733` применил `cargo_process_priority_override=normal` и
  `isolated_test_order=BeforeAggregate`, но isolated test всё равно упал на
  прежнем 10-second restart IPC-ready deadline за 61.34s;
- все 276 aggregate tests после него прошли; scheduling-only hypothesis
  отвергнута, а empty-cache reporter не был достигнут из-за итогового failure.

## M0.9.96 task-aware runtime test readiness (2026-10-14)

- В `#[cfg(test)]` добавлен bounded helper, который до 60 секунд ждёт IPC file
  и одновременно наблюдает runtime `JoinHandle`.
- Если runtime завершится раньше, helper немедленно возвращает реальную ошибку
  или unexpected-clean-exit context; retry и success masking отсутствуют.
- Helper применён к initial и recovered startup одного isolated lifecycle
  test; production constants и runtime behavior не менялись.
- `cargo fmt --check` и exact test прошли; exact duration 61.02s.
- Rebuilt `target/debug/kilogram-cli.exe`: `57974784` bytes,
  SHA-256 `e10b162a004031165179eb52a84e2d8a70a4e894f0e1203d40ff0f08243a6979`.
  Previous field EXE: те же `57974784` bytes, SHA-256
  `c932e72291a5a55f62b847085e50b570ce6a7f1955926926a4545cb833b7f962`.
  PE identity/debug metadata prevents a byte-identity claim.
- Existing fail-closed field surface hashes all `apps/kilogram-cli`, поэтому
  требуется fresh physical field run и candidate refresh exact revision.

## M0.9.97 configured relay evidence (2026-10-14)

- Fresh two-host run `20261014-135100` on revision `78164eca...` completed the
  full product lifecycle: 2-of-2 storage receipts, Alice offline, Bob received
  once, both replicas deleted after commit, restart produced no redelivery.
- The verifier rejected the run because local Provider 1 reported a bounded
  APS1 registration timeout and therefore had no observed `relay_home_url`.
- Runtime now prints `relay_config_url` for an explicit configured relay before
  the online wait. `relay_home_url` still means an observed online relay only.
- The evidence verifier requires the canonical configured pin and rejects any
  divergent observed home. It does not reinterpret timeout as online status.
- M0.9.69/M0.9.87 verifier self-tests pass, including the new divergent-config
  mutation. The exact isolated lifecycle test passes `1/1` in 60.64 seconds.
- The rejected run is diagnostic only. Rebuild, exact-revision field rerun,
  candidate refresh, clean readiness and hosted CI are still required.
- A fresh kit built from `be80d28ec12b41987bdbfa800016fb86b54b4723`
  completed two-host run `20261014-141058` on desktop plus SSH-controlled
  laptop. The application lifecycle proved two distinct stores, exact 2/2
  signed receipts, sender shutdown before receive, two delete-after-commit
  outcomes, one committed history message and no restart redelivery.
- The first final wrapper attempt exposed a sync ordering race: the stop marker
  arrived before Provider 2's already-closed log. Final verification now waits
  for both provider stdout/stderr pairs before writing `08-boundaries.log`.
- Relay-pin verification now requires one exact `relay_config_url` for every
  endpoint-bearing runtime log, permits absent observed registration under
  `auto`, and rejects every divergent `relay_home_url` or `target_relay_url`.
- Direct strict verification reports `result=verified-reduced-two-host` and
  `independent_provider_field_acceptance=False`. Provider processes and local
  field-sync controller are stopped; no background Kilogram process remains.

## M0.9.98 current candidate refresh (2026-10-14)

- Candidate field baseline: run `20261014-141058`, revision
  `be80d28ec12b41987bdbfa800016fb86b54b4723`, reduced result only.
- Field selected-tree identity was recomputed with the production
  `git-ls-tree-sha256-v1` algorithm: 83 entries,
  `b5cb9bc9e06d24c73e63289e662ba019f88b7b052638c6a4b30099470f558095`.
- Current HEAD matches that field tree exactly. Independent revision
  `68804c93...` and current HEAD also match on the 63-entry offline protected
  tree (`2a7db2c5...d68`), so existing artifact/attestation identity is retained.
- Candidate and boundary self-tests pass while the control surface is dirty;
  production verification intentionally waits for a committed clean control
  surface.
- Candidate-control commit `bc3e8248b2f3b278ed86792260fd20147ced08ac`
  passed production verification from a clean tree.
- Complete readiness passed all `33/33` boundaries, format and locked/offline
  strict Clippy with `CargoJobs=2`. Mutation markers remained false: no Rust
  test executable, network action, release build, archive, service or Git tag.
- Next gate is push plus a fresh green GitHub-hosted Windows CI run.

## M0.9.99 hosted Windows acceptance (2026-10-14)

- Pushed range `a45ce0c..9d7895f` to `origin/master`; no tag was created.
- GitHub run `38588961507`, job `115938408741`, exact SHA
  `9d7895f28853d43eaecad15897b2ca5d1c279713`, completed success in 38m41s.
- Hosted readiness reported `m1_readiness_status=verified`, gates `33/33`,
  candidate verified and all mutation flags false.
- Isolated lifecycle passed `1/1` in 96.61s. Aggregate harness counts sum to
  276; every result had zero failures. Final markers: `status=passed`,
  `isolated_test_count=1`, `cache_above_warning=false`.
- This closes the hosted gate only. Reduced field topology and all candidate
  residual risks remain unchanged; strict independent providers and M1 tag are
  not claimed.

## M1.0.0 acceptance procedure (2026-10-14)

- User explicitly authorized M1.0.0 with the recorded limitations.
- The milestone is a Windows technical proof of concept, not a public security
  release or stable Cargo API; workspace package version remains `0.1.0`.
- Add RFC-0123 and current README/roadmap state without changing runtime,
  protocol, independent-reproduction or field-evidence surfaces.
- Commit the acceptance documents, pass the production candidate verifier and
  complete readiness, push the exact commit, and require a fresh green hosted
  Windows workflow.
- Only then create and push one annotated `M1.0.0` tag at that exact commit.
  Do not create a GitHub Release, executable attachment, ZIP or release build.

## Post-M1 accepted-tag readiness (2026-10-15)

- Governance commit `3a8aa7c` was pushed and annotated tag `M1.0.1` was
  created without moving `M1.0.0`; collaboration baseline commit `bcf4c81`
  was then pushed.
- Both ordinary post-M1 pushes exposed the intended old candidate behavior:
  current Cargo manifests no longer equal the pre-M1 selected tree, so the
  rolling candidate check failed before tests. The runs were diagnostic and
  were cancelled/superseded rather than weakening historical evidence.
- `verify-kilogram-m1-tag-acceptance.ps1` verifies exact local identities for
  the annotated `M1.0.0` object, commit, tree, candidate record, RFC-0123 and
  accepted decision log, plus ancestry to current `HEAD`.
- `verify-kilogram-m1-readiness.ps1 -AcceptedTag M1.0.0` passed all `33/33`
  boundaries, format and locked/offline strict Clippy with `CargoJobs=2` and
  BelowNormal priority.
- Result markers: `verification_mode=accepted-tag`,
  `accepted_m1_tag_status=verified`, `network_executed=false`,
  `test_executables_started=false`, `release_build=false`,
  `archive_created=false`, `git_tag_created=false`.
## M1.1.1 canonical-wire verification snapshot (2026-10-15)

- Targeted `kilogram-protocol` canonical module: 5/5 tests passed in the normal
  unoptimized test profile with two Cargo jobs and BelowNormal priority.
- `verify-kilogram-canonical-wire-boundary.ps1 -SelfTest` and production mode
  passed under Windows PowerShell; the independent encoder reproduced both
  checked CBOR contents, signing preimages and signed envelopes.
- Complete accepted-tag readiness passed `35/35`, immutable `M1.0.0`, format
  and locked/offline workspace Clippy. It started no network action, release
  build, archive or Git tag.
- The test executable was the pure protocol harness and opened no sockets; the
  readiness pass itself continued to report `test_executables_started=false`.

## M1.1.2 canonical-authority verification snapshot (2026-10-15)

- `kilogram-protocol` full library regression passed `25/25` in the normal
  unoptimized test profile with `CargoJobs 2` and BelowNormal priority; the new
  authority/session module contributes four tests and opens no sockets.
- `verify-kilogram-canonical-authority-reference.mjs` reconstructed the public
  Root/Device keys, four deterministic CBOR contents/envelopes and all four
  Ed25519 signatures with Node's maintained `crypto` implementation; no npm
  dependency, download or handwritten signature primitive was used.
- `verify-kilogram-canonical-authority-boundary.ps1 -SelfTest` and production
  mode passed. Negative cases rejected activation without migration, removed
  schema object, changed Rust binding, non-canonical hex, signature tamper and
  unknown capability.
- Complete accepted-tag readiness passed `36/36`, immutable `M1.0.0`, format
  and locked/offline workspace Clippy. It started no Rust test executable,
  network action, release build, archive or Git tag.
- Runtime remains Postcard. The CDDL/CBOR objects and domains are inactive
  conformance material and do not require the laptop or a field run.

## M1.1.21 canonical publication-conflict evidence snapshot (2026-10-15)

- `kilogram-protocol` library regression passed `66/66`; publication,
  observation and conflict-proof golden/negative tests opened no sockets.
- Independent Node reference reproduced exact deterministic CBOR, Ed25519
  public keys/signatures and BLAKE3 publication/observation/proof/evidence IDs;
  its mutation self-test rejected ticket and detector substitutions.
- Strict decoder mutation regression passed `10583/10583` in the unoptimized
  profile with `CargoJobs=2` and BelowNormal priority.
- Complete accepted-tag readiness passed `55/55`, immutable `M1.0.0`, rustfmt
  and workspace-wide locked/offline strict Clippy.
- Active Postcard/base64url publication-conflict runtime remains unchanged;
  coverage is intentionally partial until M1.1.22 resolution objects land.

## M1.1.22 canonical publication-conflict resolution snapshot (2026-10-15)

- New Rust module:
  `crates/kilogram-protocol/src/canonical_publication_resolution_v1.rs`.
- It defines strict deterministic-CBOR request, Root resolution, response and
  portable claim codecs. Authority verification consumes a separately verified
  `CanonicalSignedAccountDeviceListV1`; request bytes never authorize
  themselves.
- The reference generator supports deterministic vector regeneration with
  `--emit --output <path>` and independently derives Ed25519/BLAKE3/CBOR/URI.
- Targeted protocol regression passed 68/68 with `CargoJobs=2` and BelowNormal
  priority. Decoder mutation regression passed 11627/11627 in about 219
  seconds in the unoptimized profile.
- Complete accepted-tag readiness passed `55/55`, immutable `M1.0.0`, rustfmt
  and workspace-wide locked/offline strict Clippy. It started no network
  action, release build, archive or Git tag.
- Claim mutation is contextual: standalone claim integrity is not claimed;
  `verify_publication_conflict_claim_for_response_v1` binds every mutation to
  the exact signed response while the shared KPC1 user-comparison code remains
  identical for request and response.
- Canonical coverage is 13 complete, 0 partial, 5 not-started and 2
  not-targeted. The candidate remains inactive and the accepted M1.0.0 tag is
  unchanged.

## M1.1.23 canonical endpoint-announcement plaintext snapshot (2026-10-15)

- New Rust module:
  `crates/kilogram-protocol/src/canonical_endpoint_announcement_v1.rs`.
- Deterministic vectors cover the signed plaintext bundle, accepted observation
  evidence and all current publication-state variants; Node independently
  reproduces CBOR, Ed25519 and BLAKE3.
- Full protocol library regression passes 70/70 with `CargoJobs=2` and
  BelowNormal priority. Eleven filesystem-backed legacy tests need the normal
  Windows temp-directory boundary outside the restricted tool sandbox; their
  first sandbox failure was environmental, and the identical elevated offline
  command passed.
- Decoder mutation inventory is 12149/13000. No network, release build, archive
  or Git tag is required.
- Complete accepted-tag readiness passed `56/56`, immutable `M1.0.0`, rustfmt
  and workspace-wide locked/offline strict Clippy.
- Canonical coverage is 13 complete, 1 partial, 4 not-started and 2
  not-targeted. Endpoint announcement remains partial until M1.1.24 adds its
  recipient-only HPKE envelope and session-bound acknowledgement.
- Active Postcard v4 bundle, HPKE v1 envelope and ACK v2 remain unchanged; the
  accepted M1.0.0 tag remains immutable.

## M1.1.24 canonical endpoint-announcement transport snapshot (2026-10-15)

- New Rust module:
  `crates/kilogram-protocol/src/canonical_endpoint_announcement_transport_v1.rs`.
- The deterministic-CBOR transport uses the exact encoded header as HPKE AAD,
  resolves the recipient key through the current Root-signed device list and
  verifies the decrypted canonical bundle in the same authority context.
- A separate recipient-signed acknowledgement binds the 32-byte session ID,
  bundle/envelope IDs, applied count and all derived bundle totals.
- Independent Node reference covers CBOR, plaintext Ed25519/BLAKE3 and ACK
  Ed25519/session/count binding. Real HPKE seal/open and tamper/replay rejection
  are exercised by Rust because Node has no handwritten HPKE implementation.
- Decoder mutation inventory is 12671/14000. No network, release build, archive
  or Git tag is required.
- Accepted-tag readiness contains 57 gates. Canonical coverage is 14 complete,
  0 partial, 4 not-started and 2 not-targeted; next priority is history-rewrap.
- Active Postcard v4/HPKE v1/ACK v2 remains unchanged. Canonical activation
  still requires an explicit identifier, downgrade tests, sustained fuzzing,
  independent cross-language HPKE conformance and fresh field evidence.

## M1.1.25 canonical history-rewrap snapshot (2026-10-15)

- New inactive Rust module:
  `crates/kilogram-protocol/src/canonical_rewrap_v1.rs`.
- Deterministic-CBOR request, manifest, entries, bundle and transfer bind one
  exact Account roster, conversation membership, roles, session, full ordered
  inventory and bounded range. Entries accept only verified RatchetText events.
- HPKE uses the certified recipient key and exact manifest/index/Event ID AAD.
  The checked-in fixture opens to `canonical history body`; wrong private key,
  session, request, signature and ciphertext substitutions fail closed.
- Independent Node reference rebuilds the roster and membership from seeds,
  verifies the existing authorized event signature/ID, and reproduces all new
  CBOR, Ed25519 and BLAKE3 bytes. It deliberately does not claim independent
  HPKE seal/open support.
- Protocol library regression passed 74/74. Bounded decoder mutation regression
  passed 13715/13715 in 555.52 seconds, debug/offline, `CargoJobs=2` and
  BelowNormal priority.
- Canonical coverage is 15 complete, 0 partial, 3 not-started and 2
  not-targeted. Readiness inventory contains 58 gates. Active Postcard v1 and
  the accepted M1.0.0 tag remain unchanged.

## M1.1.26 canonical Account recovery quorum snapshot (2026-10-15)

- New inactive Rust module:
  `crates/kilogram-protocol/src/canonical_account_recovery_v1.rs`.
- CDDL, JSON schema, generated vectors and a dependency-free Node reference
  cover Root-signed package, independent witness, bounded unsigned request and
  Device-signed current-roster approval/quorum.
- Exact authority/device-list equivalence, ordered membership heads, freshness,
  state-vector and roster digests, certified approvers and strict majority are
  validated fail closed. Cross-roster policy transition is deliberately not
  claimed by this stage.
- The first mutation run correctly exposed that an unsigned request can be
  changed into another valid request. The harness now binds this fixture to its
  expected domain-separated request ID; the rerun passed 14759/14759 in 598.78
  seconds with `CargoJobs=2` and BelowNormal priority.
- Protocol library regression passed 76/76 outside the restricted filesystem
  sandbox. An earlier identical sandbox run produced 11 Windows error-5 temp
  directory failures; those were environmental and all passed unchanged with
  normal filesystem access.
- Accepted-tag readiness passed 59/59, rustfmt and locked/offline strict Clippy.
  No network action, stable-path test EXE, release build, archive or tag was
  created. Coverage is 15 complete, 1 partial, 2 not-started and 2 not-targeted.

## M1.1.27 canonical Account recovery policy snapshot (2026-10-15)

- New inactive Rust module:
  `crates/kilogram-protocol/src/canonical_account_recovery_policy_v1.rs`.
- CDDL, schema, generated vectors and dependency-free Node reference cover a
  monotonic old/new package transition, per-Device approvals and joint-quorum
  certificate. Active Postcard code is deliberately untouched.
- The portable verifier validates both Root package+witness pairs, monotonic
  authority/membership/revocation/certificate state, strict approval ordering
  and both roster majorities. Installation still requires a separately stored
  local monotonic anchor.
- Protocol library regression passed 78/78 outside the restricted filesystem
  sandbox. Decoder mutation regression passed 15542/15542 in 748.84 seconds
  with debug/offline Cargo, two jobs and BelowNormal priority.
- Accepted-tag readiness passed 60/60; rustfmt and locked/offline strict Clippy
  passed. No network, stable-path test executable, release build, archive, push
  or tag was created.
- Canonical coverage is 16 complete, 0 partial, 2 not-started and 2
  not-targeted. Next target is local `runtime-ipc`; `transport-sync` remains the
  final not-started wire-negotiation surface.

## M1.1.28 canonical runtime IPC core snapshot (2026-10-15)

- New inactive module: `crates/kilogram-protocol/src/canonical_runtime_ipc_v1.rs`.
- CDDL, schema, golden vectors and independent Node reference cover a
  Device-signed loopback descriptor, short-lived bearer request and
  Device-signed response for Ping, WaitForChange, Shutdown and Error.
- Active `kilogram-runtime-ipc` remains Postcard exact version 26 with JSON
  descriptor signature domain v7. The candidate module is not imported by the
  active crate and no listener, firewall or runtime behavior changed.
- The response signature binds descriptor ID, caller request ID, exact request
  digest and responder Device. Request freshness is contained by descriptor
  freshness; command/result substitutions fail closed.
- A first full mutation run rejected every mutation but exposed an arithmetic
  inventory mismatch (actual 16311 versus predicted 16325) because the request
  fixture is shorter than the 128-position grid cap. After correcting the
  exact bounded inventory, the complete corpus passed 16311/16311 in 737.63
  seconds with debug/offline Cargo, two jobs and BelowNormal priority.
- Protocol library regression passed 81/81. Accepted-tag readiness passed
  61/61 with rustfmt and workspace locked/offline strict Clippy. No network,
  stable-path test executable, release build, archive, push or tag was created.
- Canonical coverage is 16 complete, 1 partial, 1 not-started and 2
  not-targeted. Runtime IPC is partial; transport-sync is the sole not-started
  source surface.

## M1.1.29 canonical runtime IPC read-only snapshot (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_read_v1.rs`.
- `CANONICAL-RUNTIME-IPC.cddl` and the exact JSON registry now include bounded
  conversation-list and snapshot-bound history-page request/results. Generated
  vectors and a dependency-free Node reference cover CBOR, Ed25519 and BLAKE3.
- Active `kilogram-runtime-ipc` remains Postcard version 26 with JSON descriptor
  version 7; no listener, stable-path executable, firewall or network behavior
  changed.
- Final decoder mutation regression passed 17337/17337 in 774.07 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. The first run rejected
  every mutation and differed only at the deliberately closed old inventory.
- Protocol library tests passed 85/85 outside the restricted Windows temp
  sandbox. Accepted-tag readiness passed 62/62 with rustfmt and workspace-wide
  locked/offline strict Clippy.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. Runtime IPC still lacks mutation/contact/mailbox/publication/
  automation/recovery command families; transport-sync remains not-started.

## M1.1.30 canonical runtime IPC contact snapshot (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_contact_v1.rs`.
- Shared CDDL/schema, generated vectors and independent Node reference cover
  exact peer contact enrollment plus read-only own-device-directory status.
- Active `kilogram-runtime-ipc` remains Postcard version 26 with JSON descriptor
  version 7. The canonical module is not imported by the active crate and no
  listener, stable-path executable, firewall or network behavior changed.
- `ApplyOwnDeviceDirectory` is explicitly not part of this slice. Descriptor
  path portability is canonical; absolute/application-owned/non-symlink checks
  remain the platform adapter's responsibility.
- Decoder mutation regression passed 18357/18357 in 779.37 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 89/89.
- Accepted-tag readiness passed 63/63; rustfmt and workspace locked/offline
  strict Clippy passed. No network action, release build, archive, push or tag
  was created.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next runtime IPC boundary is authority-changing own-device
  directory application.

## M1.1.31 canonical runtime IPC directory apply snapshot (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_directory_v1.rs`.
- CDDL, schema, generated vectors and a dependency-free Node reference cover a
  current-state-anchored, Root-signed, removal-only own-Device transition and
  its Device-signed exact result. Active Postcard v26/JSON descriptor v7 is
  untouched.
- The embedded successor closes pathname TOCTOU; an eventual adapter must keep
  an immutable exact file snapshot through its recoverable transaction. Device
  enrollment/replacement remains outside this bearer-authenticated command.
- The contract explicitly limits revocation to future recipient selection and
  does not claim retroactive ciphertext rewriting or remote erasure.
- Final decoder mutation regression passed 18879/18879 in 794.93 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 95/95 outside the restricted Windows temp sandbox.
- Accepted-tag readiness passed 64/64; rustfmt and workspace locked/offline
  strict Clippy passed. No network, stable-path test executable, release build,
  archive, push or tag was created.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next runtime IPC slices are durable queue insertion and a
  snapshot-bound paginated outbox projection.

## M1.1.32 canonical runtime IPC durable queue snapshot (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_queue_v1.rs`.
- CDDL, schema, generated vectors and a dependency-free Node reference cover
  idempotent durable `QueueMessage` insertion. The caller request ID is the
  exact queue ID and the Device-signed result binds the complete request digest.
- An eventual adapter must verify enrolled contact/current membership and seal
  the durable record under one state lock/transaction. `inserted=false` is an
  exact-retry result, not delivery evidence; plaintext must not enter logs.
- Active `kilogram-runtime-ipc` remains Postcard version 26 with JSON descriptor
  version 7. No listener, stable-path executable, firewall or network behavior
  changed.
- Final decoder mutation regression passed 19401/19401 in 793.87 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 99/99 with two explicit vector generators ignored.
- Accepted-tag readiness passed 65/65; rustfmt and workspace locked/offline
  strict Clippy passed. No network, release build, archive, push or tag was
  created.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next runtime IPC boundary is snapshot-bound paginated
  outbox projection.

## M1.1.33 canonical runtime IPC outbox snapshot (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_outbox_v1.rs`.
- CDDL, schema, generated vectors and a dependency-free Node reference cover a
  content-derived complete outbox snapshot and Device-signed bounded pages.
  Active Postcard v26/JSON descriptor v7 is untouched.
- Complete snapshots are strictly Queue-ID ordered and limited to 4096 items,
  4096 retry records and 256 contacts. Pages are limited to 128 items and
  continue with the exact snapshot ID and next index.
- An eventual adapter must retain the exact immutable view for continuation or
  explicitly reject a stale cursor after state drift, eviction or restart.
- Final decoder mutation regression passed 19905/19905 in 823.63 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 103/103 with three explicit generators ignored.
- Accepted-tag readiness passed 66/66; rustfmt and workspace locked/offline
  strict Clippy passed. No network, stable-path executable, release build,
  archive, push or tag was created.
- Mutation capacity now has only 95 slots below the 20000 ceiling. Before the
  next command family, increase the explicit bounded maximum consistently in
  the manifest, Rust corpus and all guarding scripts; do not silently drop
  existing fixtures.

## M1.1.34 canonical runtime IPC provider snapshot (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_provider_v1.rs`.
- CDDL, schema, generated import/selection vectors and an independent Node
  reference cover signed offer import and capability-free provider selection.
  The original signed offer is retained; transport identity and observation
  time are the only local projection fields.
- Import requires a current offer and the existing eight-bit canonical
  admission-work floor. Selection is bounded to eight, rejects duplicate store
  keys/offer IDs and must run against one immutable active-registry snapshot.
- Final decoder mutation regression passed 20947/20947 in 826.67 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. The prior run rejected
  every mutation and exposed only an exact inventory estimate two cases high;
  manifest, harness and all guarding boundaries now agree on 20947/25000.
- Protocol library regression passed 106/106 outside the restricted Windows
  temp sandbox, with four explicit vector generators ignored. The matching
  sandbox attempt had only known Windows temp-directory error-5 failures.
- Accepted-tag readiness passed 67/67; rustfmt and workspace locked/offline
  strict Clippy passed. Active Postcard v26/JSON descriptor v7 is unchanged;
  no network action, release build, archive, push or tag was created.

## M1.1.35 canonical runtime IPC mailbox capability status (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_mailbox_status_v1.rs`.
- CDDL, schema, generated vectors and a dependency-free Node reference cover a
  content-derived complete capability-head snapshot and Device-signed bounded
  pages. Active Postcard v26/JSON descriptor v7 is untouched.
- Complete snapshots are strictly ordered by owner Account/Device, recipient
  Account/Device and scope, limited to 1024 heads, with pages limited to 16.
  Only exact signed updates and optional exact signed acknowledgements cross
  this read-only boundary; local binding and read/write secrets do not.
- Final decoder mutation regression passed 21451/21451 in 849.44 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 109/109 with five explicit generators ignored.
- Targeted post-Clippy-fix regression passed 3/3 with one explicit generator
  ignored. Workspace strict locked/offline Clippy then passed with two jobs.
- Accepted-tag readiness passed 68/68; rustfmt and workspace locked/offline
  strict Clippy passed. No network, stable-path executable, release build,
  archive, push or tag was created.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next runtime IPC boundary is create/rotate/revoke mailbox
  capability mutation, separate from this secret-free status projection.

## M1.1.36 canonical runtime IPC mailbox capability mutations (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_mailbox_mutation_v1.rs`.
- CDDL, schema, generated vectors and an independent Node reference cover
  create, rotate and revoke requests plus Device-signed transition results.
  Active Postcard v26/JSON descriptor v7 is untouched.
- Activation exact-binds the target roster digest, Device-signed provider
  selection, provider-bounded expiry and exact-volunteer replica set. Rotation
  and revocation require the exact active recipient-acknowledged predecessor.
- Local mailbox binding/read/write secrets never cross this IPC boundary. An
  eventual adapter must compare and commit durable state atomically and replay
  the exact idempotency request without duplicating a transition.
- Final decoder mutation regression passed 23013/23013 in 932.66 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 112/112 with six explicit generators ignored.
- Accepted-tag readiness passed 69/69; rustfmt and workspace locked/offline
  strict Clippy passed. No network, stable-path executable, release build,
  archive, push or tag was created.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next bounded runtime IPC slice is publication-channel
  rotation and conflict handling; independent HPKE conformance and adapter
  transaction evidence remain activation prerequisites.

## M1.1.37 canonical runtime IPC publication-channel rotation (2026-10-15)

- New inactive module:
  `crates/kilogram-protocol/src/canonical_runtime_ipc_publication_v1.rs`.
- CDDL, schema, generated vectors and an independent Node reference cover one
  exact-current-state publication-channel rotation. Active Postcard v26/JSON
  descriptor v7 is untouched.
- The runtime helper validates descriptor/bearer/request lifetime, exact Device
  secret and peer-scope digest, re-derives the current public capability, then
  derives the successor internally. No Device secret or plaintext peer scope
  crosses the IPC boundary.
- A successful local transition is explicitly awaiting its first network
  publication and peer delivery. Network work and Root-authorized conflict
  resolution are not claimed by this stage.
- Final decoder mutation regression passed 23535/23535 in 932.48 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 115/115 with seven explicit generators ignored.
- Accepted-tag readiness passed 70/70; rustfmt and workspace locked/offline
  strict Clippy passed. No network, stable-path executable, release build,
  archive, push or tag was created.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next bounded runtime IPC slice is exact network publication
  plus peer-delivery evidence, separate from Root conflict authorization.

## M1.1.38 canonical runtime IPC publication network (2026-10-15)

- Added inactive
  `crates/kilogram-protocol/src/canonical_runtime_ipc_publication_network_v1.rs`
  with exact-state publish and refresh commands, Device-signed responses and
  strict canonical root-only HTTPS origin validation.
- The trusted runtime owns ticket construction, recipient HPKE slot creation
  and opaque-envelope transport. IPC exposes only bounded public identifiers,
  digests, lengths, authorization and Device observation; no ticket plaintext,
  HPKE ciphertext, reusable secret or local path crosses the boundary.
- CDDL, schema, generated vectors, RFC-0164 and an independent Node reference
  agree on the candidate. The active Postcard v26/JSON descriptor v7 runtime
  remains unchanged and no network was executed.
- Final decoder mutation regression passed 24579/24579 in 961.89 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority.
- Protocol library regression initially exposed sandbox-denied system TEMP;
  with a workspace-local TEMP it passed 118/118 with eight explicit vector
  generators ignored. This was an execution-environment permission issue, not
  a protocol failure.
- Accepted-tag readiness passed 71/71; rustfmt and workspace locked/offline
  strict Clippy passed. No release build, archive, push or tag was created.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. Root-authorized publication-conflict commands are the next
  bounded runtime IPC slice.

## M1.1.39 canonical runtime IPC publication conflict (2026-10-15)

- Added inactive
  `crates/kilogram-protocol/src/canonical_runtime_ipc_publication_conflict_v1.rs`
  with exact-state create/apply commands, content-addressed staged artifacts
  and Device-signed responses. Active Postcard v26/JSON descriptor v7 is
  unchanged.
- Creation stages the portable Device request durably before success. Apply
  independently verifies the exact-current Root response and atomically commits
  replacement endpoint, resolution, quarantine release and retained proof.
- Root secret, publication write key, replacement-ticket plaintext, portable
  request/response plaintext and local paths do not cross the IPC boundary.
  Sibling propagation remains explicitly pending.
- CDDL, schema, generated vectors, RFC-0165 and an independent Node reference
  agree on deterministic CBOR, Ed25519 and BLAKE3 identifiers. The reference
  self-test and fail-closed boundary passed.
- Final decoder mutation regression passed 25623/25623 in 958.12 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority.
- Protocol library regression initially exposed only the known restricted
  Windows system-TEMP permission failures; with workspace-local TEMP it passed
  121/121 with nine explicit vector generators ignored.
- Accepted-tag readiness passed 72/72; rustfmt and workspace locked/offline
  strict Clippy passed. No network, stable-path executable, release build,
  archive, push or tag was created.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next bounded runtime IPC slice is own-device discovery and
  automation, followed by recovery/device-link commands.

## M1.1.40 canonical runtime IPC own-device announcement automation (2026-10-16)

- Added inactive
  `crates/kilogram-protocol/src/canonical_runtime_ipc_own_device_announcement_v1.rs`
  with exact configure/status commands, Device-signed responses and a bounded
  foreground scheduler projection. Active Postcard v26/JSON descriptor v7 is
  unchanged.
- Configuration binds the exact Root-signed Account Device list, another active
  Device, a content-addressed recipient-ticket artifact, interval/validity,
  bounded retry values and an explicit Ethernet/Wi-Fi/mobile/unknown network
  mask. Ticket plaintext and local filesystem paths never cross canonical IPC.
- Policy IDs commit generation and predecessor; status snapshot IDs commit the
  complete Device-ID-ordered projection. Scheduler state is derived from
  enabled/network/failure state, and attempt/success/next/bundle/transport
  tuples fail closed when incomplete or temporally inconsistent.
- Results explicitly state foreground-runtime-only execution, no restart and
  `os_background_service_enabled=false`; no Task Scheduler entry, service,
  autostart, network call, stable-path executable, release build or archive was
  created.
- CDDL, schema, deterministic vectors, RFC-0166 and an independent Node
  Ed25519/BLAKE3 implementation agree. The dedicated fail-closed boundary and
  canonical conformance self-tests pass.
- Final decoder mutation regression passed 26667/26667 in 985.34 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 124/124 with ten explicit vector generators ignored.
- Accepted-tag readiness passed 73/73; rustfmt and workspace locked/offline
  strict Clippy passed. The immutable accepted `M1.0.0` tag remained verified;
  no network, push or tag was performed.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next bounded slices are pairwise own-device ticket
  discovery and then roster-wide orchestration; recovery/device-link IPC and
  transport-sync remain later work.

## M1.1.41 canonical runtime IPC own-device ticket discovery (2026-10-16)

- Added inactive
  `crates/kilogram-protocol/src/canonical_runtime_ipc_own_device_discovery_v1.rs`
  with exact configure/status commands, Device-signed responses and a bounded
  foreground pairwise discovery projection. Active Postcard v26/JSON
  descriptor v7 is unchanged.
- Configuration binds the exact Root-signed Account Device list, another
  active Device, two distinct directional public channel IDs, a canonical
  root-only HTTPS origin, publication TTL/refresh, announcement timing,
  bounded retry and an explicit Ethernet/Wi-Fi/mobile/unknown network mask.
- The trusted runtime derives private publication capabilities internally.
  Verified content-addressed ticket locator and child announcement policy are
  installed atomically; private capabilities, ticket plaintext, reusable
  secrets and filesystem paths never cross the canonical IPC boundary.
- Policy and snapshot identifiers bind generation/predecessor and the complete
  Device-ID-ordered projection. Derived status distinguishes disabled,
  network-blocked, awaiting-publish/fetch/announcement, backoff and
  synchronized states, and rejects incomplete time/bundle/transport tuples.
- Results explicitly state foreground-runtime-only execution, no restart and
  `os_background_service_enabled=false`; no Task Scheduler entry, service,
  autostart, network call, stable-path executable, release build or archive was
  created.
- CDDL, schema, deterministic vectors, RFC-0167 and an independent Node
  Ed25519/BLAKE3 implementation agree. The dedicated fail-closed boundary,
  canonical conformance and coverage checks pass.
- Final decoder mutation regression passed 27711/27711 in 1026.61 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 127/127 with eleven explicit vector generators ignored.
- Accepted-tag readiness passed 74/74; rustfmt and workspace locked/offline
  strict Clippy passed. The immutable accepted `M1.0.0` tag remained verified;
  no network, push or tag was performed.
- Canonical coverage remains 16 complete, 1 partial, 1 not-started and 2
  not-targeted. The next bounded runtime IPC slice is roster-wide own-device
  availability orchestration; recovery/device-link IPC and transport-sync
  remain later work.

## M1.1.42 canonical runtime IPC own-device roster automation (2026-10-16)

- Added inactive
  `crates/kilogram-protocol/src/canonical_runtime_ipc_own_device_roster_v1.rs`
  with exact configure/status commands, Device-signed responses and one
  foreground parent policy that projects the complete current sibling roster.
  Active Postcard v26/JSON descriptor v7 remains unchanged.
- The projection contains every active sibling from the exact Root-signed list
  plus retained permanently revoked tombstones, strictly ordered by Device ID.
  Active children inherit the parent policy; retired children are forced
  disabled. At most one child workflow may execute concurrently.
- Each complete pairwise discovery child is embedded as independently
  validated canonical CBOR inside a bounded `bstr`. This preserves the global
  canonical depth limit of eight instead of weakening it for composition.
- Ticket plaintext, derived private capabilities, reusable secrets and local
  filesystem paths do not cross IPC. Results explicitly retain
  foreground-runtime-only execution, no restart and no OS background service.
- CDDL, schema, deterministic vectors, RFC-0168 and the independent Node
  Ed25519/BLAKE3 reference agree. Dedicated actual/self-test boundaries,
  canonical conformance and coverage checks pass.
- Final decoder mutation regression passed 28755/28755 in 1023.72 seconds with
  debug/offline Cargo, two jobs and BelowNormal priority. Protocol library
  regression passed 130/130 with twelve explicit vector generators ignored.
- Accepted-tag readiness passed 75/75; rustfmt and workspace locked/offline
  strict Clippy passed. The readiness runner now preserves successful child
  stderr as evidence while still requiring zero exit code and an exact success
  marker, avoiding false Windows PowerShell `NativeCommandError` failures.
- The immutable accepted `M1.0.0` tag remained verified. No network, test EXE,
  release build, archive, push or tag was created. Canonical coverage remains
  16 complete, 1 partial, 1 not-started and 2 not-targeted; the next bounded
  runtime IPC slice is recovery-plan/device-link commands.

## M1.1.43 canonical runtime IPC history-recovery plan custody (2026-10-16)

- Added inactive
  `crates/kilogram-protocol/src/canonical_runtime_ipc_recovery_plan_v1.rs`
  with approve/status/cancel commands, Device-signed responses, exact request
  binding and content-addressed local custody. Active Postcard IPC v26/JSON
  descriptor v7 remains unchanged.
- Approval verifies the exact source-signed recovery link at current time,
  recomputes the complete SAS digest, binds the execution policy and signs the
  resulting bounded plan with the recipient Device. A short display code is not
  accepted in place of the full digest.
- Cancellation exact-compares plan and current record IDs, retains the original
  signed plan byte-for-byte and adds an immutable Device-signed tombstone.
  Successful results explicitly remain foreground-only, require no restart and
  assert that no OS background service is enabled.
- Root/pre-enrollment secrets and the one-shot Device-link bootstrap remain
  outside routine IPC. Recovery payloads, private capabilities, decrypted
  history and local filesystem paths do not cross this boundary.
- CDDL, schema, deterministic vectors, RFC-0169 and an independent Node
  Ed25519/BLAKE3 implementation agree. Dedicated actual/self-test boundaries,
  canonical conformance and coverage checks pass.
- The full bounded mutation regression passed 29799/29799 in 3716.29 seconds
  with debug/offline Cargo, two jobs and BelowNormal priority. Focused tests
  passed 3/3 with one explicit generator ignored.
- The first protocol regression invocation exposed only the known restricted
  Windows system-TEMP permission failures. With workspace-local TEMP it passed
  133/133 with thirteen explicit vector generators ignored in 280.19 seconds.
- Strict Clippy found and removed one `expect` from the ignored vector helper.
  Accepted-tag readiness then passed 76/76, immutable `M1.0.0`, rustfmt and
  workspace locked/offline strict Clippy. The default current-candidate mode
  correctly rejected post-M1 protected-surface drift and was not treated as
  fresh field evidence.
- No network operation, test executable, stable-path binary, release build,
  archive, push or tag was created. Canonical coverage remains 16 complete,
  1 partial, 1 not-started and 2 not-targeted. The next bounded runtime IPC
  slice is recovery-plan execution and scheduler results.

## M1.1.44 canonical runtime IPC history-recovery execution (2026-10-16)

- Added inactive
  `crates/kilogram-protocol/src/canonical_runtime_ipc_recovery_execution_v1.rs`
  with idempotent start/status requests, Device-signed responses, exact approved
  plan-record binding and a recipient-Device-signed append-only scheduler chain.
  Active Postcard IPC v26/JSON descriptor v7 remains unchanged.
- The scheduler verifies initialized, attempt-started, attempt-failed,
  attempt-progressed, cancelled and completed states. Every successor binds the
  exact predecessor, increments generation with checked arithmetic and carries
  the exact signed retry configuration and bounded attempt lease.
- Attempt failure accepts only a nonzero bounded code and the exact derived
  equal-jitter exponential delay. A focused regression added during final audit
  proves that counter overflow, zero failure code and a substituted delay fail
  closed even when the candidate successor is correctly signed.
- Routine callers cannot supply observed network/power, attempt outcomes or
  checkpoints. Runtime readiness is derived from plan policy, lifecycle, time,
  attempt lease and the signed observation; execution remains foreground-only
  and does not register an OS service.
- CDDL, schema, deterministic vectors, RFC-0170 and the independent Node
  Ed25519/BLAKE3 implementation agree. The reference self-test, dedicated
  actual/self-test boundary, aggregate conformance and coverage checks pass.
- The complete decoder mutation corpus passed 30843/30843 in 2674.96 seconds
  with debug/offline Cargo, two jobs and BelowNormal priority. Protocol unit
  regression passed 137/137 with fourteen explicit generators ignored in
  199.58 seconds; the only integration target is the already completed mutation
  corpus, and there are no doctests.
- Final rustfmt and strict offline Clippy passed. Accepted-tag readiness passed
  77/77 and kept immutable `M1.0.0` verified. The readiness audit also updated
  all exact aggregate IPC coverage expectations from 136 to 154 semantic rules.
- No network operation, stable-path executable, release build, archive, push or
  tag was created. Canonical coverage remains 16 complete, 1 partial,
  1 not-started and 2 not-targeted. The next priority is explicit migration of
  the active unpaged conversation list, followed by remaining mutation results.

## M1.1.45 active runtime IPC conversation pagination (2026-10-16)

- Active local-control IPC now uses Postcard version 27 and signed JSON
  descriptor domain v8. The unpaged conversation-list command/result was
  removed in favor of a cursor-bound page with a maximum of 64 summaries.
- The cursor is the exact UTF-8-label/peer-Account-ID tuple. Runtime selection
  sorts authenticated contacts, rejects duplicate keys, uses a strict
  `partition_point`, and performs expensive membership/history/endpoint work
  only for the selected page. The existing 16,384-event inspection ceiling is
  therefore page-local rather than account-wide.
- The desktop collects continuations automatically, independently validates
  every page, and has an explicit 1,024-page safety ceiling. The user-visible
  list remains complete or fails explicitly; silent truncation is forbidden.
- The co-installed runtime/desktop migration is an exact-version hard cut.
  Correctly signed legacy v26/v7 descriptors and authenticated v26 requests are
  rejected before connection/actor dispatch. No peer-network format changed,
  and the deterministic-CBOR candidate remains inactive.
- `kilogram-runtime-ipc` unit tests passed 17/17, the focused Windows
  multi-page aggregation test passed 1/1, and the existing direct-only
  two-runtime CLI integration passed 1/1. Tests needing Windows loopback and
  system TEMP were rerun outside the restricted sandbox; dependencies remained
  offline and Cargo used two jobs with BelowNormal priority.
- The dedicated migration boundary, all canonical runtime-IPC reference gates,
  protocol compatibility, coverage, rustfmt and workspace locked/offline
  strict Clippy passed. Accepted-tag readiness passed 78/78 against immutable
  `M1.0.0`.
- Default current-candidate production verification correctly rejected reuse of
  old physical field evidence after protected runtime/protocol drift. No new
  field claim, network test, executable, release build, archive, push or tag was
  created. The final partial runtime-IPC item is associated mutation results.

## M1.1.46 exhaustive runtime IPC completion inventory (2026-10-16)

- `CANONICAL-RUNTIME-IPC-COMPLETION.json` is now the exact completion ledger
  for 32 canonical commands, 31 canonical results, 32 command/result pairs and
  every 34/31 command/response variant in the active Postcard v27 Rust enums.
- `scripts/verify-kilogram-runtime-ipc-completion-inventory-boundary.ps1`
  derives the active enum inventory from source and self-tests missing variants,
  untracked results, omitted blockers and false completion claims.
- The old catch-all remaining item was replaced with exactly three actionable
  gaps: ticket automation policy/status, endpoint-announcement bootstrap helper
  disposition and removal of legacy central-HTTPS mailbox commands.
- All 19 runtime-IPC boundaries passed together. Canonical coverage, protocol
  compatibility and accepted-tag readiness passed 79/79; rustfmt and workspace
  locked/offline strict Clippy passed with two Cargo jobs and BelowNormal
  priority. No test executable or network code ran.
- No active encoding or peer wire changed. Canonical CBOR remains inactive and
  fresh multi-device field evidence is still required before activation.

## M1.1.47 canonical runtime IPC ticket automation (2026-10-16)

- Added inactive deterministic-CBOR ticket-automation request/response module,
  CDDL, schema/coverage/completion entries, golden vectors, an independent Node
  reference, a dedicated fail-closed boundary and four new mutation fixtures.
- Public policy/snapshot ID helpers reject a contact ID that cannot be rederived
  from the descriptor Account, peer Account and conversation label. The status
  verifier also derives exact success recheck and capped exponential failure
  deadlines from the signed policy and rejects noncanonical default HTTPS ports.
- Focused Rust tests passed 4/4 with one explicit generator ignored. Independent
  Node reference and dedicated/conformance/completion boundaries passed.
- Full decoder mutation corpus passed 31,851/31,851 over 123 fixtures in
  2647.50 seconds. Full `kilogram-protocol --lib` regression passed 141 tests,
  failed 0 and ignored 15 explicit golden-vector generators in 977.21 seconds.
- Accepted-tag readiness passed 80/80 against immutable `M1.0.0`; rustfmt and
  workspace locked/offline strict Clippy passed with two Cargo jobs and
  BelowNormal priority. No test network, release build, archive, executable,
  push or tag was created.
- Completion ledger now records 34 canonical commands, 33 results and 34 exact
  pairs. Canonical CBOR remains inactive; active Postcard v27/JSON descriptor v8
  is unchanged. Two runtime-IPC gaps remain: endpoint-announcement bootstrap
  helper disposition and legacy central-HTTPS mailbox command removal.

## M1.1.48 active runtime IPC endpoint bootstrap retirement (2026-10-16)

- Removed the obsolete public CLI/loopback IPC export, import and push endpoint
  announcement helpers while retaining authenticated peer push and the
  foreground automation paths that use the same internal storage primitives.
- The active local-control protocol made an exact hard cut from Postcard v27
  and signed descriptor domain v8 to v28/v9. Correctly signed legacy v27/v8
  descriptors and requests are rejected before connection or actor dispatch;
  no compatibility reader was added.
- The dedicated retirement boundary rejects a forged current version, an
  omitted retired variant and a reintroduced public CLI command. The exhaustive
  active inventory now contains 31 commands and 28 responses with two explicit
  legacy central-HTTPS mailbox blockers.
- `kilogram-runtime-ipc --lib` passed 17/17 tests and the focused CLI endpoint
  announcement regression passed 4/4. All canonical IPC boundaries, coverage,
  protocol compatibility, rustfmt and workspace locked/offline strict Clippy
  passed with two Cargo jobs and BelowNormal priority.
- Accepted-tag readiness passed 81/81 against immutable `M1.0.0`. Default
  current-candidate mode correctly rejected reuse of pre-change physical field
  evidence. No network test, release build, archive, executable, push or tag
  was created.

## M1.1.49 active runtime IPC legacy mailbox retirement (2026-10-16)

- Removed the two legacy central-HTTPS mailbox create/rotate variants from the
  public active loopback enum and runtime actor. Exact volunteer-provider
  create/rotate, revoke, and bounded one-way legacy disk upgrade remain.
- Active local control made an exact hard cut from Postcard v28 / signed JSON
  descriptor domain v9 to v29/v10. Correctly signed legacy descriptors and
  requests are rejected before dispatch; there is no dual reader.
- `CANONICAL-RUNTIME-IPC-COMPLETION.json` now reports both the canonical target
  and the active Postcard adapter complete: 34 commands, 33 results, 34 pairs,
  29 active command variants and 28 response variants, with no blocker or
  remaining-work entry.
- Canonical coverage is 17 complete, 0 partial, 1 not-started and 2
  not-targeted. The deterministic-CBOR local-control candidate is still
  inactive; no protocol identifier, negotiation or network transport was
  enabled by this stage.
- The dedicated legacy-mailbox retirement boundary, inventory/coverage gates
  and all 18 canonical runtime-IPC boundaries pass. `kilogram-runtime-ipc`
  passed 17/17 unit tests outside the restricted TEMP/loopback sandbox;
  `kilogram-cli` passed locked/offline compile check without starting its
  network-capable test binary.
- Accepted-tag readiness passed 82/82 against immutable `M1.0.0`; rustfmt and
  workspace locked/offline strict Clippy passed with two Cargo jobs and
  BelowNormal priority. The first sandboxed readiness attempt reached all 82
  boundaries but its embedded rustfmt was denied path canonicalization; the
  identical outside-sandbox rerun completed successfully.
- No network run, release build, archive, stable-path EXE, push or tag is part
  of this stage. The next bounded engineering surface is `transport-sync`
  parallel CBOR negotiation.

## M1.1.50 canonical sync ALPN allocation (2026-10-16)

- `kilogram/m1/sync/cbor/1` is allocated in the transport crate and canonical
  manifests, but remains absent from all active CLI endpoint registration and
  dial paths. The active compatibility registry still names only
  `kilogram/m0/sync/8` and Postcard v6.
- The new `SyncWireProtocol` classifier is exact and fail closed; unit tests
  reject truncated and future identifiers and prove the candidate is inactive.
- Dedicated boundary self-tests reject identifier substitution, premature
  registration, reuse of the legacy ALPN and candidate leakage into active CLI
  source. Canonical migration/sync/session/coverage gates consume the same exact
  identifier.
- Initial sandboxed `cargo fmt` was denied workspace path canonicalization. The
  identical outside-sandbox debug/offline run passed rustfmt and all 7 transport
  unit tests with two Cargo jobs and BelowNormal priority.
- Accepted-tag readiness passed 83/83 against immutable `M1.0.0`, including
  workspace locked/offline strict Clippy. The first readiness attempt exposed
  historical runtime-IPC gates that froze the aggregate coverage milestone;
  those gates now retain their surface assertions while accepting the current
  M1.1.50 aggregate ledger.
- No network execution, release build, archive, stable-path executable, push or
  tag occurred. The next bounded task is an authenticated listener capability
  bound to current Account authority before any canonical ALPN dial.

## M1.1.51 authenticated sync transport capability (2026-10-16)

- Added the Iroh-free `kilogram-sync-capability` crate and embedded its
  separately Device-signed v1 artifact into connection ticket v12. The exact
  context includes listener Account/Device, authority revision, requester
  Account, endpoint ID, publication epoch and the common prekey validity window.
- Capability validation freezes the current safe state: legacy Postcard ALPN is
  active, canonical CBOR ALPN is candidate-only and canonical dialing is false.
  The candidate remains absent from endpoint registration and dial paths.
- Online and offline ticket verification authenticates the outer v12 ticket
  before checking the nested capability binding. One captured clock value is
  used for directory and capability freshness in each verifier.
- The dedicated migration boundary self-tests reject a forged current v11,
  dropped endpoint binding, enabled canonical dial, undeclared dual reader and
  premature canonical ALPN registration. Offline dependency inspection proves
  the shared crate adds no Iroh, Tokio or Reqwest surface.
- Debug/offline checks used two Cargo jobs and BelowNormal priority. Capability
  tests passed 2/2 and filtered CLI connection-ticket tests passed 6/6; no
  network, release build, archive, stable-path executable, push or tag ran.
- Accepted-tag readiness passed 84/84 against immutable `M1.0.0`, including
  rustfmt and workspace locked/offline strict Clippy. The first standalone
  sandboxed rustfmt attempt was denied path canonicalization; the identical
  outside-sandbox check completed successfully.
- Canonical coverage remains 17 complete / 1 partial / 0 not-started / 2
  not-targeted. The next bounded work is an active dual reader plus an explicit
  signed capability activation, with no failure-triggered legacy fallback.

## M1.1.52 canonical sync historical-signature bridge (2026-10-16)

- Added `canonical_sync_legacy_event_v1` to `kilogram-protocol` plus CDDL,
  schema, committed vector, compatibility ledger, RFC and an independent Node
  reference. The bridge retains exact active Postcard `AuthorizedEvent` bytes
  and verifies the original event/certificate/authority signatures before use.
- Focused debug/offline protocol tests passed 2/2 with two Cargo jobs and
  BelowNormal priority. The one-time fixture generator required an
  outside-sandbox run because Windows DPAPI intentionally protects the test
  Account Root under the current user; the committed vector test itself needs
  no Root secret or DPAPI write.
- Dedicated boundary self-tests reject premature canonical registration,
  historical resigning, invalid vector shape and leakage into active wire,
  transport or CLI sources. The independent JavaScript reference reproduces
  the exact outer CBOR bytes and size bounds.
- `kilogram/m1/sync/cbor/1` remains allocated but unregistered and undialed. It
  is now explicitly marked incompatible with installed active history; a new
  explicit history-compatible generation is required before dual-reader work.
- Accepted-tag readiness passed 85/85 against immutable `M1.0.0`, including
  rustfmt and workspace locked/offline strict Clippy with two Cargo jobs. No
  network execution, release build, archive, stable-path executable, push or
  tag belongs to this stage.

## M1.1.53 typed history-compatible sync event carrier (2026-10-16)

- Added `canonical_sync_history_v2` with explicit legacy-Postcard and
  canonical-CBOR Event-ID namespaces, deterministic typed keys and a closed
  two-variant carrier. Raw 32-byte equality never collapses the namespaces.
- Allocated `kilogram/m1/sync/cbor/2` in `SyncWireProtocol`, but kept it out of
  CLI endpoint registration, dial paths and ticket capability. Both canonical
  generations remain inactive; Postcard v6 remains the only active sync wire.
- Independent Node generation exactly matched committed key/carrier vectors.
  Rust debug/offline regression passed 2/2 with two jobs and BelowNormal
  priority; variant substitution, unknown namespace, trailing data and raw-ID
  collision cases fail closed.
- History/migration/transport/coverage ledgers now distinguish verified typed
  carrier from pending v2 frames and pending authenticated ticket capability.
  Related boundary self-tests pass. Accepted-tag readiness passed 86/86 against
  immutable `M1.0.0`, including rustfmt and workspace locked/offline strict
  Clippy with two Cargo jobs.
- No network operation, release build, archive, stable executable replacement,
  push or tag is part of this stage.

## M1.1.54 history-compatible canonical sync v2 frames (2026-10-16)

- Added `canonical_sync_v2` as an inactive deterministic-CBOR codec for the
  complete `/cbor/2` inventory, diff, event-batch, complete, pause and rejection
  frame set. The active runtime still compiles and uses Postcard v6 only.
- Independent Node vectors bind the new v2 Ed25519 domains and exact nested
  typed-key/carrier framing. The large historical carrier remains single-source
  in the history vector and the sync vector records its exact CBOR prefix and
  encoded length rather than duplicating opaque fixture bytes.
- Focused debug/offline Rust regression passed 3/3 with two Cargo jobs and
  BelowNormal priority. Boundary normal/self-test runs reject premature ALPN
  activation and closed-variant drift.
- Accepted-tag readiness passed 87/87 against immutable `M1.0.0`, including
  rustfmt and workspace locked/offline strict Clippy with two Cargo jobs.
- No network operation, release build, ZIP/archive, stable executable
  replacement, push or tag belongs to this stage.

## M1.1.55 typed canonical sync v2 session checkpoint (2026-10-16)

- Added inactive `canonical_sync_session_v2` beside the unchanged v1 session
  module. Its content has 15 fields and signs the exact 23-byte
  `kilogram/m1/sync/cbor/2` identifier under new v2 signature and BLAKE3-ID
  domains.
- Replaced bare received/sent Event IDs with nested
  `CanonicalSyncEventKeyV2` values. The decoder rejects unknown namespaces,
  non-canonical order, duplicates and evidence rollback; equal raw IDs in the
  two namespaces remain distinct.
- Independent Node generation exactly matched four committed checkpoints and
  their Ed25519 signatures/BLAKE3 IDs. Focused debug/offline Rust regression
  passed 4/4 with two jobs and BelowNormal priority.
- Dedicated boundary normal/self-tests and the related history, frame,
  migration, transport, capability and coverage boundaries passed. The closed
  readiness inventory is now 88 gates.
- Final rustfmt, workspace-wide `--all-targets --all-features --locked
  --offline -D warnings` Clippy and accepted-tag readiness passed 88/88 against
  immutable `M1.0.0`. The run started no test executable and performed no
  network operation.
- `/cbor/2` is still unregistered, undialed and absent from connection-ticket
  capability. Active Postcard v6, network, release build, archive, stable-path
  executable, push and tag remain unchanged for this stage.

## M1.1.56 authenticated history-compatible sync capability (2026-10-16)

- Migrated portable connection tickets from exact v12/domain v12 to
  v13/domain v13 and the nested Device-signed sync capability from v1/domain
  v1 to v2/domain v2. There is no legacy or dual reader.
- Capability v2 exact-advertises `/cbor/2` as its sole inactive candidate;
  Postcard v6 remains its sole active protocol and canonical dial stays false.
- Shared capability regression passed 3/3, focused CLI connection-ticket
  regression passed 6/6 outside the filesystem sandbox, and the offline crate
  compiled/tested without Iroh or network dependencies.
- Aggregate coverage, migration, transport, history bridge, typed carrier,
  frame, compatibility and ticket boundaries passed normal and negative
  self-test modes. Readiness inventory remains 88 gates.
- Final accepted-tag readiness passed 88/88 against immutable `M1.0.0`, with
  rustfmt and workspace locked/offline strict Clippy. It reported
  `test_executables_started=false` and `network_executed=false`.
- No endpoint registration, canonical dial, network operation, release build,
  archive, stable-path executable replacement, push or tag belongs to this
  stage.

## M1.1.57 dormant history-compatible sync runtime adapter (2026-10-16)

- Added `crates/kilogram-session/src/canonical_sync_v2_adapter.rs` as a dormant,
  transport-independent `/cbor/2` client/server state machine. It signs typed
  inventory/diff frames and validates every closed frame after encoding and
  strict decoding in regression.
- `CanonicalSyncV2Store` preserves the complete namespace-aware Event key and
  defines atomic/idempotent batch semantics. `CanonicalSyncV2EventPolicy` is a
  mandatory pre-storage authorization boundary; the active store and concrete
  legacy/canonical membership policy remain deliberately unimplemented.
- Focused debug/offline regression passed 3/3 with two Cargo jobs and
  BelowNormal priority. It covers both carrier encodings and both transfer
  directions, plus policy rejection, duplicate inventory and incomplete store
  reads.
- `CANONICAL-SYNC-V2-RUNTIME-ADAPTER.json`, RFC-0183 and a dedicated negative
  boundary record that the module is absent from active CLI/Iroh transport.
  Aggregate migration, transport, history and coverage boundaries were updated
  without changing endpoint registration or dial behavior.
- Full `kilogram-session` regression passed 8/8. Final rustfmt, workspace
  locked/offline strict Clippy and accepted-tag readiness passed 89/89 against
  immutable `M1.0.0`; readiness started no Rust test executable and executed no
  network operation.
- No release build, archive, stable executable replacement, push or tag belongs
  to this stage.

## M1.1.58 dormant atomic typed carrier store (2026-10-16)

- Added `CanonicalSyncV2CarrierStore` to `kilogram-store` as a separate redb
  database with an exact schema marker. Its 72-byte key is conversation ID +
  big-endian Event namespace + Event ID; stored values are exact closed carrier
  bytes.
- Store reads strictly decode and verify every carrier, then match its embedded
  conversation and derived typed key against the database key. Missing,
  malformed, cross-conversation and immutable-conflict cases fail closed.
- `put_batch` validates/canonicalizes first, preflights all existing records in
  one redb write transaction and only then inserts missing records before an
  immediate-durability commit. Regression proves a later conflict leaves no
  earlier batch element visible.
- `kilogram-session` now implements its dormant `CanonicalSyncV2Store` trait
  for the concrete database. Focused locked/offline debug tests passed 3/3 for
  `kilogram-store` and 4/4 for the adapter, with two Cargo jobs and BelowNormal
  priority.
- `CANONICAL-SYNC-V2-TYPED-STORE.json`, RFC-0184 and a dedicated negative
  boundary keep the store absent from active CLI, Iroh transport and wire code.
  Legacy import, membership policy, checkpoint transaction and capability
  activation remain pending.
- Final locked/offline debug regressions passed 14/14 for `kilogram-store` and
  9/9 for `kilogram-session`. Rustfmt, workspace-wide strict Clippy and
  accepted-tag readiness passed 90/90 against immutable `M1.0.0`; readiness
  started no Rust test executable and executed no network operation.
- No network operation, release build, archive, stable executable replacement,
  push or tag belongs to this stage.

## M1.1.64 checkpoint-bound runtime frame journal (2026-10-16)

- Added `canonical-sync-v2-session-runtime-v1` to the dormant redb database.
  Its key reuses exact session identity and its value contains a version,
  current checkpoint ID, strict presence marker and optional closed canonical
  client-frame bytes.
- Bootstrap creates an empty record. Signed successor append reads, verifies
  and rebinds the record in the same immediate-durability transaction as the
  immutable successor and head. An exact legacy-bootstrap replay backfills only
  an absent empty record.
- `session_runtime_state` verifies the complete signed chain before exposing a
  decoded frame. `set_session_pending_client_frame` is current-head-bound and
  write-once; exact replay succeeds while stale or different frames fail
  closed. Inventory restore also checks local Device and session binding.
- Regression covers reopen, preserved successor rebind, idempotent retry,
  conflicting/stale/cross-session frames, corrupt checkpoint binding, invalid
  presence and missing runtime records. Full workspace debug/offline tests
  completed without failures; `kilogram-store` passed 26/26 and
  `kilogram-session` passed 12/12.
- Fifteen related boundary scripts passed both normal and negative self-test
  modes (30/30). All 94 root JSON documents parsed. Rustfmt, workspace
  locked/offline strict Clippy and accepted-tag readiness passed 96/96 against
  immutable `M1.0.0`.
- Cargo used two jobs and BelowNormal process priority. No active network path,
  release build, archive, stable executable replacement, push or tag was
  created. The next slice must atomically couple adapter transitions, received
  carrier writes and signed successors, then resume the exact restored action.

## M1.1.63 dormant signed session successor store (2026-10-16)

- `CanonicalSyncV2CarrierStore` now appends a verified Device-signed successor
  only against the exact current durable head. The canonical protocol verifier
  enforces identity, sequence, predecessor, round, typed evidence, availability,
  next-action, pause/resume and terminal transition rules.
- The immutable successor record and replacement head share one redb write
  transaction with immediate durability. Exact replay is idempotent; gaps,
  stale or forked successors, missing heads and terminal successors fail closed.
- `session_checkpoint_chain` reads every record from sequence one through the
  bounded head (maximum 65,536), revalidates the initial bootstrap and verifies
  every signed successor edge before returning a head. Missing or corrupted
  ancestry is never skipped or repaired implicitly.
- Focused store tests passed 5/5; full `kilogram-store` passed 24/24 and full
  `kilogram-session` passed 12/12. The dedicated boundary plus thirteen related
  aggregate boundaries passed normal and negative self-test modes.
- All 93 root JSON documents parsed. `cargo fmt --all -- --check`, workspace
  locked/offline strict Clippy and accepted `M1.0.0` readiness passed 95/95.
- Runtime frame transitions still do not write or resume these checkpoints.
  `/cbor/2` remains unregistered and undialed pending that integration and
  mutual authenticated capability selection. No network operation, release
  build, archive, stable executable replacement, push or tag belongs here.

## M1.1.62 dormant signed session bootstrap (2026-10-16)

- Added versioned redb tables `canonical-sync-v2-session-checkpoints-v1` and
  `canonical-sync-v2-session-checkpoint-heads-v1`. The immutable journal key is
  exact conversation/local Device/peer Device/session binding plus checkpoint
  sequence; its value locally binds import plan ID, checkpoint ID and canonical
  signed checkpoint bytes. The head repeats plan, sequence and checkpoint ID.
- `install_legacy_import_session_bootstrap` opens one immediate-durability
  transaction, re-reads and verifies the exact durable import checkpoint, and
  refuses missing or incomplete imports before installing journal and head.
  Exact replay is idempotent; orphan/conflicting state fails closed.
- `CanonicalSyncV2Client::bootstrap_from_completed_legacy_import` constructs
  and signs only the sequence-1/round-1 `SendInventory` shape. The import plan
  remains local evidence and is not claimed as peer-visible signed content.
- Focused debug/offline store tests passed 3/3; full `kilogram-store` passed
  22/22. Focused cross-crate session test passed 1/1; full
  `kilogram-session` passed 12/12. Tests required execution outside the file
  sandbox because Windows temporary directories returned access denied inside
  it; Cargo stayed at two jobs and below-normal priority.
- The dedicated boundary plus twelve related aggregate boundaries passed both
  normal and negative self-test modes. All 92 root JSON files parsed. `cargo
  fmt --all -- --check`, workspace locked/offline strict Clippy and accepted
  `M1.0.0` readiness passed 94/94.
- This stage starts no active network path, test/release archive or stable EXE.
  `/cbor/2` remains unregistered and undialed. Next work is durable successor
  checkpoint integration across every runtime transition, followed by mutual
  authenticated capability selection and downgrade regression.

## M1.1.59 dormant cross-format membership policy (2026-10-16)

- Added `CanonicalSyncV2MembershipPolicy` to the dormant session adapter. It is
  constructed from one explicit legacy Postcard or canonical-CBOR membership
  snapshot and preserves its conversation ID and revision.
- The complete 2-by-2 matrix authorizes both closed carrier variants from both
  snapshot formats. Each path rechecks the snapshot signature, exact
  conversation binding, carrier author authorization and Account membership
  before the atomic typed store can be called; format sniffing and parse-error
  fallback remain forbidden.
- This cross-format rule lets an installed Root-signed conversation membership
  authorize native canonical events without requiring the owner to re-sign
  historical membership state solely for migration.
- Focused regression passed 2/2 and full `kilogram-session` regression passed
  11/11. The dedicated boundary plus nine related aggregate boundaries passed
  normal and negative self-test modes.
- `CANONICAL-SYNC-V2-MEMBERSHIP-POLICY.json`, RFC-0185 and the fail-closed
  boundary keep the policy absent from active CLI, Iroh transport and wire
  code. `/cbor/2` remains unregistered and undialed.
- Final rustfmt, workspace locked/offline strict Clippy and accepted-tag
  readiness passed 91/91 against immutable `M1.0.0`; readiness started no test
  executable and executed no network operation.
- Legacy import, checkpoint/store transaction and authenticated capability
  selection remain pending. No release build, archive, stable executable
  replacement, push or tag belongs to this stage.

## M1.1.60 dormant bounded/resumable legacy-history import (2026-10-16)

- `kilogram-store` now builds an immutable import plan from the exact verified
  Postcard membership snapshot and every exact verified `AuthorizedEvent` in
  one conversation. Inventory is bounded at 4096 and sorted by Event ID.
- Domain-separated BLAKE3 claims bind membership bytes, source Event IDs and
  exact authorization bytes. The plan never converts installed history into a
  native canonical event; it creates the verified legacy bridge carrier.
- One import call accepts 1..64 events. The plan-bound cursor cannot skip work:
  every earlier carrier must already exist and exactly match the source claim
  and membership before a later page can commit. Same-cursor retry is
  idempotent, changed source/membership and cross-plan cursors fail closed.
- The existing carrier-store batch preflight and redb write transaction make a
  page atomic; regression proves a conflicting item leaves later missing page
  items absent. The cursor itself is deliberately not persisted in this stage,
  avoiding a false cross-file atomicity claim.
- Focused import regression passed 3/3 and full `kilogram-store` regression
  passed 17/17. The dedicated boundary and ten related aggregate boundaries
  passed normal plus negative self-test modes under Windows PowerShell 5.1.
- Root JSON parsing, rustfmt, workspace locked/offline strict Clippy and
  accepted-tag readiness passed 92/92 against immutable `M1.0.0`; readiness
  started no Rust test executable and executed no network operation.
- Next work is a redb transaction that couples carrier page and durable import
  cursor, followed by signed `/cbor/2` checkpoint/runtime capability
  integration. No active CLI/Iroh/wire path, release build, archive, stable
  executable, push or tag changed here.

## M1.1.61 dormant atomic legacy-import checkpoint (2026-10-16)

- Added a separate versioned redb table for the 129-byte
  `CanonicalSyncV2LegacyImportCheckpoint`. Its key is conversation ID plus
  immutable plan ID; the value repeats both and binds membership revision and
  digest, total event count, last committed page start and next source index.
- The importer now reads the durable cursor, preflights/inserts the complete
  carrier page and advances the checkpoint in one immediate-durability write
  transaction. Any carrier conflict or checkpoint failure rolls back both.
- A missing checkpoint accepts only index zero. Exact lost-response replay
  requires both stored page start and end, then revalidates every expected
  carrier without writing again; alternate same-end pages, stale/skipped bounds
  and malformed, corrupt or cross-plan records fail closed.
- Focused debug/offline regression covers reopen, retry, missing-checkpoint
  prefix rejection, stale page shape, corruption and later-page rollback.
- `CANONICAL-SYNC-V2-IMPORT-CHECKPOINT.json`, RFC-0187 and the dedicated
  negative boundary keep this state out of active CLI/Iroh/wire paths.
- Focused regression passed 5/5 and full `kilogram-store` regression passed
  19/19. The dedicated boundary plus eleven related aggregate boundaries passed
  normal and negative self-test modes under Windows PowerShell 5.1.
- Root canonical JSON parsing, rustfmt, workspace locked/offline strict Clippy
  and accepted-tag readiness passed 93/93 against immutable `M1.0.0`;
  readiness started no Rust test executable and executed no network operation.
- This local checkpoint is deliberately distinct from the Device-signed
  `CanonicalSyncSessionCheckpointV2`. Signed session-checkpoint/runtime and
  authenticated capability selection remain the next activation work.
- No network operation, release build, archive, stable executable replacement,
  push or tag belongs to this stage.
## M1.1.65 atomic runtime transition transaction (2026-10-16)

- Рабочая ветка: `master`; predecessor: local commit `f0f7970` (M1.1.64).
- Новый store API: `CanonicalSyncV2CarrierStore::commit_session_runtime_transition`.
  Он использует прежний `CanonicalSyncV2StoredSessionRuntime` как exact
  optimistic-concurrency token и одной immediate redb transaction пишет
  verified received carriers, signed successor record, head и replacement
  pending-frame runtime record.
- Exact replay после потерянного ответа идемпотентен только при полном совпадении
  successor/runtime/carrier bytes. Stale runtime, orphan record, malformed или
  conflicting carrier fail closed до частичного commit.
- Focused debug/offline regression запускается resource-friendly wrapper с
  `CargoJobs 2` и below-normal priority. Внутри файловой sandbox Windows temp
  directories возвращают access denied, поэтому тесты требуют разрешённого
  запуска вне sandbox; release build и ZIP не нужны.
- Новый verifier:
  `scripts/verify-kilogram-canonical-sync-v2-atomic-runtime-transition-boundary.ps1`.
  Он имеет negative `-SelfTest` и проверяет отсутствие вызова store primitive в
  adapter/wire/transport/CLI.
- Focused tests: 2/2; `kilogram-store`: 28/28; `kilogram-session`: 12/12.
  Все 95 root JSON parsed; 16 связанных boundary scripts прошли normal/self;
  accepted-tag readiness прошёл 97/97 с offline strict Clippy.
- Active Postcard v6 не изменён; `/cbor/2` не зарегистрирован и не dial-ится.
  Следующий этап — adapter transition integration и exact restart dispatch.

## M1.1.66 dormant durable client transition adapter (2026-10-16)

- Рабочая ветка: `master`; predecessor: local commit `f377960` (M1.1.65).
- Concrete client APIs: `restore_durable_runtime`, `durable_action`,
  `begin_durable_round`, `accept_durable_diff`, `mark_durable_batch_sent` и
  `accept_durable_complete`.
- До любого transport write exact Inventory/EventBatch уже связан с signed
  checkpoint. Diff received carriers и successor/frame пишутся через
  `commit_session_runtime_transition`, поэтому adapter не создаёт отдельного
  carrier/checkpoint crash window.
- Restart action не перестраивает frame из изменившегося local inventory:
  `ReceiveDiff`, `SendEventBatch` и `ReceiveComplete` возвращают exact persisted
  canonical frame. Wrong identity, action/frame mismatch и missing durable
  session fail closed.
- Двухраундовый debug/offline regression с redb reopen прошёл; полный
  `kilogram-session` regression — 13/13. Cargo запускался через friendly wrapper
  с `CargoJobs 2` и below-normal priority.
- Новый verifier:
  `scripts/verify-kilogram-canonical-sync-v2-durable-client-adapter-boundary.ps1`.
  Он проверяет aggregate contracts, negative self-test и отсутствие adapter
  calls в active wire/Iroh/CLI.
- Все 96 root JSON documents parsed; полный accepted-tag readiness прошёл
  98/98 против immutable `M1.0.0`, включая rustfmt и workspace locked/offline
  strict Clippy. Readiness не запускал Rust test executables и не выполнял
  network operation.
- Active Postcard v6 и network surface не менялись; `/cbor/2` остаётся
  unregistered/undialed. Release build, ZIP, stable EXE, push и tag не нужны.

## M1.1.67 dormant durable responder transition adapter (2026-10-17)

- Рабочая ветка: `master`; predecessor: local commit `5d25e51` (M1.1.66), над
  ним уже находится documentation-only commit `fa96186` (RFC-0193).
- Новый protocol module:
  `crates/kilogram-protocol/src/canonical_sync_responder_session_v2.rs`.
  Signature и checkpoint-ID domains намеренно отличаются от initiator domains.
- Новый store module:
  `crates/kilogram-store/src/canonical_sync_v2_responder_store.rs`. Отдельные
  checkpoint/head/runtime tables открываются вместе с dormant carrier store;
  received carriers и responder successor/head/pending frame записываются одной
  immediate-durability transaction.
- Concrete server APIs: `restore_durable_responder_runtime`,
  `durable_responder_action`, `accept_durable_inventory`,
  `mark_durable_diff_sent`, `accept_durable_event_batch` и
  `mark_durable_complete_sent`.
- Windows filesystem regressions требуют запуск вне sandbox из-за `os error 5`
  на tempdir/canonicalize. Использован resource-friendly wrapper с
  `CargoJobs 2`, offline debug profile и below-normal process priority.
- Проверено: `kilogram-protocol` 170/170, `kilogram-store` 28/28,
  `kilogram-session` 14/14; 18 связанных scripts прошли normal и `-SelfTest`
  (36 запусков); 97 root JSON parsed; rustfmt и workspace locked/offline strict
  Clippy прошли.
- Dedicated verifier:
  `scripts/verify-kilogram-canonical-sync-v2-durable-responder-adapter-boundary.ps1`.
  Он отвергает shared role domain, premature capability activation,
  parse-error fallback и active transport leak.
- Accepted-tag readiness прошёл 99/99 против immutable `M1.0.0`; внутри
  readiness Rust test executables и network не запускались. Отдельные
  regressions выше являются test evidence этого этапа.
- Следующий slice — mutual authenticated capability selection между durable
  client/responder без регистрации или dial `/cbor/2` до отдельного решения.

## M1.1.68 dormant mutual authenticated candidate selection (2026-10-17)

- Production dependencies: `kilogram-sync-capability` uses workspace BLAKE3
  for domain-separated capability/offer IDs; `kilogram-session` depends on the
  capability crate and uses `ed25519-dalek` only in integration tests.
- Targeted offline debug tests use
  `scripts/invoke-cargo-friendly.ps1 -CargoJobs 2`; redb/tempdir session tests
  need execution outside the Windows sandbox because sandboxed TEMP can return
  `os error 5`.
- Verified before documentation close: sync-capability 5/5 and focused mutual
  selection session regression 1/1; full session regression 15/15.
- Dedicated verifier:
  `scripts/verify-kilogram-canonical-sync-v2-mutual-candidate-selection-boundary.ps1`.
  It rejects activation permission, fallback, a missing authority binding,
  shared offer/acceptance signature domain and any active Iroh/CLI leak.
- No network operation, release build, archive or stable executable is needed
  for this dormant stage.
- Final local evidence: 20 affected boundary normal runs and 19 negative
  self-test runs passed; 98 root JSON files parsed; rustfmt and workspace
  locked/offline strict Clippy passed; accepted-tag readiness passed 100/100
  against immutable `M1.0.0`.

## M1.1.69 dormant mixed-version and downgrade matrix (2026-10-17)

- Implemented in
  `crates/kilogram-session/src/canonical_sync_v2_adapter.rs`; no active wire,
  Iroh or CLI call site was added.
- Targeted matrix regression and the full `kilogram-session` debug/offline suite
  pass 16/16. Redb/tempdir tests must run outside the Windows sandbox; the
  sandboxed attempt predictably returned `os error 5` while pure tests passed.
- Resource policy remained `scripts/invoke-cargo-friendly.ps1 -CargoJobs 2`,
  below-normal priority, debug/offline only. No release build or archive.
- Dedicated and 19 affected historical boundaries pass in normal and negative
  self-test modes (20/20 total including the new boundary). All 99 root JSON
  documents parse.
- `cargo fmt --all -- --check`, workspace `--all-targets --all-features`
  locked/offline Clippy with `-D warnings`, and accepted-tag readiness 101/101
  against immutable `M1.0.0` all pass.
- Active Postcard v6 remains unchanged; canonical `/cbor/2` remains allocated,
  unregistered and undialed. No network, stable EXE, push or tag was produced.

## M1.1.70 dormant dual-reader runtime coordinator (2026-10-17)

- Implemented in
  `crates/kilogram-session/src/canonical_sync_v2_adapter.rs`; the coordinator
  consumes a verified dormant transport plan and never calls active transport.
- Legacy dispatch is symbolic only. Canonical client/responder dispatch is
  derived from exact restored durable runtime after identity verification;
  missing or mismatched state fails closed.
- `CanonicalSyncV2DormantResponderDispatch` boxes the larger responder action;
  this satisfies workspace strict Clippy without suppressing
  `large_enum_variant` and does not alter persisted or wire bytes.
- Full `kilogram-session` offline debug regression passed 17/17 outside the
  Windows sandbox. Resource policy remained `CargoJobs 2`, below-normal
  priority, debug/offline only.
- The new and 20 affected historical boundary scripts passed both normal and
  `-SelfTest` modes (21/21 pairs). All 100 root JSON documents parse.
- `cargo fmt --all -- --check`, workspace locked/offline strict Clippy with
  `-D warnings`, and accepted-tag readiness 102/102 against immutable
  `M1.0.0` passed.
- Active Postcard v6 and executable/network surfaces are unchanged;
  `/cbor/2` remains unregistered and undialed. No release build, archive,
  stable EXE, network operation, push or tag was produced.

## M1.1.71 deterministic dormant rollback drill (2026-10-17)

- Implemented in
  `crates/kilogram-session/src/canonical_sync_v2_adapter.rs`; production code
  adds strict rollback snapshot/record codecs and BLAKE3-derived evidence IDs.
- The restart drill writes only to a test `tempdir`; no active runtime store or
  transport call site was added. Windows tempdir tests ran outside the sandbox
  with the existing resource-friendly wrapper.
- Full `kilogram-session` offline debug regression passed 18/18 using
  `CargoJobs 2` and below-normal priority. The two established long-running
  synchronization tests completed in about 83 seconds total.
- The new and 21 affected boundary scripts passed normal and `-SelfTest` modes
  (22/22 pairs). All 101 root JSON documents parse.
- `cargo fmt --all -- --check`, workspace `--all-targets --all-features`
  locked/offline Clippy with `-D warnings`, and accepted-tag readiness 103/103
  against immutable `M1.0.0` passed.
- No network operation, release build, archive, stable executable, push or tag
  was produced. Canonical `/cbor/2` remains unregistered and undialed.

## M1.1.72 bounded canonical sync v2 decoder mutations (2026-10-17)

- New focused test:
  `crates/kilogram-protocol/tests/canonical_sync_v2_decoder_mutations.rs`.
  Run it through `scripts/invoke-cargo-friendly.ps1` with `CargoJobs 2`, locked
  dependencies, offline debug profile and below-normal process priority.
- The deterministic corpus contains 20 exact fixtures and 2863 mutations. It
  uses no network or randomness and is bounded by an 8 MiB fixture limit, 128
  mutation grid positions and 4000 total cases.
- The responder checkpoint vector is regenerated in the test from a fixed
  Ed25519 seed and exact-checked against content, preimage, signature, signed
  CBOR and checkpoint ID stored in
  `CANONICAL-SYNC-RESPONDER-SESSION-V2-VECTORS.json`.
- Dedicated verifier:
  `scripts/verify-kilogram-canonical-sync-v2-decoder-mutations-boundary.ps1`.
  Its self-test rejects a false coverage claim, count/policy drift, signature
  drift and premature transport activation.
- Full `kilogram-protocol` locked/offline debug regression passed outside the
  Windows sandbox: library 155 passed / 15 explicit generators ignored, the
  historical 31,851-case corpus passed in 2607.50 seconds, and the new
  2,863-case sync-v2 corpus passed in 23.71 seconds. The initial sandboxed run
  produced only the established `os error 5` temp-directory failures; every
  one of those tests passed unchanged outside the sandbox.
- All 103 root JSON files parse. The closed readiness inventory and actual
  boundary script set both contain exactly 104 entries.
- `cargo fmt --all -- --check`, workspace all-target/all-feature locked/offline
  Clippy with `-D warnings`, and accepted-tag readiness 104/104 against the
  immutable `M1.0.0` tag all pass.
- This is deterministic regression evidence only. `cargo-fuzz`, nightly
  instrumentation and sustained coverage-guided execution were not available
  or claimed; that activation gate remains pending.

## M1.1.73 canonical sync v2 coverage fuzz harness (2026-10-17)

- Harness paths: `fuzz/Cargo.toml` and
  `fuzz/fuzz_targets/canonical_sync_v2_decoders.rs`; stable dispatcher is
  compiled only with protocol feature `decoder-fuzzing`.
- Exact seed validation passed with offline debug build and `CargoJobs 2`:
  `cargo test --package kilogram-protocol --features decoder-fuzzing --test canonical_sync_v2_fuzz_seeds --locked --offline` (1/1).
- Selector/dispatcher focused unit regression passed (1/1); all 13 selectors
  reject empty input without panic.
- Full protocol library regression with `decoder-fuzzing` passed 156 tests with
  15 explicit vector generators ignored in 190.98 seconds.
- `prepare-kilogram-canonical-sync-v2-fuzz-corpus.ps1 -VerifyOnly` verified 20
  raw-binary seeds totaling 5066 bytes and emitted exact SHA-256 evidence.
- `invoke-kilogram-canonical-sync-v2-fuzz.ps1 -Mode Preflight` completed without
  network or installation and reported `fuzz_toolchain_installed=false`,
  `cargo_fuzz_installed=false`, `windows_msvc_asan_detected=false`.
- 24 affected boundary scripts passed normal and negative `-SelfTest` modes;
  `verify-kilogram-m1-readiness-boundary.ps1` passed its normal mode. All 104
  root JSON documents parse.
- Root and nested-fuzz rustfmt checks passed outside the Windows sandbox after
  the sandbox returned the established `os error 5`. Workspace all-target,
  all-feature, locked/offline Clippy with `-D warnings` passed with two jobs and
  below-normal priority. Accepted-tag readiness passed 105/105 against
  immutable `M1.0.0`.
- Do not mark `observed.harness_compiled_with_libfuzzer`, smoke/sustained run or
  coverage as true until a prepared nightly + cargo-fuzz + ASan/LLVM host has
  actually produced the bounded machine record and coverage output.

## M1.1.74 pinned instrumented fuzz toolchain (2026-10-17)

- Workspace default remains stable Rust. The isolated harness uses installed
  toolchain `nightly` = rustc `1.101.0-nightly`, commit
  `5d8fec0669a64607db9ccbf663e7c23125bf4748`, commit date `2026-10-16`, LLVM
  `23.1.4`, with `llvm-tools-preview`.
- Installed driver is cargo-fuzz `0.13.2`; `fuzz/Cargo.lock` pins
  libfuzzer-sys `0.4.13` and is part of the reviewable source contract.
- Windows runtime files are under Visual Studio 2022 Community MSVC
  `14.44.35207`: `bin/Hostx64/x64/clang_rt.asan_dynamic-x86_64.dll` and
  `lib/x64/clang_rt.asan_dynamic_runtime_thunk-x86_64.lib`.
- Runner discovers both runtime files, verifies exact tool versions and only
  then temporarily prepends the runtime DLL directory to the child PATH.
- Windows PowerShell 5.1 reports ordinary native stderr as
  `NativeCommandError`; the runner therefore logs both channels quietly and
  uses the native exit code as the only process-success decision.
- Before every run, the ignored flat corpus and artifact directories are reset
  with bounded-path, reparse-point and unexpected-subdirectory guards, then
  exactly 20 seeds are materialized again. A one-second dirty-tree diagnostic
  completed 37,447 iterations with 0 artifact files.
- Instrumented build completed successfully. Output binary was 2,178,560 bytes
  with SHA-256
  `1c53c755571bcdaf2399fd82b1c54c81850253815e6e8a6add9041ee60d5b03a`.
- Generated `fuzz/target` is ignored and retained as a bounded incremental build
  cache; campaign logs, corpus, artifacts and coverage stay under ignored
  paths. No release archive or stable executable was produced.
- This stage proves environment and build reproducibility only. Run evidence,
  coverage and the 8-hour/1,000,000-execution gate remain separate stages.

## M1.1.75 instrumented decoder smoke (2026-10-17)

- Clean source revision before and after:
  `f9148093a7476e2d77c4a967b018c4b26a230225`.
- Runner mode `Smoke`, requested 300 seconds, observed 301 seconds, one job and
  one worker, process exit code 0.
- Observed 5,697,050 executions; exact reset started with 20 seeds and produced
  859 corpus files. The artifact directory remained empty.
- Machine record:
  `.tmp/fuzz/evidence/20261017T110000Z-canonical_sync_v2_decoders.json`
  (1,175 bytes, SHA-256
  `60b5dbc862b914eef257c37749b478acce76f94379f6306432949c87f28dbf5b`).
- Run log: 566,714 bytes, SHA-256
  `d96ddd1c6f830c1f1a1b7fa36a61085694d8735d66ae0c54ba3db3bc3d248e50`.
  Coverage log: 498,484 bytes, SHA-256
  `61b97c3560359699742e3d6c946cad0e54d9ff1c5c9e032225dc150f580d6829`.
- Coverage profile:
  `fuzz/coverage/canonical_sync_v2_decoders/coverage.profdata` (226,312 bytes,
  SHA-256
  `16e801dbd95013ac16e4bc413723a65ef3d243b0552c5160f5013614d253a723`).
- Pinned nightly `llvm-profdata show` accepted the profile: front-end
  instrumentation, 871 functions, maximum function count 193,715,260,
  maximum internal block count 9,996,470, 1,986 blocks, total count
  838,948,912.
- The tracked manifest is portable; ignored raw evidence is optional on another
  clone. This run intentionally does not satisfy the 28,800-second gate.
- The new smoke boundary and 23 affected historical boundaries passed normal
  and negative self-test modes. All 106 root JSON documents parse.
- Accepted-tag readiness passed 106/106 against immutable `M1.0.0`; rustfmt and
  workspace all-target/all-feature locked/offline Clippy with `-D warnings`
  passed.
- The first sustained-mode launch exited before workload start because the
  seed preparer still pinned the historical M1.1.74 campaign state. The
  preparer now accepts the exact M1.1.75 smoke observation while still
  rejecting claimed sustained completion or activation. Verify-only corpus,
  full preflight, and the harness boundary normal/self-test passed afterward.
- A subsequent launch exposed a file-system race during flat corpus reset: a
  listed file disappeared before `Remove-Item`. Reset now checks every exact
  parent path, rejects file reparse points, and treats an already-absent file
  as the successful postcondition while preserving failure for any path that
  still exists. Two consecutive reset/materialization runs and the boundary
  self-test passed.
- Process inspection then proved that an earlier detached sustained launch had
  survived its command host, so two runners were writing the same ignored
  corpus and artifact paths. Both exact process trees were stopped and that
  overlapping evidence was rejected. The runner now acquires an exclusive
  workspace-local file handle before any corpus reset and releases it in a
  `finally` block; a second runner fails before changing campaign state.

## M1.1.76 sustained decoder campaign (2026-10-18)

- The authoritative run started at `2026-10-17T12:02:20.6781480Z` from clean
  revision `eaf3d75bb16ce42c8368017a68c6f4183d15ab9f`; the same revision and a
  clean worktree were observed after completion.
- One Windows MSVC ASan worker/job ran for 28,801 seconds and completed
  469,853,771 executions. It began with 20 seeds, ended with 1,043 corpus
  files, produced zero failure artifacts, and exited successfully.
- Evidence record/log/coverage-log/profile hashes are tracked in
  `CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json`; all four ignored local files were
  re-hashed successfully during formalization.
- `llvm-profdata show` reported front-end instrumentation, 888 functions,
  2,010 blocks, maximum function count 204,308,962, maximum internal block
  count 10,543,038, and total count 885,327,009.
- The sustained decoder gate is now verified. Fresh hosted and physical-device
  evidence remains pending, and no candidate ALPN registration/dial or active
  runtime change is authorized by this result.
- The new boundary plus 25 affected historical boundaries passed normal and
  negative self-test modes. All 107 root JSON documents parsed; accepted-tag
  readiness passed 107/107 with rustfmt and locked/offline strict Clippy.

## M1.1.77–M1.1.80 sync v2 network evidence environment (2026-10-18)

- Evidence build command:
  `cargo build --offline --locked -p kilogram-cli --features sync-v2-evidence --bin kilogram-sync-v2-evidence --jobs 2`.
- Production control command:
  `cargo clippy --offline --locked -p kilogram-cli --bin kilogram-cli --jobs 2 -- -D warnings`.
  It does not compile or expose the evidence binary.
- Evidence strict Clippy command:
  `cargo clippy --offline --locked -p kilogram-cli --features sync-v2-evidence --bin kilogram-sync-v2-evidence --jobs 2 -- -D warnings`.
- New boundaries:
  `verify-kilogram-sync-v2-evidence-boundary.ps1` and
  `verify-kilogram-sync-v2-network-evidence.ps1 -SelfTest`; readiness boundary
  now closes an exact inventory of 108 scripts.
- The physical listener should use `--advertise-ip 192.168.0.140` on the main
  PC or the exact current LAN address returned by `ipconfig`. The option fails
  closed unless that address occurs exactly once in Iroh endpoint discovery.
- Strict relay control should pass
  `--relay-url https://aps1-1.relay.n0.iroh.link./` to the listener. The dialer
  consumes and pins the same URL from the signed ticket; no production relay
  configuration is changed.
- Laptop SSH identity: host `192.168.0.152`, Windows hostname
  `paul_hp_omen`, test user `test`. The stable staged program path is
  `C:\Users\test\KilogramLab\field-kit-current\kilogram-cli.exe`; its inbound
  UDP firewall rule is bound to that exact program and main-PC source address.
- Main-PC LAN address observed during preparation: `192.168.0.140`. Active
  `xray_tun` uses Wintun address `172.18.0.1` and currently causes even
  `ping 192.168.0.1` and `ping 192.168.0.152` to fail with `General failure`.
  This is an external test-environment blocker, not accepted protocol evidence.
- Before the physical run, verify no stale `kilogram-cli` or
  `kilogram-sync-v2-evidence` process on either host. Use fresh run/state
  directories and identical binary SHA-256 on both machines.
- No release build, ZIP, tag or automatic-on-push workflow is part of this
  stage. Both Windows workflows are manual-only.
- SSH must be probed outside the Codex network sandbox. On 2026-10-18 the
  outside-sandbox checks proved TCP/22, `plink` command execution and `pscp`
  upload/delete while the main-PC VPN remained enabled; no stale remote
  Kilogram process existed. The earlier sandbox `Test-NetConnection` failures
  are not authoritative LAN evidence.
- Windows PowerShell 5.1 on the main PC leaves `ExitCode` empty for a completed
  `Start-Process -PassThru` object even after `WaitForExit()` and `Refresh()`.
  Evidence controllers therefore use `System.Diagnostics.ProcessStartInfo`,
  asynchronously drain both output streams and read the native exit code from
  the direct Process object. Zero/non-zero runner probes must pass before a
  field or hosted run.
- Recursive `pscp` cannot traverse a 267-character local role-state path even
  with Windows long paths enabled. The ignored field output root is shortened
  to `.tmp/f/<experiment>`; the longest observed state path becomes 249
  characters without changing or omitting any state file.
- Rehearsal `m1180-field-20261018T033516Z` used two physical hosts and exact
  binary SHA-256
  `e23ba26692caf7fe355f04dda5f8825b0222b32ffe62794f0228bd35e7e1f625`.
  Direct, checkpoint/resume, pinned APS1 relay, mixed-version legacy,
  selected-v2 corruption with zero fallback and rollback all completed. The
  bounded manifest passed after the verifier was corrected to require the
  actual durable checkpoint: Alice count 2 / `SendEventBatch`, Bob count 1 /
  `ReceiveEventBatch`, then count 2 and one digest on both roles after restart.
- The rehearsal revision is not final evidence because the process runner and
  verifier fixes are tracked changes. Freeze and commit those fixes, rebuild
  the evidence binary, then collect final physical and manual hosted manifests
  from that same exact revision.
- Outside-sandbox SSH was reconfirmed on 2026-10-18 while the main-PC VPN was
  enabled: `test@192.168.0.152` returned host `paul_hp_omen`, and the LAN
  exception is sufficient for autonomous `plink`/`pscp` coordination.
- A second fresh physical run on revision
  `402e86861a4b4145d7947df36ea8be246cfd1031` completed the full matrix and
  produced `.tmp/f/m1180-field-20261018T034245Z/manifest.json`, SHA-256
  `ae873be47c1e755d95c6ba76cb2261dfc7c0b0e3fef0495dc3c78216dc9bedd8`.
  Later historical-boundary compatibility fixes require a final rerun on the
  next committed revision; the previous state directories must not be reused.
- Accepted-tag readiness after those compatibility fixes passed `108/108`,
  rustfmt and locked/offline strict Clippy. The full stable test command used
  profile `test`, two Cargo jobs, the isolated test before aggregate harnesses,
  normal process priority, `-Run` and `-NoFailFast`; it completed with
  `status=passed`. The legacy decoder mutation harness checked exactly 31 851
  cases in 2 552.55 seconds, and the sync-v2 mutation harness passed its exact
  2 863-case corpus in 23.28 seconds.
- The stable regression grew the retained debug cache to about 77.41 GiB,
  including 48.46 GiB of incremental data. No automatic vacuum is performed
  before the final evidence build because it would discard useful compilation
  cache; target remains ignored and is not evidence.
- Final pre-hosted physical run `m1180-field-20261018T051713Z` used revision
  `ad983147ad9c0435cb8bcf5b916bf7b0f6ba8b01`; direct, restart/resume, pinned
  relay, mixed-version, selected-v2 fail-closed and rollback verified. Manifest
  SHA-256 is `52f9dbbb54d267617d86f61010e2e3a85c83ebf8437b22b37830b3f199e9ab55`;
  evidence binary SHA-256 remains
  `e23ba26692caf7fe355f04dda5f8825b0222b32ffe62794f0228bd35e7e1f625`.
- Manual GitHub run `39114897550` on that exact revision passed clean build,
  evidence boundary, verifier self-test and process-runner probe, then failed
  before the direct scenario because orchestration pre-created the run
  directory used by the fail-closed Rust `prepare` command.
- The runner now keeps prepare logs outside the not-yet-created scenario,
  refuses an existing scenario, and its self-test performs an actual prepare
  lifecycle. It also no longer reads `$LASTEXITCODE` after invoking a `.ps1`;
  terminating verifier errors are the PowerShell failure contract.
- The corrected runner completed the entire local hosted matrix outside the
  Codex network sandbox, including a verified relay path. Because these tracked
  fixes change source revision, freeze a new commit and repeat final physical
  and hosted manifests on that same revision.
- Frozen source revision
  `b1f23c3719a0842979f047d874141953ae93ccac` is the accepted M1.1.80 evidence
  baseline. Physical manifest:
  `.tmp/f/m1180-field-20261018T093610Z/manifest.json`, 29,646 bytes, SHA-256
  `3b87c95c9298fead83ab6cf07d2c37d59e1d8e3336db1d85eb6841a8c9831b37`.
- Accepted hosted workflow run is `39116855502`. Downloaded manifest:
  `.tmp/m1180-hosted-39116855502/canonical-sync-v2-hosted-b1f23c3719a0842979f047d874141953ae93ccac/manifest.json`,
  29,618 bytes, SHA-256
  `d6382354c0327bb1aa900e7dc33536b467401f95edade1e9113f33e89e21e535`.
- The physical executable hash is
  `e23ba26692caf7fe355f04dda5f8825b0222b32ffe62794f0228bd35e7e1f625`;
  the hosted executable hash is
  `6a0add9561ce59e4c9b2d36a0ab3f81ab2fc5287df25905579f94c81ec031f9c`.
  Cross-builder binary equality is not claimed; exact equality on both
  physical roles and within the hosted topology is verified.
- `CANONICAL-SYNC-V2-NETWORK-EVIDENCE.json` is the portable compact record.
  Raw state, identities and logs remain ignored. Production `/cbor/2` default
  registration/dial remain false; no release, ZIP or tag was created.
- Final tracked-tree validation used
  `scripts/run-cargo-tests-stable.ps1 -Profile test -CargoJobs 2
  -IsolatedTestsFile .\scripts\windows-ci-isolated-tests.json
  -IsolatedTestOrder BeforeAggregate -NormalProcessPriority -Run -NoFailFast`
  outside the Codex filesystem sandbox so Rust tests could use the normal
  Windows temporary directory. The run ended with `status=passed`: the
  isolated runtime case passed `1/1`, legacy decoder mutations passed
  `31 851/31 851` in `2640.20s`, and canonical sync v2 decoder mutations
  passed `2 863/2 863` in `24.07s`; all remaining workspace harnesses passed.
- The same final tree passed all 27 changed boundary self-tests and accepted-tag
  readiness `108/108`, including rustfmt and locked/offline strict Clippy. A
  first in-sandbox test attempt was explicitly discarded after the sandbox
  redirected `%TEMP%` to a location that returned Windows `os error 5`; it is
  not product evidence and the valid outside-sandbox rerun had zero failures.

## M1.1.81 canonical sync v2 activation semantics environment (2026-10-18)

- `CanonicalSyncV2ActivationPermissions` является transport-independent и
  вычисляется из migration state, activation disposition и наличия persisted
  canonical evidence;
- policy разделяет шесть прав: legacy reader, capability publication, ALPN
  registration, accept new, initiate new и resume selected;
- normal `legacy-only` не публикует и не регистрирует `/cbor/2`;
  `advertise-cbor-v2` регистрирует reader до publication, но new accept/dial
  остаются false;
- containment без canonical evidence и canonical evidence в legacy/advertise
  state отвергаются. С evidence containment оставляет ALPN только для exact
  resume, запрещая новые sessions и legacy fallback;
- проверка:
  `cargo test -p kilogram-session activation_ --locked --offline --jobs 2`
  прошла `2/2`; новый boundary verifier с `-SelfTest` отверг publication без
  reader, fallback, containment без evidence, premature runtime activation и
  утечку evaluator в active transport;
- этот этап не менял CLI/Iroh, не запускал network, не создавал release, ZIP
  или tag. Следующий пакет — durable Device-bound local activation authority.

## M1.1.82 durable activation authority environment (2026-10-18)

- record format custom fixed-prefix v1: magic `KILOGRAM-CSAP-V1`, Ed25519
  Device signature, domain-separated BLAKE3 record ID, max reason 256 bytes;
- signed high-water `KILOGRAM-CSHW-V1` хранится рядом с state root, связывает
  exact epoch/head и публикуется atomic replace после append-only commit;
- runtime record path:
  `runtime/canonical-sync-v2-activation/<epoch>-<record-id>.csv2-activation`;
  он включён в runtime DB-primary read и typed append-only delta;
- новые команды: `canonical-sync-policy-status`,
  `canonical-sync-policy-plan`, `canonical-sync-policy-apply`;
- focused test:
  `cargo test -p kilogram-session canonical_sync_v2_activation --locked
  --offline --jobs 2` прошёл 5/5;
- полный regression изменённых crates вне sandbox прошёл: `kilogram-session`
  25/25 за 83.50 s, `kilogram-cli` 83/83 за 153.67 s. Предшествующий
  session-run внутри sandbox отклонён как infrastructure-only: 11 тестов,
  использующих Windows `%TEMP%`, получили известный `os error 5`;
- locked/offline strict Clippy для `kilogram-session` и `kilogram-cli` со всеми
  targets/features и `-D warnings` прошёл; rustfmt check зелёный;
- реальный CLI lifecycle прошёл на fresh filesystem state и на migrated
  Windows-DPAPI vault state. Искусственное удаление high-water после record
  commit заставило `status` fail closed, а byte-exact повторный `apply`
  восстановил только тот же planned record;
- verifier
  `verify-kilogram-canonical-sync-v2-activation-authority-boundary.ps1
  -SelfTest`, прежний activation-policy self-test и readiness boundary прошли;
  inventory теперь `closed-110`;
- полный `verify-kilogram-m1-readiness.ps1 -AcceptedTag M1.0.0` прошёл
  `110/110`, включая locked/offline workspace Clippy; две исторические
  dormant-проверки уточнены так, чтобы разрешать CLI только migration enum и
  permission evaluator, но по-прежнему запрещать wire state machine, ALPN
  registration и dial path;
- local high-water обнаруживает tail rollback только пока более новый marker
  сохранён. Coordinated rollback всего state directory вместе со старым
  marker требует независимого external/hardware witness и не заявлен закрытым;
- active Iroh transport не изменён; network, release, ZIP и tag не запускались.

## M1.1.83 production runtime canonical sync v2 dark launch (2026-10-18)

- `kilogram-cli` загружает signed activation authority до Iroh bind и выводит
  stable policy telemetry: state, epoch, containment, registration,
  publication, new accept/initiate, selected resume и evidence binding;
- default capability теперь создаётся через `issue_legacy_only`; canonical
  candidate появляется только при verified publish permission. Ticket rotation
  и directory refresh сохраняют текущую policy;
- Postcard wire получил bounded selection offer/accepted/rejected variants с
  пределом 256 KiB. Selection record version 1 хранится в
  `runtime/canonical-sync-v2-selections` и содержит обе подписи, оба Device
  certificates и оба Account authority snapshots;
- cross-Device selection epoch равен signed listener ticket publication epoch
  + 1. Независимые локальные activation epochs Alice и Bob не сравниваются;
- production canonical state живёт в `canonical-sync-v2-store`; legacy import,
  client/responder bootstrap, shared Iroh adapter и final materialization
  выполняются под существующим state lock/vault mirror boundary;
- `cargo check -p kilogram-cli --all-targets --all-features --locked --offline
  --jobs 2` прошёл; вне sandbox CLI tests прошли 85/85 за 153.93 s;
- local rehearsal `m1184-local-20261018T145634Z` использовал debug binary
  SHA-256 `d4301bd4b1145149da98cd4f7c87170709b9b47efdcb4842e8f2772a879ab878`:
  два legacy controls и один exact canonical run сошлись без fallback;
- `verify-kilogram-canonical-sync-v2-production-dark-launch-boundary.ps1
  -SelfTest` прошёл. Restart/resume, selected-path failure, containment и
  двухмашинный run остаются M1.1.84; release, ZIP и tag не создавались.

## M1.1.84 local canonical resilience rehearsal (2026-10-18)

- Debug build only, two Cargo jobs; release/ZIP/tag were not created.
- `cargo test -p kilogram-cli --bin kilogram-cli --locked --offline --jobs 2`
  must run outside the Codex Windows sandbox because its redirected Temp ACL
  makes filesystem tests fail with OS error 5. The unrestricted run passed
  `86/86` in `153.83s`.
- Build command:
  `cargo build -p kilogram-cli --bin kilogram-cli --locked --offline --jobs 2`.
- Successful local runner invocation used
  `scripts/invoke-kilogram-canonical-sync-v2-dark-launch-local.ps1` with a fresh
  output directory and run ID `m1184-resilience-20261018T152502Z`.
- Accepted debug binary SHA-256:
  `18fe1d407bb1a7c4d58fc739f1bf25c2e6c8a07604a71646f81a98baef690f6c`.
- The first containment attempt was intentionally not accepted after ratchet
  anti-rollback rejected reuse of generation-zero Bob prekeys. The runner now
  creates a fresh next-generation pool for its second conversation, and the
  complete fresh matrix then passed.
- A production-path field probe between the PC and laptop transferred responses
  of 1, 32, 256, 1024 and 8123 bytes intact. This ruled out N0/QUIC response
  size and Windows firewall as the source of the initial timeout.
- Root cause: the requester's 20-second stream deadline also covered remote
  state-lock, vault, store, legacy import and durable-session preparation. The
  runtime now uses a bounded 120-second first-response/preparation deadline and
  retains 20 seconds for individual stream I/O. A unit test pins the separation.
- Transport regression
  `bidirectional_response_larger_than_one_packet_is_delivered` passes with an
  8123-byte response; `kilogram-transport-iroh` passes 8/8.
- Exact-revision command used the committed debug executable from
  `7ee5ef808cb56c3c48d7a0b6dc07c0984b81992c` on both hosts through
  `scripts/invoke-kilogram-canonical-sync-v2-dark-launch-two-host.ps1`.
- Accepted two-host manifest:
  `.tmp/m1184-exact-20261018T165652Z/M1184-TWO-HOST-EVIDENCE.json`.
  Both `PAUL_HP_OMEN` and `DESKTOP-PAUL-RY` reported executable SHA-256
  `be8ca16754c2ad1291d4ea56c0a339b6e6a0f5994e6816c1c3d966871f42c532`
  and all six scenarios passed over direct Iroh LAN.
- M1.1.84 is complete. M1.1.85 is the remaining local aggregate regression and
  ledger/formalization gate.

## M1.1.85 opt-in dark-launch acceptance environment (2026-10-18)

- Aggregate contract: `CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json`; verifier:
  `scripts/verify-kilogram-canonical-sync-v2-opt-in-dark-launch-boundary.ps1`.
- Final readiness command used `-AcceptedTag M1.0.0` and passed `112/112`,
  rustfmt and locked/offline workspace strict Clippy. No test executable was
  started by readiness.
- Stable regression used profile `test`, `-CargoJobs 2`, isolated test order
  `BeforeAggregate`, normal process priority, `-Run` and `-NoFailFast`.
  It completed with `status=passed` and isolated runtime `1/1`.
- The only skips were
  `bounded_golden_decoder_mutation_corpus_fails_closed` and
  `bounded_canonical_sync_v2_decoder_mutation_corpus_fails_closed`. Their exact
  31 851 and 2 863 mutation corpora, plus sustained fuzz, remain accepted from
  the unchanged decoder/frame surface.
- The first regression invocation inside the Codex filesystem sandbox was
  discarded after redirected `%TEMP%` produced repeated Windows `os error 5`.
  The identical outside-sandbox rerun had zero failures.
- The stable runner retained the debug cache. Its trailing cache report showed
  about 92.2 GiB total target data; no automatic vacuum was performed because
  the accepted resource policy preserves useful incremental compilation state.
- Public default remains `kilogram/m0/sync/8`; `/cbor/2` is available only via
  verified local Device authority and mutual selection. No release, ZIP, tag
  or automatic GitHub workflow was created.

## M1.1.86 desktop activation environment (2026-10-19)

- Изменены только `kilogram-windows`, новый manifest/RFC и boundary scripts;
  wire codec, decoder, runtime endpoint и IPC protocol не менялись.
- Desktop activation запускает существующий debug `kilogram-cli.exe` только
  при stopped/disconnected runtime. CLI остаётся владельцем state lock, vault и
  Device signing key; GUI получает только проверенный текстовый результат.
- `cargo test -p kilogram-windows --lib --locked --offline --jobs 2` необходимо
  запускать вне filesystem sandbox: redirected Temp внутри sandbox даёт
  инфраструктурный Windows `os error 5` в существующих filesystem tests.
  Вне sandbox весь desktop suite прошёл 25/25 до дополнительного rights-matrix
  усиления; финальный прогон включает новые negative cases.
- Targeted all-target/all-feature locked/offline strict Clippy прошёл. Fresh
  CLI smoke в `.tmp/m1186-desktop-activation-smoke-20261019T003440Z` прошёл
  implicit legacy-only status, no-write plan, exact apply и verified signed
  epoch-one head `23aac6b46fc61e41673c2150c73c74695b5481432f9e2e061df97c534a7e8fdf`.
- Build policy: debug only, Cargo jobs 2. Network, release build, ZIP/archive,
  tag и automatic GitHub build не выполнялись.

## M1.1.87 runtime observability environment (2026-10-19)

- Изменены `kilogram-runtime-ipc`, production runtime adapter в
  `kilogram-cli`, Windows desktop и canonical IPC inventories. Peer transport,
  sync frame codec и decoder mutation surface не менялись.
- Local IPC exact-мигрировал на version 30 / descriptor domain v11. Старый v29
  request regression отвергается до actor dispatch; co-installed runtime,
  desktop и CLI должны обновляться вместе.
- Snapshot policy перечитывает durable authority под существующим state lock;
  observation ring process-local, capacity 32, drop-oldest, без persistence и
  без чувствительных network/content fields.
- Первый sandboxed IPC test run дал только известный infrastructure `os error
  5` из redirected Windows Temp. Тот же locked/offline suite вне sandbox прошёл
  17/17. CLI targeted test прошёл 1/1; desktop suite после добавления нового
  authenticated mock-request прошёл 26/26.
- Новый boundary self-test и 32 затронутых исторических boundaries прошли.
  Полный accepted-tag readiness завершился `114/114`; rustfmt и workspace-wide
  all-target/all-feature locked/offline strict Clippy также прошли.
  Build policy остаётся debug-only, Cargo jobs 2; network/release/ZIP/tag и
  automatic GitHub build не выполнялись.

## M1.1.88 guided restart environment (2026-10-19)

- Изменён только Windows desktop orchestration/state machine и formal
  manifest/RFC/boundary; runtime IPC, peer transport и sync codecs не менялись.
- `cargo test -p kilogram-windows --lib --locked --offline --jobs 2` запущен
  вне filesystem sandbox из-за известного redirected Temp ACL и прошёл 28/28.
- Pure regression покрывает exact success, wrong Device, record, epoch, rights,
  dishonest loaded/configured flags, ordered stop/inspect/apply/start/verify и
  сохранение exact expectation для retry.
- Новый boundary требует actual child exit до offline inspection, verification
  до conversations, fail-closed termination и запрещает hot apply/public
  default drift. Его negative self-test прошёл.
- Полный readiness: 115/115 против accepted tag `M1.0.0`, rustfmt и
  workspace-wide all-target/all-feature locked/offline strict Clippy.
- Build policy: debug only, Cargo jobs 2. Network, release build, ZIP/archive,
  tag и automatic GitHub build не выполнялись.

## M1.1.89 public rollout decision readiness (2026-10-19)

- Новый tracked decision:
  `CANONICAL-SYNC-V2-PUBLIC-ROLLOUT-DECISION.json`.
- Новый RFC:
  `docs/RFC-0213-m1189-canonical-sync-v2-public-rollout-decision.md`.
- Новый boundary:
  `scripts/verify-kilogram-canonical-sync-v2-public-rollout-decision-boundary.ps1`.
- Self-test отвергает восемь unsafe mutations: преждевременный public default,
  ложную eligibility, скрытый blocker, cohort по stable Device identity,
  automatic expansion, phase skip, selected legacy fallback и broad default
  без нового decision manifest.
- `verify-kilogram-runtime-ipc-legacy-mailbox-retirement-boundary.ps1` повторно
  прошёл после exact обновления canonical coverage priority.
- `verify-kilogram-m1-readiness-boundary.ps1` фиксирует closed inventory
  `116`; полный accepted-tag readiness прошёл `116/116`.
- Cargo jobs: 2; использован существующий debug target cache. Предупреждения о
  fallback incremental hard-link -> copy относятся к файловой системе и не
  ослабили strict Clippy.
- Rust source, peer/sync wire и runtime IPC не менялись; Rust test executables,
  network, release build, ZIP, tag и automatic GitHub build не запускались.

## M1.1.90 dormant rollout local authority (2026-10-19)

- Новый crate: `crates/kilogram-rollout`; workspace dependency добавляет
  cached `sha2 0.10.9`, отдельной загрузки registry не потребовалось.
- Targeted command:
  `cargo test -p kilogram-rollout --offline` — 12/12 плюс doc-tests.
- Targeted strict command:
  `cargo clippy -p kilogram-rollout --all-targets --all-features --locked --offline -- -D warnings`.
- Formal artifacts:
  `CANONICAL-SYNC-V2-ROLLOUT-LOCAL-AUTHORITY.json`,
  `docs/RFC-0214-m1190-rollout-local-authority.md`,
  `scripts/verify-kilogram-canonical-sync-v2-rollout-local-authority-boundary.ps1`.
- Boundary self-test отвергает public activation, runtime integration, stable
  user identity input, seed export, automatic upload, unbounded history,
  cohort-as-authority, broad default и rollback-security overclaim.
- Readiness inventory расширен до 117. Build policy остаётся debug-only,
  Cargo jobs 2; hard-link fallback warning относится к файловой системе target
  и не является compiler warning.
- Сеть, второй хост, release build, ZIP/archive, tag и automatic GitHub build
  не использовались.

## M1.1.91 N/N-1 production executable interop (2026-10-19)

- Exact field evidence:
  `.tmp/m1191-n-minus-one/evidence-attempt9/M1191-N-MINUS-ONE-EVIDENCE.json`;
  compact tracked copy: `CANONICAL-SYNC-V2-N-MINUS-ONE-INTEROP.json`.
- Accepted M1.0.0: revision `60b20b364c8df2324a0a287b4bcb6c056ae12c39`,
  SHA-256 `24c5681000af166cdbe63e61942c0d0dfbc5e1fca0772e257d99dd7984cde7c6`,
  57,975,296 bytes.
- Successor: revision `6d95cb73359f44f20908d52b4fd2eaf7869cdede`,
  SHA-256 `22265eb35c8a94f84ab3e83c67d12235080e80c9e8f81fc56f60362897e1e13e`,
  60,089,856 bytes.
- Hosts: `DESKTOP-PAUL-RY` and `PAUL_HP_OMEN`; 4/4 direct/relay direction
  matrix and forward upgrade chain passed with exact history equality.
- M1.0.0 imported four events in about 53.8 seconds. Server follow-up accept is
  now separately bounded at 120 seconds; individual wire I/O limits remain.
- Harness uses stable local `.tmp/kilogram-field-runner/kilogram-cli.exe` and
  remote `KilogramLab/n-minus-one-stable/kilogram-cli.exe` paths. Never run
  network Rust tests via raw `cargo test` on Windows; use
  `scripts/run-cargo-tests-stable.ps1` to avoid recurring hash-path firewall
  prompts.
- Build profile debug, target x86_64-pc-windows-msvc, Cargo jobs 2. Raw logs
  and state stay ignored. Release/ZIP/tag were not created.

## M1.2.5 complete rollout rehearsal environment (2026-10-19)

- Trusted rollout health is now anchored by the offline release root and
  synchronized from the complete verified trusted-policy chain; an operational
  key rotation no longer strands the health store.
- The named deterministic rehearsal covers five policy bundles, three explicit
  Device-approved transitions, containment, recovery, replay and foreign,
  stale, truncated and forked negatives. Rollout suite size is 56 tests.
- CLI approval/recovery apply can repair only the interrupted boundary where
  the exact Device-signed activation record is durable but its high-water is
  not. The focused regression passed with TEMP/TMP redirected to
  `.tmp/verification-temp`; the first sandbox run against system TEMP failed
  only with Windows `os error 5`.
- Two-host harness uses stable names, a verification-profile rollout test
  executable and debug CLI. Its disposable state belongs under ignored
  `.tmp/rollout-field`; tracked output is bounded JSON evidence only.
- Build policy remains jobs 2, no release build, ZIP/archive or tag. Windows
  test executables are launched only through the stable-path runner to avoid
  recurring Firewall prompts.
- Full stable-name workspace regression passed outside the socket-restricted
  sandbox. It includes the isolated live-runtime revocation test, CLI 91/91,
  protocol 156 passed plus 15 explicit vector-regeneration ignores, legacy
  decoder mutations 31,851/31,851 in 2534.98 s, canonical sync v2 decoder
  mutations 2,863/2,863, rollout 56/56, session 25/25 and Windows 30/30.
- Exact two-host run: `m1205-rollout-20261019T071251Z`, implementation revision
  `2f005c124a8333af4cd7a09bd36dee93cb1111a8`.
- The verification harness was 5,033,984 bytes with SHA-256
  `8a0733cf380367fc7696bf815463967cccfc816a63ae7281288021842d06a10a`;
  the debug CLI was 61,408,256 bytes with SHA-256
  `56bfdd7900084a09080b1e19b2dc9b3897316a95e2c3d6d0ab76cf859a33ef01`.
- Both physical Windows hosts passed rollout 56/56, the named end-to-end test,
  absent-store observer and operator observer (`hold-without-create`). SSH was
  only the control/artifact-transfer channel; no P2P listener was created.
- Bounded tracked record:
  `CANONICAL-SYNC-V2-ROLLOUT-END-TO-END-TWO-HOST-EVIDENCE.json`. Raw logs,
  state, Account/Device identifiers and secrets remain ignored.

## M1.2.6 peer capability transport enforcement environment (2026-10-19)

- Implementation is in the existing `kilogram-cli sync` path and reuses
  `kilogram-peer-capability-store`; no dependency, wire or runtime IPC change.
- The enforcement call is ordered after authenticated ticket/authority/
  membership processing and before `endpoint_builder_for_remote(...).bind()`.
- Focused Rust tests were compiled without execution and launched only through
  `scripts/run-cargo-tests-stable.ps1`; 4/4 passed using the stable verification
  harness name.
- Full CLI regression through the same stable path passed 96/96 binary tests
  and 1/1 library test outside the socket-restricted sandbox. The first
  sandboxed full attempt was stopped after expected loopback socket denials;
  it was not treated as product evidence.
- Aggregate M1 readiness passed 132/132, including rustfmt and workspace-wide
  locked/offline strict Clippy. The only compiler messages were the known
  target-filesystem incremental hard-link-to-copy warnings.
- The sandbox cannot write test temp state under the system TEMP (`os error
  5`), so TEMP/TMP were redirected to ignored `.tmp/verification-temp`; this
  does not alter product behavior or test assertions.
- Build profile remains verification/debug with Cargo jobs 2. No release build,
  archive, tag, second host or network was used for this local milestone.

## M1.2.7 peer capability two-host rehearsal environment (2026-10-19)

- Accepted run: `m1207-capability-20261019T101809Z`, source revision
  `120fa77ff7e832fb9a285f72dfd377ee39825296`.
- Stable local path: `.tmp/kilogram-field-runner/kilogram-cli.exe`; remote path:
  `KilogramLab/field-kit-current/kilogram-cli.exe`. Exact SHA-256 on both hosts:
  `1b58772af5ebbee4c46c929f78b2a822fc39c9dcfef47a3f50dbf0d3fb2c300b`,
  size 61,414,912 bytes, debug `x86_64-pc-windows-msvc`.
- Product traffic used direct Iroh LAN between two physical Windows hosts. SSH
  was only the control/copy channel. Three base pairs and all three field
  scenarios passed.
- First attempt exposed an obsolete historical containment expectation after
  the new durable floor; the helper now has explicit basic-pair-only mode.
  Second attempt exposed plain-JSON handling in the new harness; actual tickets
  are base64url JSON envelopes and now round-trip explicitly.
- Raw directories remain ignored under `.tmp/m1207-field`; the tracked evidence
  contains no raw log, state, ticket, address, selection/Account/Device ID or
  secret. No release build, ZIP/archive or tag was created.
- Aggregate readiness is 133/133. M1.2.8 should bring the same enforcement into
  automated runtime outbound sync without automatic quarantine recovery.

## M1.2.8 automated runtime enforcement environment (2026-10-19)

- Implementation remains inside the ordinary `kilogram-cli` runtime and reuses
  the M1.2.6 conversation-scoped capability authorizer and durable
  `kilogram-peer-capability-store`; there is no dependency or wire-format bump.
- Outbox preflight is ordered before sealed-body open, ratchet/event
  materialization, endpoint use and mailbox fallback. Scheduled sync preflights
  every authority-current endpoint candidate before invoking `sync`.
- Process-local blocked queue/contact sets prevent busy retry and let unrelated
  contacts progress. They are deliberately not durable recovery authority;
  restart re-evaluates the Device-signed durable floor.
- Focused Rust tests are compiled with `cargo test --no-run` and executed only
  through `scripts/run-cargo-tests-stable.ps1`, keeping stable Windows Firewall
  executable paths. Build profile remains debug/verification with Cargo jobs 2;
  no release build, ZIP/archive, tag or second-host run is required.
- The new boundary is
  `verify-kilogram-canonical-sync-v2-automated-runtime-enforcement-boundary.ps1`;
  its normal and mutation self-test modes passed. Focused stable tests passed
  5 peer-capability + 1 outbox/restart + 1 scheduler/fairness cases.
- Full stable-path CLI regression passed 99/99 binary and 1/1 library tests in
  596.81 seconds. Accepted-tag readiness passed 134/134 against immutable
  `M1.0.0`, including rustfmt and workspace-wide locked/offline strict Clippy.
  Runtime IPC, automatic quarantine recovery and the public `legacy-only`
  default remain unchanged.

## M1.2.9 runtime quarantine observability environment (2026-10-19)

- Runtime IPC is now exact v31 with descriptor domain v12; co-installed v30
  binaries are rejected rather than dual-read. Peer/sync network wire is
  unchanged.
- Capability observation uses `redb::ReadOnlyDatabase` and never initializes a
  missing store. Tests preserve the pre-observation DB bytes and reject foreign
  owner binding.
- Windows tests continue to compile with `cargo test --no-run` and execute only
  through `scripts/run-cargo-tests-stable.ps1`, so test harness EXE paths stay
  stable and do not trigger new Firewall rules.
- A sandboxed aggregate attempt compiled successfully but system `%TEMP%` was
  outside the writable sandbox, causing unrelated tests to fail with Windows
  `os error 5`. Product evidence comes from the same stable runner outside that
  filesystem isolation.
- Verification remains debug/verification, Cargo jobs 2, no release build,
  ZIP/archive, tag, network run or second physical host. Aggregate readiness is
  135/135 and the new fail-closed boundary rejects 7/7 negative mutations.
  Stable-path execution passes 157/157 tests: CLI 100 + library 1, peer
  capability store 8, runtime IPC 18 and Windows library 30.

## M1.2.10 peer re-enrollment planning environment (2026-10-19)

- Added workspace crate `kilogram-peer-reenrollment`; it has only offline
  identity/capability dependencies and no redb writer, runtime, IPC, Iroh,
  Tokio, CLI or desktop dependency.
- Windows Account Root test fixtures use protected root-key creation. Inside
  the filesystem sandbox this returns `os error 5`; the same stable-name test
  harness must therefore execute outside that sandbox. This is an environment
  permission constraint, not a product test failure.
- Focused stable-path verification uses profile `test`, Cargo jobs 2 and fixed
  executable names. Results: peer capability store 8/8 and peer re-enrollment
  5/5. Direct hash-named `cargo test` remains forbidden by AGENTS.md.
- Boundary normal/self-test passed with 7/7 rejected mutations. Accepted-tag
  aggregate readiness passed 136/136 against immutable `M1.0.0`, including
  rustfmt and workspace-wide `--locked --offline -D warnings` Clippy.
- No release build, ZIP/archive, tag, automatic GitHub build, network test or
  second-host run is required for this pure offline state-machine milestone.

## M1.2.11 durable peer re-enrollment acceptance environment (2026-10-19)

- Added workspace crate `kilogram-peer-reenrollment-acceptance`; the original
  planner remains independent of redb. The acceptance crate has redb and local
  identity/capability dependencies but no runtime IPC, Iroh, Tokio, HTTP,
  Windows UI or network dependency.
- Windows Account Root fixtures again require the stable-name harness outside
  the filesystem sandbox because protected root-key creation otherwise returns
  `os error 5`. Direct hash-named `cargo test` was not used.
- Focused stable-path profile `test`, Cargo jobs 2: planner 5/5 and acceptance
  5/5. The acceptance group includes exact old capability database byte
  preservation and reopen after injected pre-commit failure.
- New boundary normal/self-test passes with 8/8 rejected mutations. Aggregate
  accepted-tag readiness passes 137/137 against immutable `M1.0.0`, including
  rustfmt and workspace-wide `--locked --offline -D warnings` Clippy.
- Existing Cargo target cache was retained for build speed despite its
  approximately 47 GiB warning; no full clean, release build, ZIP/archive, tag,
  automatic GitHub build, network test or second host was used.

## M1.2.12 fresh-start peer re-enrollment consumption environment (2026-10-19)

- Added workspace crate `kilogram-peer-reenrollment-runtime`; it uses identity,
  acceptance, sync-capability, postcard and redb only. It has no Iroh, Tokio,
  HTTP, runtime IPC, Windows UI or network dependency.
- Runtime consumption is called from the ordinary CLI startup before
  `endpoint_builder_with_relay` and endpoint bind. Full context preflight occurs
  before any consumed head is committed.
- Windows Rust execution used only `scripts/run-cargo-tests-stable.ps1` with
  Cargo jobs 2. In the restricted tool sandbox, `TEMP` and `TMP` were pointed
  at workspace `.tmp/rust-tests`; the initial default-temp `os error 5` was an
  environment write restriction, not a product failure.
- Focused stable-path results: planner 5/5, acceptance 5/5 and runtime
  transition store 5/5. Full CLI stable-path regression passes 101/101 binary
  plus 1/1 library, including the final hardened fresh-start integration test
  (690.02 s).
  The focused CLI scenario was repeated after adding monotonic post-consumption
  local/peer authority advancement and replacement-revocation validation; 1/1
  passed outside the filesystem sandbox (inside it `%TEMP%` returned expected
  `os error 5`). Boundary normal/self-test passes with 10/10 rejected mutations; aggregate
  readiness passes 138/138 with rustfmt and workspace-wide locked/offline
  strict Clippy.
- The first full CLI execution inside the restricted network sandbox passed
  88 tests and rejected 13 loopback/IPC tests with Windows `os error 10013`.
  Re-running the identical stable EXE outside only that network sandbox passed
  all 102 tests; no source or fixture was changed between the runs.
- No release build, ZIP/archive, tag, automatic GitHub build, network test or
  second physical host was used. Existing target cache was retained.

## M1.2.13 two-host peer re-enrollment environment (2026-10-19)

- The field harness is
  `scripts/invoke-kilogram-peer-reenrollment-two-host-rehearsal.ps1`. It first
  runs the accepted M1.2.7 capability prerequisite and then reuses its fresh
  quarantined pair. Every run uses a unique ignored local/remote directory.
- The exact debug binary from revision `d307e965...` is 62,375,424 bytes with
  SHA-256 `31e3b6e...`. Local and laptop copies use the stable paths
  `.tmp/kilogram-field-runner/kilogram-cli.exe` and
  `KilogramLab/field-kit-current/kilogram-cli.exe`; no hash-named network test
  executable was started and no new Firewall identity is expected.
- The first diagnostic run reached the final sync and failed correctly because
  six predecessor-authored events targeted Alice's account and the replacement
  Bob Device had no local projection. The test was fixed with explicit offline
  `history-rewrap-export/import`; recipient-account validation was not weakened.
- Accepted run `m1213-field-20261019T171026Z` passed descriptor substitution,
  fresh-start consumption, direct delivery/ACK, restart replay and final sync.
  Histories match at 8 events and the queued body occurs exactly once.
- Raw SSH/process logs, state, tickets, IDs and addresses remain under ignored
  `.tmp`. Only bounded evidence is tracked. Laptop VPN was not required; LAN
  was sufficient because route selection was not under test.
- Boundary normal/self-test rejects 6/6 mutations; aggregate readiness is
  139/139 with rustfmt and locked/offline strict Clippy. The preceding exact
  binary regression remains 101/101 CLI binary plus 1/1 library and runtime
  transition store 5/5. No release, ZIP/archive, tag or automatic GitHub build
  was created.

## M1.2.14 dormant runtime IPC peer re-enrollment contract (2026-10-19)

- Implementation is isolated in
  `crates/kilogram-runtime-ipc/src/peer_reenrollment_contract.rs`; it is
  re-exported as a library contract but is absent from the active
  `RuntimeIpcCommand` and `RuntimeIpcResponse` enums. `IPC_VERSION=31` and
  descriptor signature domain v12 remain unchanged.
- The codec adds the already locked workspace `blake3` dependency to
  `kilogram-runtime-ipc` for request, descriptor and privacy-safe Device
  bindings. Request limit is 4 KiB, signed response limit 16 KiB, canonicality
  is exact Postcard re-encoding.
- Targeted stable runner command used package `kilogram-runtime-ipc` with
  filter `peer_reenrollment_contract`; 8/8 tests passed. Full stable-name crate
  regression passed 26/26 outside the restricted filesystem sandbox. The
  first identical full run inside the sandbox produced `os error 5` only for
  pre-existing tempfile/loopback tests under the sandbox-specific `%TEMP%`;
  this was an environment ACL boundary, not a product failure.
- Contract boundary normal/self-test passed and rejected 8/8 semantic
  mutations. Accepted-tag readiness passed 140/140 with rustfmt and
  workspace-wide locked/offline strict Clippy. A preceding readiness run
  without `-AcceptedTag M1.0.0` correctly rejected post-M1 source drift against
  the historical candidate manifest; post-M1 verification must use the
  accepted-tag mode.
- No network, laptop, release build, archive, tag or GitHub build was used.
  Test execution kept the stable executable path
  `target/verification/deps/kilogram-runtime-ipc-lib-kilogram_runtime_ipc-tests.exe`.

## M1.2.15 read-only runtime peer re-enrollment adapter environment (2026-10-20)

- Active local IPC is now exact version 32 with descriptor signature domain
  v13. Co-installed version 31 clients fail before actor dispatch; peer and
  sync network wire formats are unchanged.
- The new staging crate has no Iroh, HTTP, Tokio or desktop dependency. It
  writes only after explicit offline import and opens through a strict
  read-only path for runtime observation. Missing stores remain absent.
- Windows Rust harnesses were compiled without execution and run only through
  `scripts/run-cargo-tests-stable.ps1` with fixed executable names, profile
  `test`, two Cargo jobs and workspace-local `TEMP`/`TMP`.
- Focused stable-path results were CLI 2/2, staging 3/3, peer re-enrollment IPC
  8/8 and desktop authenticated IPC 1/1. The complete changed-package run
  passed isolated runtime 1/1, CLI binary 101/101 plus library 1/1, staging
  3/3, runtime IPC 26/26 and Windows library 30/30.
- The first desktop IPC attempt inside the restricted network sandbox failed
  with Windows `WSAEACCES` 10013. The identical stable executable passed
  outside that loopback restriction; no source or fixture changed between
  attempts.
- The dedicated boundary rejects 8/8 mutations. Accepted-tag readiness passes
  141/141 against immutable `M1.0.0`, including rustfmt and workspace-wide
  locked/offline strict Clippy.
- The existing Cargo target reached roughly 60.8 GiB after regression because
  this filesystem cannot hard-link incremental artifacts. Disposable
  verification and debug incremental generations are pruned after validation;
  reusable debug deps/build outputs remain for faster subsequent builds.
- No product network operation, laptop, release build, archive, tag or
  automatic GitHub build was used. Public sync default remains `legacy-only`.

## M1.2.16 fresh-start peer re-enrollment mutation adapter environment (2026-10-20)

- Active local IPC is exact version 33 with descriptor signature domain v14;
  version 32 is rejected before actor dispatch. Peer and sync wire formats are
  unchanged.
- Rust harnesses were compiled without execution and launched only by
  `scripts/run-cargo-tests-stable.ps1` under stable EXE names. Cargo used the
  debug/test profile, two jobs and below-normal priority; no release artifact
  or ZIP was produced.
- Changed-package stable regression passed CLI binary 102/102 plus library
  1/1, peer re-enrollment 5/5, acceptance 5/5, runtime/journal 7/7, runtime IPC
  27/27 and Windows library 30/30. The focused crash/replay scenario also
  passed after the full run.
- The crash test exposed two implementation defects before acceptance: exact
  transition replay used wall-clock preparation time as identity, and the
  acceptance redb handle remained open during response collection. Replay now
  compares immutable operation context without timestamp, and store handles
  are released before reopening read views.
- Dedicated mutation boundary rejects 10/10 negative mutations. Accepted-tag
  readiness passes 142/142 against immutable `M1.0.0`, including rustfmt and
  workspace-wide locked/offline strict Clippy.
- Full regression grew `target` to about 45.6 GiB because this filesystem
  copies incremental artifacts instead of hard-linking them. Disposable debug
  incremental generations may be pruned after validation without removing
  reusable debug dependencies and build outputs.
- No product network operation, laptop, release build, archive, new tag or
  automatic GitHub build was used. Public sync default remains `legacy-only`.

## M1.2.17 guided desktop peer re-enrollment environment (2026-10-20)

- Windows GUI now uses the existing authenticated IPC v33/domain v14 for an
  explicit status/plan/approve/accept ceremony. The verified descriptor is
  canonically encoded and hashed as an internal UI binding; signed responses
  are verified for every operation.
- The desktop state machine holds only opaque IDs, bounded summaries and the
  confirmation phrase. Any contact, signed response, plan or descriptor drift
  clears typed consent. No secret, path, ticket, endpoint or reusable
  capability is projected into that state machine.
- Guided restart is limited to a connected runtime owned by the desktop. The
  old child receives authenticated shutdown, a fresh process is created, and
  exact receipt plus transition consumption is checked before success. A
  mismatch or even failure to submit the verification request kills the fresh
  child and retains only a bounded retry expectation.
- Windows Rust harnesses were compiled without execution and launched only
  through `scripts/run-cargo-tests-stable.ps1` at fixed EXE paths, debug
  profile and two Cargo jobs. Full Windows library regression passed 36/36,
  including five pure ceremony tests and one authenticated loopback sequence.
- The new boundary rejected 10/10 negative mutations; predecessor read and
  mutation boundaries still pass 8/8 and 10/10. Accepted-tag readiness passes
  143/143 against immutable `M1.0.0`, including rustfmt and workspace-wide
  locked/offline strict Clippy.
- No second host, product network, release build, ZIP/archive, new tag or
  automatic GitHub build was used. Existing debug dependencies were retained;
  public sync default remains `legacy-only`.

## M1.2.18 explicit old-history recovery plan environment (2026-10-20)

- Added the pure leaf crate `kilogram-peer-history-recovery`; it depends only
  on existing identity, account-authority, event-log, history-rewrap and
  peer-reenrollment primitives. It has no Tokio, Iroh, HTTP, redb, desktop or
  runtime dependency and exposes no execution method.
- Windows tests were compiled without execution and launched through
  `scripts/run-cargo-tests-stable.ps1` as the fixed executable
  `target/debug/deps/kilogram-peer-history-recovery-lib-kilogram_peer_history_recovery-tests.exe`.
  The six tests passed under the debug/test profile with two Cargo jobs.
- The first stable invocation inherited the default sandbox TEMP directory and
  all fixtures failed to create with Windows `os error 5`. Re-running the same
  stable executable with TEMP/TMP inside workspace `.tmp/rust-tests` passed
  6/6; this was an environment write restriction, not a planner failure.
- Dedicated boundary verification passes and rejects 10/10 negative
  mutations. It proves body-free bounded inventory, separate source consent,
  consumed-transition binding and absence of store/network/desktop/IPC
  authority.
- Accepted-tag readiness passes 144/144 against immutable `M1.0.0`, including
  rustfmt and workspace-wide locked/offline strict Clippy.
- No product network operation, second host, release build, ZIP/archive, tag
  or automatic GitHub build was used. Active IPC remains v33/domain v14 and
  public sync default remains `legacy-only`.

## M1.2.19 durable old-history recovery consent environment (2026-10-20)

- Added `kilogram-peer-history-recovery-consent`, a separate owner-bound redb
  store with immediate durability, immutable source-Device-signed records and
  one exact head per consumed re-enrollment transition.
- First commit reuses the M1.2.18 verifier over the live authority,
  memberships, body-free inventory, ranges, checkpoints and consumed
  transition. Exact committed replay is checked before live expiry so durable
  evidence remains idempotently readable after the original plan expires.
- Windows tests were compiled without execution and launched only through
  `scripts/run-cargo-tests-stable.ps1` at the fixed executable path, debug/test
  profile and two Cargo jobs. All six store tests passed.
- Dedicated boundary verification rejects 10/10 mutations. Accepted-tag
  readiness passes 145/145 against immutable `M1.0.0`, including rustfmt and
  workspace-wide locked/offline strict Clippy.
- No product network operation, second host, release build, ZIP/archive, tag
  or automatic GitHub build was used. Whole-database rollback detection
  without an external witness is not claimed; active IPC remains v33/domain
  v14 and public sync default remains `legacy-only`.

## M1.2.20 bounded single-source history execution environment (2026-10-20)

- Added the local/offline crate `kilogram-peer-history-recovery-execution` and
  reused existing protocol history-rewrap bundles, local encrypted projections
  and recipient-signed checkpoints. No Iroh, Tokio, HTTP, runtime IPC or GUI
  dependency was added.
- The execution redb is separate from consent and product history state. It is
  Account/source-Device-bound, immediate-durability and append-only, with exact
  bundle/checkpoint/projection artifacts and one head per consent selection.
- The short plan expiry is still enforced by the initial M1.2.19 consent
  commit. M1.2.20 adds an explicit committed-context verifier so an exact
  durable consent can resume after expiry only while authority, membership,
  inventory, ranges, source and recipient remain unchanged.
- Focused tests were compiled without execution and then launched only through
  `scripts/run-cargo-tests-stable.ps1` under the stable EXE name. The first
  sandboxed run hit Windows `Access Denied` while creating protected Account
  Root key files; the identical stable harness passed 6/6 outside the file
  sandbox. This was not a network or firewall failure.
- The six tests cover atomic commit/reopen, three bounded resume chunks, exact
  replay/divergence, injected crash rollback, foreign/context drift, and
  corruption/orphan detection. The affected stable-path regression passed
  planner 6/6, consent 6/6 and execution 6/6; protocol passed 156 tests with
  15 explicit generators ignored, the 31,851-case decoder mutation corpus
  passed in 2575.21 seconds, and sync-v2 decoder mutations passed 1/1 in 23.87
  seconds. Dedicated boundary rejects 10/10 mutations; accepted-tag readiness
  passes 146/146 with rustfmt and workspace-wide locked/offline strict Clippy.
- Cargo jobs remained 2 for compilation. No second host, product network,
  release build, archive, Git tag or automatic GitHub build was used. Public
  sync default remains `legacy-only`.

## M1.2.21 atomic recipient history-store import environment (2026-10-20)

- The existing `kilogram-peer-history-recovery-execution` crate now has a
  local/offline import module using `kilogram-store`, `kilogram-state` and
  `tempfile`; no Iroh, Tokio, HTTP, runtime IPC or GUI dependency was added.
- Import writes the ordinary `events` and `local-messages` stores, both signed
  checkpoints and the recipient Device-signed receipt under one
  `StateTransaction` and state-directory lock. Existing encrypted vaults use
  the established DB-primary shadow/dual-write lifecycle.
- Deep stable-test state roots exposed Windows error 3 for descriptive
  checkpoint filenames longer than MAX_PATH. Checkpoints now use their full
  content-addressed ID as a short filename; signed content and verification are
  unchanged.
- Windows tests were compiled without execution and launched through
  `scripts/run-cargo-tests-stable.ps1` at the stable executable path. The
  combined execution/import harness passed 10/10 in 252.28 seconds.
- Dedicated boundary verification rejects 10/10 mutations. Accepted-tag
  readiness passes 147/147 with rustfmt and workspace-wide locked/offline
  strict Clippy.
- No product network operation, second host, release build, ZIP/archive, tag or
  automatic GitHub build was used. Whole-database rollback detection without
  an external witness is not claimed; active IPC remains v33/domain v14 and
  public sync default remains `legacy-only`.

## M1.2.22 authenticated history-recovery transfer inbox environment (2026-10-20)

- The existing `kilogram-peer-history-recovery-execution` crate now includes a
  local/offline transfer module. It uses canonical postcard envelopes, existing
  Device signatures, `StateDirectoryLock`, `StateTransaction`, the encrypted
  vault dual-write lifecycle and `tempfile`; no Iroh, Tokio, HTTP, runtime IPC
  or GUI dependency was added.
- The portable envelope is capped at 9 MiB and carries the exact source-signed
  execution record plus bundle, signed checkpoints and encrypted projections.
  The recipient inbox stores exact content-addressed bytes under
  `history-recovery/transfer-inbox` and caps the complete inventory at 16,384.
- A linear three-generation backlog is allowed before import, but the existing
  recipient-signed receipt chain still rejects out-of-order consumption. Path
  aliases are canonicalized before allowing the inbox/import bridge to share a
  state directory.
- Windows tests were compiled without execution and launched through
  `scripts/run-cargo-tests-stable.ps1` at the stable executable path. The four
  transfer tests and all 14 combined execution/import/transfer tests passed.
- Dedicated boundary verification rejects 10/10 mutations. Accepted-tag
  readiness passes 148/148 with rustfmt and workspace-wide locked/offline
  strict Clippy.
- No product network operation, second host, release build, ZIP/archive, tag or
  automatic GitHub build was used. Whole-database rollback detection without
  an external witness is not claimed; active IPC remains v33/domain v14 and
  public sync default remains `legacy-only`.

## M1.2.23 authenticated Iroh history-transfer session environment (2026-10-20)

- Added `kilogram-peer-history-recovery-transport` as a separate crate with
  Iroh/Tokio. The offline `kilogram-peer-history-recovery-execution` crate and
  active CLI/Windows app manifests remain free of this transport authority.
- The fixed test fixture is the exact valid M1.2.22 canonical envelope. Tests
  bind both Iroh Endpoint IDs, source/recipient Devices, a nonzero nonce and the
  content-addressed transfer ID; the response is signed only after durable
  recipient staging.
- Sandboxed loopback execution timed out because local networking is restricted
  there. The same stable-path executable was run outside that network sandbox
  and passed over exact `127.0.0.1`; the pre-existing Iroh loopback control also
  passed. This is host-local transport evidence, not product Internet evidence.
- Windows network tests were compiled without execution and launched through
  `scripts/run-cargo-tests-stable.ps1`; no hash-named harness was launched and
  no new Windows Firewall executable identity was introduced.
- Focused regression passed 4/4 and the pre-existing loopback control passed
  1/1. Dedicated boundary verification rejects 10/10 mutations. Accepted-tag
  readiness passes 149/149 with rustfmt and workspace-wide locked/offline strict
  Clippy.
- No second host, product Internet/relay, release build, ZIP/archive, tag or
  automatic GitHub build was used. Active IPC remains v33/domain v14 and public
  sync default remains `legacy-only`.

## M1.2.24 durable history-transfer outbox environment (2026-10-20)

- The existing history-recovery transport crate now also depends on the already
  locked `redb`; no dependency was added to active CLI/Windows app manifests.
- The outbox database is Account/source-Device-bound, capped at 16,384 intents,
  and uses immediate transactions for request+head and receipt commits. Its
  read-only API opens an existing database without creating one.
- The stable Windows runner executed only the `outbox_` filter for focused local
  persistence tests, then the full eight-test crate through the same permanent
  executable path. No hash-named Cargo harness was launched directly.
- Four focused tests cover restart/exact bytes, acknowledgement replay,
  competing intent/receipt and injected rollback before both commit types.
  Dedicated boundary rejects 10/10 mutations; accepted-tag readiness passes
  150/150 with rustfmt and workspace-wide locked/offline strict Clippy.
- No product network, second host, release build, ZIP/archive, tag or automatic
  GitHub build was used. Active IPC remains v33/domain v14 and public default
  remains `legacy-only`.

## M1.2.25 explicit restart-safe history-transfer executor environment (2026-10-20)

- The executor lives in the existing dormant history-recovery transport crate;
  no dependency or command was added to active CLI/Windows application crates.
- Its public input is a caller-owned Iroh `Endpoint`, exact target
  `EndpointAddr`, one durable transfer ID and a bounded policy. The policy
  defaults to three attempts and is capped at eight; no timer or schedule is
  persisted.
- Deterministic tests inject pre-connect and post-connect-ambiguous failures
  around the exact prepared bytes. One real network test uses only explicit
  `127.0.0.1` addresses and the dedicated history-recovery ALPN.
- Windows tests are compiled without execution and launched through
  `scripts/run-cargo-tests-stable.ps1` at the permanent test EXE path. Focused
  executor regression passes 6/6 and the full crate passes 14/14.
- Dedicated boundary rejects 10/10 mutations; accepted-tag readiness passes
  151/151 with rustfmt and workspace-wide locked/offline strict Clippy.
- No product Internet/relay, second host, release build, ZIP/archive, tag or
  automatic GitHub build was used. Active IPC remains v33/domain v14 and
  public default remains `legacy-only`.

## M1.2.26 explicit bounded recipient acceptor environment (2026-10-20)

- The acceptor lives in the existing dormant history-recovery transport crate;
  active CLI/Windows app manifests remain free of that dependency and active
  IPC remains v33/domain v14.
- Its public input is a borrowed caller-owned Iroh endpoint, recipient Device,
  durable recipient inbox and bounded policy. It processes sequentially and
  has no spawned/background lifetime.
- Request work and durable completion use separate deadlines. This allows a
  short unauthenticated request bound without truncating the bounded durable
  inbox stage; every deadline is capped at 300 seconds.
- One real authenticated loopback session exercises Iroh ALPN, request
  validation, durable staging and signed response. Multi-session rejection and
  timeout continuation are deterministic control-flow tests because replacing
  a deliberately stalled Iroh loopback connection immediately is cache-sensitive.
- Windows tests are compiled without execution and launched through
  `scripts/run-cargo-tests-stable.ps1` at the permanent test EXE path. Focused
  acceptor regression passes 5/5 and the full crate passes 19/19.
- Dedicated boundary rejects 10/10 mutations; accepted-tag readiness passes
  152/152 with rustfmt and workspace-wide locked/offline strict Clippy.
- No product Internet/relay, second host, release build, ZIP/archive, tag or
  automatic GitHub build was used. Public default remains `legacy-only`.

## M1.2.27 bounded multi-source recovery scheduler environment (2026-10-20)

- The scheduler is another dormant module in the existing transport crate. It
  adds no dependency or command to active CLI/Windows applications and changes
  no wire format.
- Inputs are borrowed caller-owned Endpoints/outboxes plus exact target
  `EndpointAddr` and transfer ID. Preflight verifies every durable request
  before the first executor call; no source discovery or file import exists.
- The core fairness machine is deterministic and uses one in-flight invocation.
  Seven focused tests cover policy, order, budgets, acknowledged skip, restart-
  safe pending state, fatal errors and durable candidate preflight.
- The scheduler adds no network primitive; it delegates to the already real-
  loopback-tested M1.2.25 executor. Windows tests still compile without running
  hash-named binaries and execute through `run-cargo-tests-stable.ps1`.
- Focused scheduler regression passes 7/7 and full crate regression passes
  26/26. Dedicated boundary rejects 10/10 mutations; accepted-tag readiness
  passes 153/153 with rustfmt and workspace-wide locked/offline strict Clippy.
- No product Internet/relay, second host, release build, ZIP/archive, tag or
  automatic GitHub build was used. Active IPC remains v33/domain v14 and
  public default remains `legacy-only`.

## M1.2.28 durable recipient recovery coordinator environment (2026-10-20)

- The execution crate now has a separate recipient coordinator redb under the
  recipient state root. Active CLI/Windows manifests still do not depend on the
  crate; IPC remains v33/domain v14 and no wire format changed.
- Approved inputs are derived from durable signed consent and are limited to
  1..=8 distinct source Devices for one Account/recipient. The database stores
  only domain-separated/source/content IDs and monotonic counts/ranges; transfer
  and import artifacts stay in their existing stores.
- `plan` verifies both full append-only stores and is no-create/read-only.
  `apply` acquires `StateDirectoryLock`, repeats the exact observation and uses
  one immediate-durability transaction for the plan record, source heads and
  chain head. It never calls transfer or import operations.
- Four focused stable-path tests pass: no-create/idempotent replay, real staged
  transfer through signed import completion, stale-head/precommit rollback and
  duplicate-source rejection. The system TEMP was inaccessible in the managed
  sandbox, so tests used the repository `.tmp` directory without changing the
  executable path or product behavior.
- Dedicated boundary rejects 10/10 mutations. Full execution regression and
  accepted-tag readiness are required at the final gate and recorded as 18/18
  and 154/154 respectively.
- No product network, relay, second host, release build, ZIP/archive, tag or
  automatic GitHub build was used. Public default remains `legacy-only`.

## M1.2.29 explicit recipient recovery import plan environment (2026-10-20)

- The execution crate now exposes a dormant read-only import planner and an
  explicit one-transfer apply. Active CLI/Windows manifests remain free of the
  crate; IPC stays v33/domain v14 and no wire format changes.
- Planning requires a durably applied M1.2.28 coordinator snapshot. It selects
  `transfer ordinal == imported chunk count`, verifies the signed membership
  and existing receipt predecessor, and creates no durable state.
- Apply acquires the common `StateDirectoryLock`; coordinator revalidation,
  exact inbox-head/backlog selection and the existing atomic import run under
  that lock. Coordinator refresh follows after the import transaction.
- The unavoidable import-before-refresh window is resumable. Fault injection
  commits the signed import receipt, skips refresh, then the same plan returns
  exact `AlreadyImported` and advances the coordinator without duplicate data.
- Four focused stable-path tests cover three-page backlog order, unapplied
  coordinator/membership rejection, newer-head staleness and crash-window
  resume. Full execution regression passes 22/22.
- Dedicated boundary rejects 10/10 mutations; accepted-tag readiness passes
  155/155 with rustfmt and workspace-wide locked/offline strict Clippy.
- The managed sandbox's system TEMP remains inaccessible, so Rust harnesses
  use repository `.tmp` while retaining the permanent verification EXE path.
  No network, second host, release build, ZIP/archive, tag or automatic GitHub
  build is used; public default remains `legacy-only`.

## M1.2.30 bounded multi-source recipient import round environment (2026-10-20)

- The execution crate adds one dormant in-process round planner/applicator over
  M1.2.29. Active CLI/Windows manifests still do not depend on it; IPC remains
  v33/domain v14 and no wire format changes.
- Default/hard budgets are 4/8 sources and 128/256 events. All caller-ordered
  ordinals and signed memberships are preflighted before the first mutation;
  imports remain synchronous with exactly one in flight.
- Five focused stable-path tests include a real two-source Account fixture in
  reverse order `[1, 0]`, late membership rejection before mutation, per-source
  backlog fairness and exact restart/replay of reconciled and unreconciled
  progress. Full crate regression passes 27/27.
- The first two-source run exposed and fixed an ordering bug where prefix state
  was initially inspected in coordinator ordinal order instead of caller round
  order. The permanent-path rerun passed.
- Dedicated boundary rejects 10/10 mutations; accepted-tag readiness passes
  156/156 with rustfmt and workspace-wide locked/offline strict Clippy.
- Rust tests use repository `.tmp` because system TEMP is inaccessible in the
  managed sandbox. No network, second host, release build, ZIP/archive, tag or
  automatic GitHub build is used; public default remains `legacy-only`.

## M1.2.31 durable recipient import-round intent environment (2026-10-20)

- The execution crate now exposes a dormant signed intent store layered over
  the M1.2.30 round. Active CLI/Windows manifests still do not depend on it;
  IPC remains v33/domain v14 and no ticket/sync/history-transfer wire changed.
- Read-only plan requires the exact unstarted round and creates no state.
  Explicit commit uses the shared state lock and a separate immediate redb,
  bounded to 4096 intents, 64 KiB each and one active unconsumed round.
- Stored content is limited to owner/round/coordinator IDs, opaque source heads,
  counts/ranges, ordered import-plan bindings, budgets and membership digests.
  Membership snapshots, history artifacts, bodies and plaintext are absent.
- Read-only progress verifies the baseline coordinator chain member, exact
  caller-order prefix, every completed receipt/artifact and unchanged unplanned
  sources. Complete-but-unrefreshed coordinator state is observable but cannot
  be consumed; explicit signed consume is crash/replay idempotent.
- Four focused stable-path tests pass in 1218.58 s, including real two-source
  import and store reopen. Full execution regression passes 31/31; boundary
  rejects 10/10 mutations and accepted-tag readiness passes 157/157 with
  rustfmt and workspace-wide locked/offline strict Clippy.
- The managed sandbox cannot write system TEMP, so Rust tests were executed by
  the same permanent-path harness outside that filesystem boundary. No product
  network, second host, release build, ZIP/archive, tag or automatic GitHub
  build was used; public default remains `legacy-only`.

## M1.2.32 explicit durable recipient import-round executor environment (2026-10-21)

- The execution crate adds a dormant in-process foreground executor over the
  M1.2.31 intent. Active CLI/Windows manifests still do not depend on the crate;
  IPC remains v33/domain v14 and no ticket/sync/history-transfer wire changes.
- Read-only planning requires one active unconsumed intent, an exact current
  coordinator observation and caller-supplied live memberships in signed order.
  It creates no state and produces a content-addressed bounded execution plan.
- `apply_next`/`apply_bounded` use the existing M1.2.29 importer with default/
  hard limits 1/8 and one import in flight. Fresh candidates must equal signed
  bindings; active intent and durable prefix are reopened after every source.
- A committed import with missing coordinator refresh is reconciled before any
  next source. Complete replay is a zero-import no-op and explicit M1.2.31
  signed consumption remains separate.
- Three focused stable-path tests pass, including real reverse-order two-source
  execution and crash-window reconciliation. Full execution regression passes
  34/34; boundary rejects 10/10 mutations and readiness passes 158/158 with
  rustfmt and workspace-wide locked/offline strict Clippy.
- The managed sandbox cannot write system TEMP, so the stable test harness was
  executed outside that filesystem boundary without changing its permanent EXE
  path. No network, second host, release build, ZIP/archive, tag or automatic
  GitHub build was used; public default remains `legacy-only`.

## M1.2.33 dormant cross-process recipient import execution contract environment (2026-10-21)

- `kilogram-runtime-ipc` exports a dormant canonical contract module, but the
  active `RuntimeIpcCommand`/server remain unchanged at v33/domain v14. No CLI,
  desktop or adapter registration was added.
- Requests are bounded to 8 KiB and split into `Plan`, `Status`, `Confirm`,
  `ApplyNext` and `ApplyBounded`. Signed responses are bounded to 16 KiB and
  exact-bind request digest, verified descriptor and current descriptor Device.
- Up to eight strictly ordered membership entries contain only opaque authority
  reference, membership digest and authenticated context IDs. The contract
  requires a future adapter to reload the caller-authorized live membership;
  no membership snapshot or history content crosses the boundary.
- Confirmation and mutation use distinct nonces/IDs. Stage skip, descriptor or
  request substitution, reordered/duplicate references and step-budget escape
  fail closed; response explicitly cannot consume the durable intent.
- Eight focused stable-path tests and all 35/35 runtime IPC tests pass. Boundary
  rejects 10/10 mutations; accepted-tag readiness passes 159/159 with rustfmt
  and workspace-wide locked/offline strict Clippy.
- Stable Windows test EXE paths were retained. No product network, second host,
  release build, archive, tag or automatic GitHub build was used; public default
  remains `legacy-only`.

## M1.2.34 dormant recipient import read adapter environment (2026-10-21)

- `kilogram-peer-history-recovery-runtime` is the separate one-way integration
  crate over recovery execution and `kilogram-runtime-ipc`. The offline
  execution production library retains no runtime-IPC dependency; its signed
  fixtures include the same adapter source only under `cfg(test)`. Active
  runtime IPC remains v33/domain v14.
- The transport-independent adapter handles only `Plan` and `Status`. It is not
  registered in the runtime server, CLI or desktop and returns an outcome plus
  an optional ephemeral in-process M1.2.32 plan.
- Planning synchronously reloads every exact caller-owned membership reference,
  verifies authority/context echo and membership digest, then calls the existing
  read-only executor planner. No membership snapshot or plan is persisted.
- Unplanned status requires the original coordinator; exact known-plan status
  reloads and replans. Missing, ambiguous, substituted, foreign or stale input
  fails closed. All confirm/apply commands are rejected before loader use.
- Eight focused tests pass through the permanent stable Windows harness in
  451.67 s. Full execution regression passes 42/42; boundary rejects 10/10
  mutations and accepted-tag readiness passes 160/160 with rustfmt and
  workspace-wide locked/offline strict Clippy.
- Cargo target was retained for warm builds (49.11 GiB after the focused run).
  No product network, second host, release build, archive, tag or automatic
  GitHub build was used; public default remains `legacy-only`.

## M1.2.35 durable recipient import confirmation ledger environment (2026-10-21)

- The dormant `kilogram-peer-history-recovery-runtime` crate now owns a
  separate confirmation ledger; active applications still do not depend on it
  and active IPC remains v33/domain v14.
- Confirmation planning requires the exact M1.2.34 planned dispatch and exact
  M1.2.33 Confirm command. It is read-only/no-create and produces a current
  recipient-Device-signed candidate with a hard 900-second lifetime.
- Explicit commit uses the shared state-directory lock and immediate redb
  durability. Records, one-to-one nonce index and a single monotonic chain head
  are bounded to 4096 and fully verified on every read.
- Exact replay is idempotent. Nonce reuse for another binding, stale concurrent
  candidates, expiry, foreign Device, internal fork/rollback/corruption and
  symlinked ledger paths fail closed. Whole-file rollback without an external
  witness is intentionally not claimed.
- Eight focused tests pass through the permanent stable Windows harness;
  package strict Clippy passes. Boundary rejects 10/10 mutations and accepted-
  tag readiness passes 161/161 with rustfmt and workspace-wide locked/offline
  strict Clippy.
- No import/consume, product network, second host, release build, archive, tag
  or automatic GitHub build was used; public default remains `legacy-only`.

## M1.2.36 explicit bounded recipient import mutation adapter environment (2026-10-21)

- The dormant runtime-integration crate now handles only `ApplyNext` and
  `ApplyBounded`; active applications still do not depend on it and active IPC
  remains v33/domain v14.
- Every first attempt requires an exact unexpired M1.2.35 confirmation, current
  owner Device, caller-reloaded live memberships and an exact fresh M1.2.32
  execution plan. Default/hard step budgets remain 1/8 and imports sequential.
- A separate immediate-durability redb stores a Device-signed `Prepared`
  record before import and a `Completed` privacy-safe receipt before response.
  It is bounded to 4096 mutations/8192 records with exact nonce index and one
  verified content-addressed chain.
- Exact completed replay performs no loader/import. Prepared retry permits only
  monotonic progress within the original budget, repairs coordinator-only
  import-before-refresh state, reconstructs exact already-imported steps and
  refuses additional import after confirmation expiry.
- Stable-path ledger tests pass 3/3; a real signed import/replay fixture passes
  1/1 and a separate crash-reconciliation fixture passes 1/1. Package strict
  Clippy passes; boundary rejects 10/10 mutations and accepted-tag readiness
  passes 162/162 with rustfmt and workspace locked/offline strict Clippy.
- The ledger stores no membership snapshot or message/history artifact. Intent
  consumption, active runtime/server/CLI/desktop/background/network, product
  network, second host, release build, archive, tag and automatic GitHub build
  remain absent; public default is `legacy-only`.

## M1.2.37 explicit recipient import-consumption contract environment (2026-10-21)

- `kilogram-runtime-ipc` exports a dormant consumption ceremony module while
  the active `RuntimeIpcCommand`, IPC v33/domain v14 and application manifests
  remain unchanged.
- Only read-only `Plan` and explicit `Commit` exist. Completed evidence binds
  the exact M1.2.36 record ID and canonical receipt digest plus all upstream
  IDs, ordered membership-set digest, final coordinator, closed source counts
  and imported event count. Both stages carry the same ordered opaque
  membership references and reject any digest substitution; no membership
  snapshot crosses the contract.
- Plans are content-addressed, use a nonce distinct from mutation evidence and
  expire within at most 300 seconds. Commit requires the exact derived
  `CONSUME` phrase and the non-default `UserConfirmed` approval enum.
- Signed responses exact-bind request digest, descriptor and current Device.
  Plan cannot claim consumption; commit receipts must clear the active intent
  inside the plan lifetime, and `AlreadyConsumed` remains exact-binding only.
- Focused stable-path tests pass 8/8 and the full runtime IPC regression passes
  43/43. Package strict Clippy passes; boundary rejects 10/10 mutations and
  accepted-tag readiness passes 163/163 with rustfmt and locked/offline
  workspace-wide strict Clippy.
- No adapter, durable consumption-plan ledger, product network, second host,
  release build, archive, tag or automatic GitHub build was used. Active CLI,
  desktop, background and network authority remain absent; default is
  `legacy-only`.

## M1.2.38 recipient import-consumption read adapter environment (2026-10-21)

- `kilogram-peer-history-recovery-runtime` owns the dormant adapter because it
  bridges runtime-IPC vocabulary and offline recovery stores; the execution
  engine still has no production runtime-IPC dependency.
- The mutation ledger exposes read-only exact-nonce inspection. A missing root
  remains absent and creates no directory/database/table/lock; prepared and
  completed records are projected separately.
- Only `Plan` reaches durable reads. `Commit` returns `CommitRejected` before
  mutation, intent or membership access. Completed evidence is reconstructed
  from durable record ID, membership-set digest and receipt and exact-compared.
- Current coordinator is checked before membership reload. The exact intent
  must remain active and unconsumed; caller-owned memberships are reloaded and
  read-only progress must be complete with no pending refresh and exact counts,
  event count and final coordinator.
- Four real signed-fixture tests pass through the stable Windows runner; the
  mutation-ledger projection test passes separately. Package strict Clippy,
  rustfmt, 10/10 boundary mutations and accepted-tag readiness 164/164 pass.
- No state mutation, plan persistence, intent consumption, active runtime,
  CLI/desktop, product network, second host, release build, archive, tag or
  automatic GitHub build is used. Default remains `legacy-only`.

## M1.2.39 recipient import-consumption authorization environment (2026-10-21)

- `kilogram-peer-history-recovery-runtime` now owns a separate dormant
  consumption-authorization ledger; active application manifests and runtime
  IPC v33/domain v14 remain unchanged.
- Planning accepts only an exact M1.2.38 ephemeral plan paired with the exact
  M1.2.37 `Commit`, completed evidence, membership-set digest,
  `UserConfirmed`, derived phrase and current owner Device. It is read-only and
  does not create missing state. Durable reads reconstruct the content-addressed
  plan from stored exact completed evidence rather than trusting its fields.
- Explicit commit preserves the original maximum 300-second plan lifetime and
  writes an immediate-durability, Device-signed, content-addressed chain with
  exact nonce index, state lock and 4096-record cap.
- Exact replay is idempotent. Rebound nonce, stale concurrent plan, expiry,
  foreign owner, injected pre-commit failure and corrupt head fail closed;
  whole-file rollback without an external witness is not claimed.
- Seven focused tests pass through the stable Windows runner. Package strict
  Clippy, rustfmt, 10/10 boundary mutations and accepted-tag readiness 165/165
  pass.
- No intent consumption, active runtime, CLI/desktop, product network, second
  host, release build, archive, tag or automatic GitHub build is used. Default
  remains `legacy-only`.

## M1.2.40 recipient import-consumption mutation environment (2026-10-21)

- `kilogram-peer-history-recovery-runtime` now contains the dormant mutation
  adapter and a separate
  `history-recovery/recipient-import-consumption-mutations-v1.redb` ledger.
- The adapter accepts only exact `Commit`, durable authorization and current
  owner Device. It reopens completed mutation evidence, reloads live
  caller-owned memberships, and requires exact current coordinator plus active
  complete progress before calling the existing intent-store consume method.
- The ledger writes immediate-durability Device-signed `Prepared` before
  consume and `Completed` before success. A post-consume/pre-completed injected
  failure reconciles from the exact signed consumption without another consume;
  exact completed replay never invokes the membership loader.
- Consumption removes only the active pointer. The signed intent, memberships,
  imported events and projections remain stored and readable.
- The managed Codex sandbox denied key-file writes to the external Windows
  `%TEMP%`; stable tests were therefore run with `TEMP` and `TMP` pointed at
  `.tmp/stable-test-temp` inside the workspace. This is a verification-host
  permission boundary, not a product-path requirement.
- Stable-path ledger tests pass 2/2 and real signed fixtures pass 3/3. Package
  strict Clippy, rustfmt, 10/10 boundary mutations and accepted-tag readiness
  166/166 pass.
- No active runtime response dispatch, CLI/desktop, product network, second
  host, release build, archive, tag or automatic GitHub build is used. Default
  remains `legacy-only`.

## M1.2.41 signed consumption response-dispatch environment (2026-10-21)

- `kilogram-peer-history-recovery-runtime` now composes the read adapter,
  authorization ledger and mutation adapter behind one dormant dispatcher that
  returns only `SignedRuntimeIpcRecipientImportConsumptionResponse`.
- Both Plan and Commit requests are canonical-round-tripped against the exact
  runtime descriptor before state access. Dispatcher owner Account, descriptor
  Device and current signing Device must match.
- Plan returns an opaque in-process stage and creates neither authorization nor
  consumption-mutation database. Commit cannot skip or fabricate that stage;
  the retained signed Plan response is verified again against the descriptor.
- Commit durably records authorization before entering mutation and signs only
  after M1.2.40 reports a durable completed receipt. Exact replay returns
  `AlreadyConsumed` with the same semantic receipt and invokes no membership
  loader.
- Windows execution used only the fixed-name stable runner with workspace-local
  `TEMP`/`TMP`; two real signed-fixture tests passed in 348.37 seconds. No
  product network or second host was involved.
- Dedicated boundary rejects 10/10 mutations; accepted-tag readiness passes
  167/167 with rustfmt and locked/offline strict Clippy. No release build,
  archive, tag or automatic GitHub build is used.

## M1.2.42 process-local consumption Plan session environment (2026-10-21)

- The dormant integration crate owns a bounded `VecDeque` cache with at most
  32 opaque M1.2.41 Plan dispatches. It has no serde codec, redb store, Tokio,
  Iroh, HTTP, file or background surface.
- Insert accepts only live signed `Planned` output. Exact plan/request-digest
  replay is idempotent; collision and capacity fail closed without evicting a
  live ceremony. Expired entries are purged before use.
- A freshly constructed cache models process restart and cannot Commit. An
  exact same-process Commit delegates to M1.2.41, and a repeated Commit returns
  durable `AlreadyConsumed`; expiry removes the stage.
- The real durable fixture passed 1/1 in 191.69 seconds through the fixed-name
  stable Windows harness with workspace-local `TEMP`/`TMP`.
- Active IPC remains version 33/domain v14. Adding the command is explicitly a
  later v34/domain v15 compatibility migration; no CLI, desktop, network,
  release, ZIP, tag, automatic GitHub build or second host is used.

## M1.2.43 runtime IPC migration preflight environment (2026-10-21)

- The machine-readable preflight allocates runtime IPC 34 and signed descriptor
  domain v15 for the exact recipient-import-consumption command/result pair.
- The implementation remains at version 33/domain v14. The preflight boundary
  rejects any early enum/version/domain activation and every hybrid generation.
- A repository scan found 58 PowerShell boundary files that pin at least one
  live version/domain constant. The activation stage must update their live
  binding expectations while preserving historical manifest claims.
- The migration is an exact-version hard cut for co-installed binaries: no
  dual reader, format sniffing, fallback or silent downgrade is permitted.
- Dedicated boundary self-test rejects 10/10 mutations. Readiness now has 169
  closed gates. No Rust build is required because executable source is
  unchanged; no network, second host, release build, archive, tag or automatic
  GitHub build is used.

## M1.2.44 active recipient-import-consumption envelope environment (2026-10-21)

- Runtime IPC is now Postcard v34 with signed JSON descriptor domain v15. Both
  exact recipient-import-consumption enum variants, canonical CDDL aggregate,
  completion/schema ledgers and compatibility bindings moved together.
- Historical M1.2.43 manifest still records previous 33/v14 and its 58 observed
  binding gates; the explicit successor gate is excluded from that historical
  count rather than rewriting past evidence.
- Runtime actor currently returns only `recipient import-consumption runtime
  service is not configured`; no success response, cache ownership, dispatcher,
  CLI/desktop or product network is reachable.
- The managed sandbox denied loopback socket creation with Windows error 10013;
  the same fixed-name stable-path harness passed both v33 downgrade tests 2/2
  outside that sandbox. This is a verification boundary, not a product error.
- Dedicated activation boundary rejects 10/10 mutations and readiness contains
  170 closed gates. No release build, ZIP/archive, tag, automatic GitHub build
  or second host is used; default remains `legacy-only`.
