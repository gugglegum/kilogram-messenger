# Среда разработки

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
multi-source claim reconciliation, а M0.7.9 — signed checkpoint pagination и
safe retry. Source discovery/background coordinator, membership removal/epochs
и group E2EE остаются открыты. M0.7.7 закрывает M0 crash consistency для device
filesystem state; M0.8.1 доказывает атомарную encrypted shadow migration в
`redb`, M0.8.2 — recoverable versioned dual-write всех live CLI commands,
M0.8.3 — typed incremental encrypted delta и exact shadow reads, M0.8.4 —
первый DB-primary canary для read-only history, M0.8.5 — тот же cutover для
manual/network history-rewrap source reads, M0.8.6 — command-local overlay для
mixed sync reads, а M0.8.7 — vault-primary commit barrier для journaled writes.
M0.8.8 добавляет typed direct journal delta и mutable DB-primary sequence.
Ratchet/trust DB-owned adapters, explicit append-only write-set, bounded backups
и защищённый key provider ещё не реализованы.

## Решения, которые ещё нельзя фиксировать

- Версию OpenMLS следует выбрать по стабильному crates.io-релизу, а не по `main`.
- Protobuf пока не выбран, поэтому отсутствие системного `protoc` не является
  блокером; при выборе Protobuf желательно использовать воспроизводимый
  vendored protoc.
- CMake/Ninja устанавливать заранее не требуется.
