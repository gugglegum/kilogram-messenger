# RFC-0203: M1.1.76 sustained canonical sync v2 decoder campaign

Статус: принято локально 2026-10-18.

## Результат

На чистом committed source `eaf3d75bb16ce42c8368017a68c6f4183d15ab9f`
выполнен единственный instrumented `canonical_sync_v2_decoders` run. Исходная
ревизия и состояние worktree до и после запуска совпали. Один worker/job за
28,801 seconds выполнил 469,853,771 decoder executions, начав ровно с 20
checked-in seeds и завершив 1,043 corpus files. Exit code равен нулю;
generated failure artifacts: 0.

Run использовал закреплённые Rust nightly `1.101.0-nightly`, compiler commit
`5d8fec0669a64607db9ccbf663e7c23125bf4748`, LLVM `23.1.4`, cargo-fuzz
`0.13.2`, libFuzzer и AddressSanitizer. Coverage profile успешно прочитан
`llvm-profdata show`: front-end instrumentation, 888 functions, 2,010 blocks
и total count 885,327,009.

## Evidence contract

Tracked manifest `CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json` сохраняет exact
source/toolchain/run metadata и размеры с SHA-256 для machine record, run log,
coverage log и binary profile. Репозиторий does not require ignored raw
evidence to exist on every clone: при наличии локальных файлов boundary
пересчитывает их размеры и digest, а portable clone проверяет tracked digest
contract.

Fail-closed boundary отвергает сокращённую длительность, ненулевое число
artifacts, dirty или изменившийся source, premature activation и рассинхрон
aggregate manifests. Исторические M1.1.74 toolchain и M1.1.75 smoke records не
переписываются.

## Граница принятия

Этот результат закрывает обязательный локальный sustained coverage-guided
decoder gate. Он не является доказательством отсутствия всех ошибок и не
активирует новый transport. Fresh hosted and physical-device evidence remains
pending. M1.1.76 does not register or dial `kilogram/m1/sync/cbor/2`; active
Postcard protocol, wire, runtime и CLI остаются без изменений.
