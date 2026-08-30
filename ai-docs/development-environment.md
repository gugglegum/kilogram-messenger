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

Публичный relay проверен на одном хосте в принудительном `relay-only`. Внешний
M0.3 direct-only тест корректно доказал невозможность hole punching в выбранной
home-to-cellular topology; внешний relay-only control требует повтора с новой
диагностической сборкой.

## Решения, которые ещё нельзя фиксировать

- Версию OpenMLS следует выбрать по стабильному crates.io-релизу, а не по `main`.
- Protobuf пока не выбран, поэтому отсутствие системного `protoc` не является
  блокером; при выборе Protobuf желательно использовать воспроизводимый
  vendored protoc.
- CMake/Ninja устанавливать заранее не требуется.
