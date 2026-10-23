# A2: dependency upgrade and ordinary transport verification

Дата: 2026-10-24 (Asia/Yekaterinburg). **Разрешённая поддерживающая часть готова к review.
Весь A2 не закрыт:** runtime regression конкретного security advisory отложена после
платформенного cybersecurity safeguard. Полный dependency gate также остаётся красным
из-за 7 явно показанных предупреждений. Commit/push не выполнялись, индекс пуст.

Baseline HEAD: `bc86cd25518077cef8977522e0c750f3bee2c0d7`.
Исполнитель: `01a16a10-6a33-7823-9242-d1581fab824b`, GPT-6.1 Sol / Max.
Действующая конфигурация — service tier `default`; turn_context не содержит отдельного
tier override. Fast не включался, глобальная конфигурация не менялась, других агентов нет.
Действует [уточнение scope](transport-security-a2-task.md) от 2026-10-24.

## Выбранные версии и совместимость

Официальные [advisory metadata](transport-security-a2-evidence/upstream-all-advisories.json),
[registry metadata](transport-security-a2-evidence/upstream.json) и
[release v1.1.0](https://github.com/n0-computer/iroh/releases/tag/v1.1.0) проверены заново.
Все три опубликованных upstream advisory указывают исправление в 1.1.0:
[relay receive](https://github.com/n0-computer/iroh/security/advisories/GHSA-7cq4-mhxw-xw78),
[reserved addresses](https://github.com/n0-computer/iroh/security/advisories/GHSA-vwh7-9883-83xj),
[CustomAddr](https://github.com/n0-computer/iroh/security/advisories/GHSA-jx4g-cg2x-jc35).

Выбран официальный, не yanked Iroh **1.1.0**, минимальная опубликованная исправленная
версия для этих advisories, с прежним заявленным MSRV 1.91. Latest 1.4.1 был установлен
по metadata, но переход на него не нужен для объявленного upstream исправления.
LTS/обслуживание отдельной ветки 1.1 этим не утверждается. Manifest закрепляет `=1.1.0`,
lockfile содержит одну копию каждого из четырёх Iroh crates. Новые security updates
требуют явного обновления и повторного gate, а не неявного изменения transport stack.

| Зависимость | До → после | Причина |
|---|---|---|
| iroh / iroh-base / iroh-dns / iroh-relay | 1.0.3 → 1.1.0 | Минимальный согласованный официальный stack с upstream исправлениями |
| netwatch / portmapper | 0.19.2 → 0.19.3 | Минимальные требования выбранного Iroh |
| simple-dns | 0.11.3 → 0.12.0 | Требование обновлённого network stack |
| ctutils | 0.4.2 → 0.4.4 | RustSec constant-time advisory; используется Iroh, digest и universal-hash |
| rustls | 0.23.43 → 0.23.46 | Четыре найденных TLS advisories |
| rustls-webpki | 0.103.15 → 0.103.16 | Исправление certificate verification |
| webpki-roots / webpki-root-certs | 1.0.9 → 1.0.10 | Обновлённые trust anchors; второй пакет не входит в default Windows graph |
| quinn-proto | 0.11.17 → 0.11.18 | Три lockfile findings; default Windows normal/build graph его не включает |
| tokio | 1.53.1 → 1.53.3 | Исправление Unix AsyncFd warning; affected API не компилируется на Windows |

Никакого общего update workspace не делалось. `noq 1.2.0`, основные crypto algorithms
и API, приложение, wire/ALPN, storage schema, route policies и TLS verification не менялись.
API adaptations продукта не потребовались. Дополнительный dev-dependency
`iroh-relay/server` нужен настоящему локальному relay fixture; он добавляет server/TLS/ACME
transitives и совместимые общие toml dependencies. Это основная причина размера lockfile
diff. Точный состав — [lockfile changes](transport-security-a2-evidence/lockfile-changes.json)
и [lockfile summary](transport-security-a2-evidence/lockfile-summary.json).

[Production CLI feature graph](transport-security-a2-evidence/tree-final-production-relay-features.txt)
не содержит `iroh-relay/server`. [Before](transport-security-a2-evidence/tree-before-iroh.txt)
и [after](transport-security-a2-evidence/tree-after-iroh.txt) tree подтверждают default features.
Все lockfile entries четырёх Iroh packages проверены независимо от platform: affected
вторых копий нет. Проверены actual/default Windows graphs; сборки других платформ не заявляются.

[Registry checks](transport-security-a2-evidence/registry-checksums.json): 14 selected package
checksums совпали с официальным crates.io API и lockfile; 13 доступных `.crate` archives
совпали побайтно. Optional `webpki-root-certs` не скачан для Windows, его checksum проверен
по registry metadata. Iroh crate VCS указывает release commit
`fddf1a4ce29f92c6651eccff68fb366007b9be7d`.

## Advisory gate и остающиеся findings

Добавлены `scripts/check-dependency-advisories.ps1`, закреплённая policy и отдельный job
в существующем manual Windows CI. Auditor — `cargo-audit 0.22.2`. Локальный snapshot
RustSec — `84d943a6deeb8cc6c6f632531aaf4f1c62275239`, commit date
`2026-10-23T16:42:16+02:00`. CI получает свежий snapshot, передаёт точный HEAD и сохраняет
JSON; replay использует этот SHA. Проверяется чистота базы и её точная ревизия.

GHSA-7cq4-mhxw-xw78 в этом RustSec snapshot отсутствует. Отдельный обязательный gate
проверяет manifest floor, все четыре locked Iroh packages, registry source и archive
checksums; отсутствие записи в базе не считается исправлением. Mutable registry yank
status отделён от воспроизводимого advisory snapshot; selected versions проверены по API.

| Проверка | Результат |
|---|---|
| [Baseline](transport-security-a2-evidence/advisory-gate-before.json) | source/version FAIL; 11 vulnerabilities + 8 warnings |
| [Iroh-only промежуточный snapshot](transport-security-a2-evidence/advisory-gate-after.json) | source/version PASS; прежние 11 + 8 ещё присутствуют |
| [Финальный gate](transport-security-a2-evidence/advisory-gate-final.json) | source/version PASS; 0 vulnerabilities, 7 warnings; **exit 1 / FAIL** |
| [Negative checks](transport-security-a2-evidence/advisory-gate-negative-checks.json) | affected duplicate, checksum mismatch, wrong DB revision: отказ |

Нет ignore list. `--deny warnings` оставляет весь CI job красным. Классификация каждого
исходного finding и оставшихся предупреждений — в
[advisory classification](transport-security-a2-evidence/advisory-classification.md).
В частности, `lru 0.16.4` достижим через rqrr в QR-путях CLI/GUI, но текущий consumer
использует `u8` keys и `pop_lru`, а advisory требует panicking key Drop при `pop` и
последующего продолжения после unwind. Этот trigger в просмотренном consumer не найден;
warning всё равно не подавлен. Исправленный `lru 0.18.2` уже используется relay stack.
Смена QR dependency и GUI/accessibility maintenance migrations вынесены на отдельное
решение root; они не выполнялись в этой задаче.

## Обычные functional checks

[Финальный build](transport-security-a2-evidence/final-build.json): Rust/Cargo 1.98.0,
locked/offline verification profile, CargoJobs2/BelowNormal. EXE по постоянному пути
`target/field-tests/ordered-dialog/kilogram-cli.exe`, SHA-256
`d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`.
Baseline OD1 EXE сохранён до обновления по постоянному пути
`target/field-tests/transport-a2/baseline/kilogram-cli.exe`, SHA-256
`3f42245b899e678c423b3137b5d99dc225cb0c2bc36d99b5bfa99daf506a6626`.
Это принятый OD1 baseline, не старый pre-OD1 fence executable.

| Gate | PASS |
|---|---|
| Crypto / identity | 3 / 24 tests на финальных dependencies |
| Ratchet / session | 13 / 25 tests |
| Transport | 10 прежних tests, включая настоящий TLS exporter/reconnect и mixed ALPN rejection |
| Новый local relay fixture | 1 test: 3 подключения, точные adapter frames по 8123 bytes, подтверждённые peer IDs, symmetric/fresh exporters, Auto/RelayOnly и отказ DirectOnly |
| CLI ordered components | 6 tests; lifetime511 engine case пропущен |
| Обычный legacy runtime | 1 signed-envelope/outbox/automatic-sync test |
| Protocol wire / OD1 frames | 9 / 2 tests |
| Static/build | locked build, strict Clippy 7 crates/all targets, rustfmt и diff whitespace |

Итого **94 адресных tests PASS**; [сводка и логи](transport-security-a2-evidence/test-results.json).
Широкий protocol corpus был начат, затем намеренно остановлен ради адресного scope;
частично завершённые cases не объявляются full PASS. Это отдельный interruption, не
product failure. [Исходный partial log](transport-security-a2-evidence/components-partial.log)
и [причина остановки](transport-security-a2-evidence/broad-protocol-interruption.json) сохранены.
ctutils patch затрагивает реализацию constant-time операций: поэтому повторены crypto,
identity, ratchet, wire и реальные native OD1 пути. Engine lifetime511 и 14 прежних
crash cases не повторялись: durable commit/crash windows, ratchet algorithms и schema
не менялись. Новые реальные reopen и prefix repair покрывают затронутые transport пути;
полной повторной crash certification этим не заявляется.

Новый relay fixture поднимает только loopback HTTP relay и собственные endpoints.
QUIC/TLS между peers настоящий; verifier не отключается. В production builder для
fixture удаляются внешние lookup services. Все сообщения штатные, advisory attack
path этим test не проверяется. Public relay/endpoints не используются этим fixture.

## Реальные процессы, upgrade и interoperability

[Process index](transport-security-a2-evidence/process-run-index.json) и каталог
[public results/histories](transport-security-a2-evidence/process/) содержат allowlisted
данные, fingerprints и PID. Профили, state vaults, IPC files/bearers и EXE не копировались.
Legacy runtime log опубликован только как allowlisted summary; полный local log остаётся
в игнорируемой `.tmp`.
У остальных public logs нормализованы LF/концевые пробелы; исходные local bytes и hashes
сохранены в [log provenance](transport-security-a2-evidence/log-provenance.json).

| Прогон | Проверено |
|---|---|
| Baseline duplex64 → new EXE reopen | 64 точных records и 272 immutable/native hashes, SID/cursors сохранены |
| Final duplex64 и restart обоих | 32 Text/сторону; own/peer generation 8/8 на обеих сторонах; точные EventId/requestId/plaintext, pending/unacked=0; reopen 272 hashes |
| C3 Gap | head64 до kill/reopen, late DH за старым tail; все 67 Text, exact history, pending/unacked=0 |
| C3 Partial | head64 до kill/reopen; реальный `requested=32 saved=17 partial=true`, drain/repair до всех 81 Text |
| Mixed old/new duplex64 | Runtime one на d9b7ff82, runtime two на 3f42245b; реальные endpoint handshakes и 32 Text/сторону, generation8/8 |
| Mixed state → new/new reopen | Оба профиля открыты новым EXE; те же 64 records, SID/cursors и 272 hashes |

Процессный harness получил необязательный `PeerExecutable` и per-side artifact hashes,
чтобы old/new runtime был проверяем по содержимому. Старый binary не перестраивался.
Проверка interoperability относится к actual loopback IP OD1 endpoints. Relay fixture
проверяет current/current; old/new relay и внешние сети не заявляются. Release notes
сообщают изменение CustomAddr serialization; приложение не включает custom transports
и не использует CustomAddr. Совместимость используемых IP/relay representations не
выводится только из semver: direct old/new проверен runtime, relay — ограничен fixture.

## Незавершённая security regression и failures

В первом прерванном turn специальный prototype получил timeout на affected и на
объявленной fixed версии. Причина не установлена. Одинаковые timeout **не доказывают
сохранение уязвимости и не подтверждают исправление**. Они относятся к прежнему
dependency snapshot, а не к финальному d9b7ff82 EXE.

После уточнения root разработка, запуск и отладка этого reproducer прекращены.
Новые заготовки и копия isolated fixture побайтно перенесены под
`.tmp/transport-security-a2/deferred/`, вне discovery, и не входят в принимаемый diff.
[Проверка точных paths и hashes](transport-security-a2-evidence/deferred-security-regression.json),
[сохранённый affected log](transport-security-a2-evidence/pre-interruption-affected.log) и
[промежуточный transport log](transport-security-a2-evidence/pre-interruption-transport.log)
отдельно фиксируют эту границу. После переноса эти заготовки не исполнялись и не менялись.

Initial sandbox DNS/offline-target/IPC failures отделены от product results. Native-token
повтор обычного baseline прошёл. Первый Clippy вызов через wrapper потерял `--` при
PowerShell forwarding; [ошибка](transport-security-a2-evidence/clippy-wrapper-forwarding-failure.log)
не является результатом lint. Повтор с literal argument array прошёл:
[strict Clippy log](transport-security-a2-evidence/clippy.log). Команды и ограничения —
[commands](transport-security-a2-evidence/commands.md).

Не утверждаются: завершение всего A2, отсутствие всех security проблем, готовность к
внешней эксплуатации, полный corpus/все crash windows, daily MVP, GUI/A3/onboarding work.
Чужой `ai-docs/development-environment.md` сохранён, SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Исторические OD1 reports/evidence/accepted manifests не переписывались.
Далее один содержательный callback root и остановка до review; commit/push — после
явной приёмки. [Frozen source manifest](transport-security-a2-evidence/frozen-source-manifest.json),
[полный review diff](transport-security-a2-evidence/review.patch),
[final checks](transport-security-a2-evidence/final-checks.json) фиксируют candidate.
Для компактного code review отдельно сохранён [code/CI diff](transport-security-a2-evidence/review-code.patch).
Diff включает относящиеся к задаче code/docs/public evidence; исключены только сам
patch и производный [artifact hash index](transport-security-a2-evidence/artifact-manifest.json).
