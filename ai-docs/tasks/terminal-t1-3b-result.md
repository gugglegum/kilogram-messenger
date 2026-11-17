# T1.3b. Кодек и Windows-примитивы для LAN discovery

База `e2d899c85b0f5eebeca67952daf8c41b3dd18e0d`. Кодек проверяет канонический CBOR, подписи, HPKE, интервалы, context/consent/cert и точный общий сокет. Он возвращает локаторы в порядке слотов вызывающей стороны. Пригодность для admission по текущей политике и durable high-water остаётся задачей Owner2. Действующая форма пустого relay в `RuntimeLocator2` — CBOR `[]`. Она занимает один байт, как и в расчёте бюджета контракта. Принятый контракт сохранён побайтно. [Контракт](terminal-t1-3-contract.md) · [Кодек](../../crates/kilogram-protocol/src/ordered_dialog_v2/lan_discovery.rs)

`kilogram-windows-lan` изолирует вынужденный Win32 FFI в модуле `native`. Остальной workspace сохраняет запрет unsafe. COM и NLM живут на отдельном потоке. Сокет на `239.255.75.72:50372` использует явный egress и join, TTL 1, DF, exclusive bind и фактический IPv4 packet info. Сокет и события освобождаются через RAII. Новый пакет пока не подключён к обычному runtime. Setup, IPC, Owner2 и ручной LAN-путь сохранили прежний код. [Windows-примитивы](../../crates/kilogram-windows-lan/src/lib.rs)

## Проверки

Финальная попытка `focused04` завершилась с exit 0, EOF и пустым Windows Job без принудительной уборки. Rust 1.98.0, `verification`, Idle, один Cargo worker, offline и locked зависимости. Все harnesses сначала компилировались с `--no-run`. Зачётные EXE совпали с хешами после сборки в `focused03`. [Квитанция](../../.tmp/terminal-t1-3b/focused04.receipt.json) · [Вывод](../../.tmp/terminal-t1-3b/focused04.stdout.log) · [Манифест](../../.tmp/terminal-t1-3b/review.json)

| Команда через `scripts/run-cargo-tests-stable.ps1 -Profile verification -CargoJobs 1 -Run` | Результат |
| --- | --- |
| `-Package kilogram-protocol -Filter lan_discovery_` | 5 тестов. Обратный порядок слотов, 244/583/650/704/720/759 байт, границы времени до `u64::MAX`, replay и отказы на неверной форме, шифровании и scope. |
| `-Package kilogram-protocol -Filter runtime_locator::lan_tests::` | 1 существующий тест ручного Direct LAN, проверка фактического префикса. |
| `-Package kilogram-windows-lan -CargoFeatures kilogram-windows-lan/native-feasibility -Filter lan_` | 2 теста. Проверка трёх полей route predicate и native self-receive на текущем ПК. |

Scoped `rustfmt --check` и `cargo clippy --locked --offline --profile verification --jobs 1 -p kilogram-protocol -p kilogram-windows-lan --all-targets --features kilogram-windows-lan/native-feasibility -- -D warnings` прошли. [Команды](../../.tmp/terminal-t1-3b/focused04.ps1) · [Линтер](../../.tmp/terminal-t1-3b/lint01.ps1)

Типизированный NLM подтвердил один активный Private Network ID на выбранном Wi-Fi `192.168.0.140/24`, ifIndex 22, MTU 1500. Получен синтетический UDP payload 759 байт. Source `192.168.0.140:50372`, destination `239.255.75.72`, входной ifIndex 22, egress `192.168.0.140`, TTL 1, DF 1. Собственный конфликт порта дал 10048. Усечённый и короткий кадры отклонены. На отдельном сокете подтверждены idle timeout, отмена и освобождение порта. Это self-receive на одном ПК. Проверка реального peer-source, двух хостов и работа с включённым Firewall относятся к полевому срезу. Firewall и сеть здесь не перестраивались.

В `native01` проверка idle получила поздний кадр с собственного адреса. Idle и отмена перенесены на отдельно открытый сокет. В `native02`, `focused02` и `focused03` тесты завершились с кодом 0, но Job пришлось убрать оставшийся `vctip`. Чистые прогоны из кэша — `native03` и `focused04`. В `focused01` слишком широкий подстрочный фильтр выбрал посторонний recovery-тест, который остановил watchdog. Зачёт использовал точные селекторы. [Архив попыток](../../.tmp/terminal-t1-3b/review.json)

## Предлагаемые файлы для публикации

- `Cargo.toml`
- `Cargo.lock`
- `crates/kilogram-protocol/src/ordered_dialog_v2.rs`
- `crates/kilogram-protocol/src/ordered_dialog_v2/lan_discovery.rs`
- `crates/kilogram-protocol/tests/lan_discovery.rs`
- `crates/kilogram-windows-lan/Cargo.toml`
- `crates/kilogram-windows-lan/src/lib.rs`
- `crates/kilogram-windows-lan/src/native.rs`
- `crates/kilogram-windows-lan/src/native/binding.rs`
- `crates/kilogram-windows-lan/src/native/socket.rs`
- `ai-docs/tasks/terminal-t1-3-contract.md` (уже принятый контракт)
- `ai-docs/tasks/terminal-t1-3b-result.md`
