# CS2: recoverable direct Runtime6 bootstrap — review result

2026-11-10. Baseline `d99e4d49e03d958df123c9bcd182313276bc862e`. Новый explicit `kilogram-bootstrap create-ordered2` и `kilogram_bootstrap::ordered2::create` создают recoverable Account Root и первый Runtime6/ordered-dialog-v2 device. Existing `create`/Desktop output/default/GUI остаются отдельными. [Безопасный CLI/API recipe](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs2-recoverable-bootstrap-recipe.md).

**Все четыре focused CS2 tests и final strict Clippy прошли. Есть отдельно сохранённая неуспешная compatibility-проверка прежнего legacy `create`, описанная ниже; полного «все проверки зелёные» не заявляю.**

## Поведение и границы

- До secret creation проверяются два distinct context IDs, отсутствие destination, bounds/JSON encodability конечного пути. Atomic `create_dir` резервирует новый workspace; появившаяся folder/file не может быть заменена. Existing/unknown/incomplete workspace всегда отвергается.
- `account-root` и `device` — sibling directories. `AccountRootState::create_recoverable`, текущие signing/encryption identity primitives, messaging certificate/authority и **`RuntimeRepository::create`** создают точное Runtime6 enrollment. Bootstrap не зависит от CLI app. Peer/context ceremony и session до CS1 не выдумываются. Нет вызовов DeviceState/RatchetState legacy initialization, `migrate_legacy_snapshot`, Root7 или migration activation. Runtime6 constructor оставляет свой штатный anti-downgrade fence; это не legacy session.
- Экспортируются реальные RootVerifiedCredential bundle, bare certificate, authority и device-list snapshots. Новая `Ordered2BootstrapOutput` имеет version/mode, public account/device/claim/context/path data и secret recovery phrase. Paths имеют фиксированные роли; no fictitious prekey artifact. Recovery phrase проверяется против Account ID, редактируется в Debug, zeroizes on drop; stdout bytes также zeroizing. Public receipt/exports phrase не содержат. JSON decode errors не цитируют secret input.
- `bootstrap-pending.json` означает reservation, `bootstrap-candidate.json` — проверенные данные. Protected handles закрываются; repository и Root открываются заново **по конечным путям**, проверяются owner/claim/device/contexts/certificate/authority и пригодность сохранённых ключей. Последняя no-clobber запись корректного `bootstrap-ready.json` и успешный API/CLI result завершают создание. Directory rename не используется.
- Failure после reservation сохраняет incomplete workspace; нет auto-reset/resume/delete/reuse. Это одна bootstrap creation transaction с ready commit point, **не** обещание атомарной видимости всей directory через rename. Файловые sync/DB transactions переиспользованы; hardware power-loss durability не объявляется. Потерянный после commit stdout не даёт автоматического повторного вывода phrase.
- Phrase восстанавливает Root key/Account identity. Authority history требует package и отдельно удерживаемый witness; message/device history отдельно. Реальный `AccountRootState::recover` с phrase+package+witness дал тот же Account ID. Full recovery UX не реализован.

## Exact source/config scope — 11 files

1. `apps/kilogram-bootstrap/Cargo.toml` — direct dependency на существующий protocol crate.
2. [bootstrap/src/lib.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-bootstrap/src/lib.rs) — одна module declaration.
3. [bootstrap/src/main.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-bootstrap/src/main.rs) — новый command, Zeroizing stdout buffer; existing JSON shape/dispatch сохранены.
4. [bootstrap/src/ordered2.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-bootstrap/src/ordered2.rs) — direct factory, reservation/ready commit, final-path verification.
5. [bootstrap/src/ordered2_tests.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-bootstrap/src/ordered2_tests.rs) — success/recovery и collision/interruption negatives.
6. `crates/kilogram-bootstrap-contract/src/lib.rs` — только module/re-export.
7. [contract/src/ordered2.rs](E:/AI-Projects/kilogram-p2p-messenger/crates/kilogram-bootstrap-contract/src/ordered2.rs) — отдельный bounded secret output/metadata, redaction/zeroization/shape tests.
8. `apps/kilogram-cli/Cargo.toml` — dev-only dependency на bootstrap-contract.
9. `apps/kilogram-cli/tests/runtime2_process.rs` — один CS2 test module.
10. [cs2_bootstrap.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/runtime2_process/cs2_bootstrap.rs) — actual child CLI, private stdout parsing, existing CS1 ceremony/import and real runtime open.
11. `Cargo.lock` — ровно две direct dependency additions, без version changes. Временный эксперимент с Linux rustix полностью удалён.

No state/identity/runtime/CS1 implementation edits, GUI/default switch, dynamic contacts, migration run, new cryptography, wire or DB design. Three paused BG files и accepted CS1 CLI сохранены byte-identical.

## Проверки и воспроизводимость

Existing `scripts/run-cargo-tests-stable.ps1` с `-Package kilogram-bootstrap,kilogram-bootstrap-contract,kilogram-cli -Filter cs2_ -Profile verification -CargoJobs 2 -Run -NoCapture -SkipCacheStatus`: Cargo compile-only, затем fixed-name EXE. Target — process-local `target/s54-diagnostics-source-checks`; NativeToken/BelowNormal/offline. Новый bootstrap EXE — `target/clean-start-runtime/kilogram-bootstrap.exe`, SHA256 `baa3de3dec6b4fa53421c1dd3c54f429a44425240423c9bbbed521e9d3abecb4`. Принятый CS1 CLI переиспользован без изменения (`c06225fe…`).

Final check04 strict Clippy all-targets трёх affected packages `--locked --offline -- -D warnings`: child/supervisor0,26.0345679s, cleanup nonforced/empty. Scoped rustfmt и whitespace прошли. Final test03: **child/supervisor0**,22.0743221s, bounded EOF/wait/Job verification, cleanup before=[]/after=[], nonforced.

В test03 реально прошли:

- Direct creation, повторное открытие Runtime6 по final path с точным owner/enrollment/keys, реальный Root recovery, exports verifier, receipt/Debug redaction; отсутствуют legacy private-device/prekey artifacts и invented peer/install.
- Existing unknown destination byte-identical, invalid contexts до filesystem/secret creation, raced empty destination не заменяется, interruption до/после reservation и до ready commit не даёт successful output; ready-file collision не перезаписывается; incomplete повторно не используется. Это controlled fault hooks, **не** physical power-loss test.
- Output roundtrip, invalid mode/version/IDs/path/phrase/extra fields/oversize, отказ интерпретировать fresh output как legacy Desktop output.
- Actual CLI `create-ordered2` **дважды**, stdout parsed только в Zeroizing memory без logging/export. Обе CS1 ceremonies (reverse public order; peer Root path unavailable per signer) и actual final imports прошли на обеих fresh repositories. Profile2 созданы, оба настоящих Runtime6 процесса открыли два контекста/status и graceful exited0. Повторное доказательство всего CS1 transport smoke не требовалось; offline/restart/endurance/rotation/desktop не запускались.

Final native inventory:0 exact-image survivors по двум CLI и семи harness paths; accepted HEAD/index сохранены, Root/private historical data не читались. Администрация не открывала/сохраняла secret bootstrap stdout или generated private stores. Evidence — `.tmp/clean-start-cs2`; process fixtures retained under `.tmp/clean-start-cs2/process/CS2-*` без private export/pins.

## Неудачные проверки и исправления

- check01: Clippy child101/supervisor1 (`chunks_exact`), исправлено на `as_chunks`; original source/output сохранены. Subsequent strict checks прошли.
- test01: первая реализация ошибочно полагалась на Windows `std::fs::rename` как no-clobber directory publication. Реальный test заменившейся empty destination выявил дефект (bootstrap native101, wrapper1). Исправлено на early atomic reservation + no-clobber ready commit; weakened assertion/обхода нет. Исходные два файла и outputs сохранены.
- test02:4 focused tests passed, но combined compilation supervisor1 сделал forced owned cleanup. Компиляция отделена, final cached test03 прошёл0/0 nonforced. build01/02/03, compile01 и test01/02 с forced owned residuals сохранены; индивидуальные executable identities не были захвачены, имена helpers не приписываются. Все owned_after=[]/cleanup verified. Все actual step codes/times/PIDs есть в companion manifest/result-facts.
- Compatibility old Desktop output: compat02 **0/0**,10.391043s, прежний roundtrip/redaction test passed.
- Compatibility old `create`: compat01 **child1/supervisor1**,4.3255296s, native test101: `commit initial device state to encrypted vault` → `Table 'vault-meta-v1' does not exist`. Старый `create_account` body и Desktop contract byte-for-byte эквивалентны HEAD после удаления только новых module declarations; state/identity source не менялись, protocol был transitive dependency и раньше. Это указывает на прежний/окруженческий legacy дефект, **но baseline replay не проводился**, поэтому не объявляю его доказанно pre-existing или исправленным. Legacy DB ремонт вне CS2 clean-start scope; failure сохранён для review, совместимость по этому тесту не считается пройденной.

Development watchdogs900/930s для source/build/CS2 и90/120s для тёплых compatibility tests; existing inner integration150s. Ни один timeout/product deadline не расширялся ради pass. Неудачные attempts не перезаписывались. Implementation commit/push не выполнены; exact candidate передаётся на независимый review **с раскрытым legacy qualification**.
