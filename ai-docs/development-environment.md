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
