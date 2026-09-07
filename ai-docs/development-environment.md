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
