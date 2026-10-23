# Reproduction and command results

Все команды выполняются из repository root на Windows. CargoJobs2/BelowNormal;
network harnesses только через stable runner. После разрешения зависимостей —
`CARGO_NET_OFFLINE=true`, locked/offline где поддерживается. Для ACL/DPAPI/loopback
нужен native Windows token: sandbox baseline получил IPC os error10013.

## Dependency advisory snapshot

```powershell
. scripts/cargo-resource-policy.ps1
$jobs = Set-KilogramCargoResourcePolicy -RequestedJobs 2
cargo install cargo-audit --version 0.22.2 --locked --jobs $jobs --root .tmp/transport-security-a2/audit-tools
git clone https://github.com/RustSec/advisory-db.git .tmp/transport-security-a2/advisory-db
git -C .tmp/transport-security-a2/advisory-db checkout --detach 84d943a6deeb8cc6c6f632531aaf4f1c62275239
cargo fetch --locked --target x86_64-pc-windows-msvc
& scripts/check-dependency-advisories.ps1 -AuditExecutable .tmp/transport-security-a2/audit-tools/bin/cargo-audit.exe
```

Final ожидаемый результат данного snapshot: source/version PASS, vulnerabilities0,
warnings7, **exit1**. CI получает fresh HEAD и передаёт `-DatabaseRevision <exact SHA>`;
повторить конкретный CI run можно checkout этого SHA. Tool version проверяется строго,
database dirty/mismatch и archive checksum mismatch дают отказ. Mutable yank state
не включён в этот snapshot (`--no-yanked`); выбранные 14 versions проверены через API.
Gate никогда не применяет `--ignore`.

Narrow resolve: manifest Iroh/relay=`=1.1.0`, точечный `cargo update -p <name> --precise
<version>` для таблицы в report; никаких workspace-wide updates. Первый candidate
resolver пытался выбрать 1.4.1 через caret transitives; собственный lockfile восстановлен
из baseline snapshot и нужные версии закреплены. `iroh-dns/base` также выбраны 1.1.0.
`cargo metadata --locked --offline --filter-platform x86_64-pc-windows-msvc` и tree
повторены после final patches.

Negative gate checks на копиях lockfile под `.tmp`: добавлена вторая affected Iroh
entry; checksum Iroh заменён нулями; передан нулевой database SHA. Все три дали exit1.
Baseline manifest/lockfile отдельно дали source FAIL. Это data validation tests,
без сетевого attack/reproducer выполнения.

## Functional verification

```powershell
$env:CARGO_NET_OFFLINE='true'
& scripts/invoke-cargo-friendly.ps1 build -CargoJobs 2 --locked --offline --profile verification --package kilogram-cli --bin kilogram-cli
Copy-Item target/verification/kilogram-cli.exe target/field-tests/ordered-dialog/kilogram-cli.exe
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-ratchet,kilogram-session,kilogram-transport-iroh -CargoJobs 2 -Run -NoCapture
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter 'ordered_dialog::' -SkipTest 'ordered_dialog::repair_tests::review_r3_engine_ipc_lifetime_capacity_reopen_receive_and_repair' -CargoJobs 2 -Run -NoCapture
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter 'runtime_outbox_delivers_and_automatic_sync_converges' -CargoJobs 2 -Run -NoCapture
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-protocol -Filter 'wire::tests::' -CargoJobs 2 -Run -NoCapture
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-protocol -Filter 'ordered_dialog_v1::tests::' -CargoJobs 2 -Run -NoCapture
```

Crypto3/identity24 прошли первыми в более широком запуске; protocol часть того запуска
остановлена намеренно, полный corpus не засчитан. Для повторения только этих завершённых
частей используйте stable runner с `-Package kilogram-crypto,kilogram-identity`.
Все исполнявшиеся harnesses имели стабильные aliases из runner; список/hashes — в
final checks. После удаления отложенной заготовки transport discovery содержит только
library harness и обычный `local_relay_transport` integration test.

Strict Clippy запущен через literal argument array: wrapper терял separator `--`.

```powershell
. scripts/cargo-resource-policy.ps1
$jobs = Set-KilogramCargoResourcePolicy -RequestedJobs 2
$clippyArguments = @('clippy','--jobs',"$jobs",'--locked','--offline','--profile','verification',
  '--package','kilogram-cli','--package','kilogram-transport-iroh',
  '--package','kilogram-crypto','--package','kilogram-identity',
  '--package','kilogram-ratchet','--package','kilogram-session',
  '--package','kilogram-protocol','--all-targets','--','-D','warnings')
& cargo @clippyArguments
cargo fmt --all --check
git diff --check
```

## Actual process checks

Baseline EXE был сохранён **до upgrade**, его hash3f42245b проверен; эти команды
не предполагают пересборку baseline из final source. Каждый process scenario требует
новый RunDirectory. В данном evidence использованы paths под `.tmp/transport-security-a2/`.

```powershell
& scripts/test-ordered-dialog-reopen.ps1 -RunDirectory .tmp/transport-security-a2/baseline-duplex-native -EvidenceName a2-upgrade-reopen
& scripts/test-ordered-dialog-process.ps1 -Scenario Duplex -MessagesPerLane 32 -Burst 4 -RunDirectory .tmp/transport-security-a2/final-duplex
& scripts/test-ordered-dialog-reopen.ps1 -RunDirectory .tmp/transport-security-a2/final-duplex
& scripts/test-ordered-dialog-process.ps1 -Scenario Gap -RunDirectory .tmp/transport-security-a2/final-gap
& scripts/test-ordered-dialog-process.ps1 -Scenario Partial -RunDirectory .tmp/transport-security-a2/final-partial
& scripts/test-ordered-dialog-process.ps1 -Scenario Duplex -MessagesPerLane 32 -Burst 4 -PeerExecutable target/field-tests/transport-a2/baseline/kilogram-cli.exe -RunDirectory .tmp/transport-security-a2/mixed-duplex
& scripts/test-ordered-dialog-reopen.ps1 -RunDirectory .tmp/transport-security-a2/mixed-duplex
```

Результаты PASS, hashes/PIDs и synthetic histories сохранены. Внешние сети и old/new
relay transport не проверялись. Вызовов отложенного security prototype здесь нет:
его запуск/разработка/отладка исключены из продолжения по task. Исходные отрицательные
логи сохранены как unresolved; они не являются доказательством fixed/affected поведения.
