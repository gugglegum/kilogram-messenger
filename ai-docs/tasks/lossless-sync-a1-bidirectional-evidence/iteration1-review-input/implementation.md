# OD1: реализация двустороннего диалога

2026-10-23. Отчёт исполнителя для независимого code review. T1–T12 проверены
в границах ниже; продуктовый diff **не принят и не закоммичен**.
Основание — [задание](lossless-sync-a1-bidirectional-task.md),
[контракт](lossless-sync-a1-bidirectional-contract.md) и [review C1–C3](lossless-sync-a1-bidirectional-contract-review-2.md).
Прямое разрешение человека проверено: userMessage `01a1661b-1561-7ec0-9380-b2835fd35738`
в чате оркестратора. Исходный design-этап завершён ранее; это следующий явно разрешённый этап.

## Ревизии и артефакты

Принятые семь документов сохранены commit `47d19c41be449deea9c1af003f7a27cb6992710c`;
обычный push в origin/master успешен, независимый ls-remote подтвердил этот SHA.
Это implementation baseline; продуктовый код соответствует принятому `dcc1300b759d45b0dfbb77e08b93d2cac0957b80`.
Contract SHA-256 `587cf786cff1cb4e946a68d37ea508f82141b27e4909e80b2b95236485f54b18`;
iteration1 archive `77c0c69babc7363353bcc422d144c057c374382a993560192415bdfca37686ab`.

До первого изменения кода собран настоящий old CLI:
`target/compatibility/ordered-dialog-baseline/kilogram-cli.exe`, SHA-256
`df4ae0d4760c85eb41ab833655061875472e4054295844d452137859f7d4c58c`.
[Old manifest с 524 source/manifests/scripts hashes](lossless-sync-a1-bidirectional-evidence/old-baseline-manifest.json),
[build log](lossless-sync-a1-bidirectional-evidence/old-baseline-build.log).
Это pre-change executable, не новый код с выключенной функцией.

Финальный CLI: `target/field-tests/ordered-dialog/kilogram-cli.exe`, SHA-256
`532a5a76e836faacca63a71306ba10f1129718e7b0aa157e363f58bf7642ce93`.
[544 source hashes](lossless-sync-a1-bidirectional-evidence/frozen-source-manifest.json),
[полный diff, включая новые файлы](lossless-sync-a1-bidirectional-evidence/review-code.patch),
[build](lossless-sync-a1-bidirectional-evidence/typed-admission-build.log),
[повторная сборка с тем же EXE](lossless-sync-a1-bidirectional-evidence/final-typed-build-confirmation.log),
[индекс process evidence](lossless-sync-a1-bidirectional-evidence/process-run-index.json).
Полный code patch:29 файлов, SHA-256 `a392d15d9102774799d22793845fb224105e7b931058ab1b034cb0f5ea4ce697`.
[Финальная сверка](lossless-sync-a1-bidirectional-evidence/final-review-checks.json): source drift0, индекс пуст,47 архивных process runs.
EXE, секретные profiles, IPC bearer descriptors и private pickles в Git не включаются.
Архивируются synthetic history, публичные Head/counts, текстовые traces/results.

Первый baseline build и несколько ранних check ошибочно использовали default CargoJobs=12.
После обнаружения все build/check явно используют `-CargoJobs 2`; stable runner получал2
с первого запуска. Его housekeeping отдельно печатает default12, без compilation/run.
Один широкий фильтр `ordered` захватил старый дорогой acceptance: он остановлен и не
объявляется PASS (`components-limits-native-token.log`). Далее применялись узкие фильтры.
Другие неуспешные попытки/repairs сохранены, включая sandbox ACL failures и ошибки harness.

## Реализованный путь и карта diff

- Fresh opt-in schema4, immutable claim/mode, mandatory feature в keyed snapshot hash,
  account fence, v2 journal и lifetime OS lease. Generic CLI/runtime/transactions/vault
  writers отказывают для OD1. Commit использует authenticated manifest index и явный delta;
  whole-vault verify/directory scan выполняются при startup/recovery, не на каждое сообщение.
- Fresh native Olm account/session на C, public pickle APIs, фазовые HPKE blobs по C path.
  OfferedA не требует D/E/SID, GrantedB не требует SID. Head/hash/AAD/phase/generation,
  Account curve, SessionKeys/SID проверяются без shadow fallback/account regeneration.
- Actual Iroh QUIC/TLS exporter, signed Hello/Proof/Admitted, InitialPair,
  durable Offer/Grant/OTK reservation, A0/B0/Ready. Только A dialer/session creator;
  один admitted/authenticating connection на enabled C, bounded sockets/streams/backoff.
- Две lanes через DH transitions: ordinal/predecessor, authenticated closure/cause,
  actual native header и inner/outer binding. Native trial применяется после guard;
  legacy unknown-tail gate сохранён. Control/ACK/prefix не расходуют Olm messages.
- Stable request16; atomic materialization native/event/projection/outbox/source ledger;
  byte-exact retries. Possession Complete отделён от readable/causal/Delivered.
  Immutable receipts, durable ACK intents Pending/Attempted/Sent, query/reconciliation.
- Ingress64 ordinary +1 head; drain≤8 атомарно. IO/unknown failures откатывают текущий
  workspace, invalid input сохраняет durable prefix/quarantine. Нет eviction/reset/subset
  Complete; bounded signed-fork proof блокирует только свой C.
- Receiver-driven prefix repair/auto drain после reopen. Source хранит весь L1 ciphertext,
  включая Delivered. A maintain стартует из profile при пустом queue; idle30s/timeout15s,
  reauth240s/lease≤300s. Locator refresh/import явный; expiry не меняет native keys.
- Bounded verified credential/policy/history metadata caches. Writers читают текущие
  authenticated authority/consent bytes и проверяют время. History cache не хранит тексты:
  projection/AAD проверяются на каждой странице. Совпадающий startup shadow не переписывается.

| Модули | Изменение |
|---|---|
| protocol/ordered_dialog_v1, transport-iroh/ordered | Restricted deterministic CBOR,17 kinds/tags, framing/exporter; no downgrade |
| ratchet/ordered | Filesystem-free public native APIs, encrypted pickles, typed failures, version1 fence |
| state/vault/ordered, state/vault guards | Mode/schema/account fences, owned lease, incremental atomic v2 writer/recovery, budgets |
| identity/account | Domain-specific Root consent sign/verify; runtime Root secret не загружает |
| cli/ordered_dialog/* | Ceremony, handshake, guard/head, outbox/ingress/repair/receipts, реальные actors/history |
| cli/main, ordered_cli, runtime-ipc | Thin explicit mode bridge/authenticated IPC, ordinary compatibility |
| scripts/test-ordered-dialog-*, targeted tests | Настоящие процессы/kill/reopen/old EXE; scheduler задерживает committed records |

Реальные команды: `account-create`, `device-enroll --profile-mode ordered-dialog-v1`,
`ordered credentials-export`, `context-create/import` с expected account и двумя Root consents;
`runtime-profile-create --profile-mode ordered-dialog-v1 --ordered-context C --locator-directory DIR`,
`runtime-from-profile`. При живом owner: `ordered locator-import`, `locator-export --refresh`,
`open`, `queue --request-id HEX32 --text TEXT`, `sync`, `status`, `history`,
`authority-import`, `shutdown`. Offline `ordered inspect` читает только публичный Head.
Успешный queue означает durable enqueue/idempotency, не peer Delivered.

## T1–T12: фактические результаты

| Gate | Evidence и наблюдение |
|---|---|
| T1 | [Final duplex](lossless-sync-a1-bidirectional-evidence/accepted-candidate-duplex.log):256 Text,128/сторону, exact IDs/request IDs/UTF8. [Тот же EXE после обоих relaunch](lossless-sync-a1-bidirectional-evidence/accepted-candidate-reopen-attempt2.log):256 exact persisted records,1040 immutable/native hashes; graceful IPC Shutdown |
| T2 | Обе receiving lanes g=16, actual key changes/index resets проверены guard, общий SID не меняется. Component доводит каждую lane до512 настоящих Olm frames |
| T3 | [Final gap](lossless-sync-a1-bidirectional-evidence/final-gap-current-binary.log): A2/B1 held, A3..65+A66(new DH) дают64 pending при next2/g1; kill/relaunch, head-only A2, drain до67/g2,67 exact Text и Delivered |
| T4 |14 final-crash logs: Offer/Grant, before/after Init/Confirm cutover, receive Confirm/Ready, Queue/materialization/pending/trial/readable/primary. Head доказывает фазовые отсутствующие IDs; loader и следующий реальный обмен проверяют consistency; committed hashes не меняются |
| T5 | Настоящий child Stop-Process kill по fsynced marker. [Primary-before-shadow/ACK](lossless-sync-a1-bidirectional-evidence/final-primary-crash-after-history-cache.log) и `final-crash-*.log`. Redb injected failure/reopen и I1 IO seams отдельно от kills; все power-loss окна ОС/fsync не заявляются |
| T6 | [Final C3, текущий EXE](lossless-sync-a1-bidirectional-evidence/accepted-candidate-partial.log):47+17=64, requested32/saved17→RoundError; no subset Complete. Kill, Prefix[2,3) ordinary0/head65536, drain до67, Prefix[67,82),81 exact Text/receipts. Duplicates/lost ACK/retry также в14 crash cases |
| T7 | [Actor components](lossless-sync-a1-bidirectional-evidence/final-actor-after-history-cache.log): real Olm signed forged g/index/tail/cause, inner mismatch/MAC failure не продвигают guard; затем authentic decrypt. C1 swapped/stale/nonce/scope blobs отказаны. Это component, не malicious physical-host acceptance |
| T8 | [Final two contexts](lossless-sync-a1-bidirectional-evidence/final-two-contexts-current-binary.log): C1 gap сохраняется при duplex C2, разные SID, interleaved author_sequence отличается от lane ordinal, обе истории точны |
| T9 | [Storage](lossless-sync-a1-bidirectional-evidence/final-storage-after-history-cache.log), [wire bounds](lossless-sync-a1-bidirectional-evidence/final-wire-bounds.log), native512/full64/reopen. Index2,712,820B/18,958 records; full old/new plan6,740,456B. Max-text event4,978B; Selected blob37,749B. Opaque maximal encode vectors: normal8 full wire262,702B, head33,081B, stored head50,176B; full32 refused. Vectors не заменяют credentials/process acceptance. Overflow/downgrade/atomic rollback проверены |
| T10 | [Old EXE/live old owner](lossless-sync-a1-bidirectional-evidence/final-old-binary-fences.log), [unsupported signed suite](lossless-sync-a1-bidirectional-evidence/final-unsupported-suite.log), [signed Offer fork/restart](lossless-sync-a1-bidirectional-evidence/final-control-fork.log), [consent expiry](lossless-sync-a1-bidirectional-evidence/final-consent-expiry.log), final B-only и lease/locator/revocation. [Actual mixed ALPN/exporter](lossless-sync-a1-bidirectional-evidence/final-transport.log). Locator import не снимает revocation/cache block; clock jumps обозначены targeted, не ожиданием30 дней |
| T11 | [I1–I4](lossless-sync-a1-bidirectional-evidence/i1-i4-regressions.log):6 PASS; [ordinary IPC](lossless-sync-a1-bidirectional-evidence/ordinary-runtime-ipc.log):1 PASS; [legacy readable](lossless-sync-a1-bidirectional-evidence/legacy-readable-history.log):1 PASS |
| T12 | [Performance](lossless-sync-a1-bidirectional-evidence/performance.json): phase totals, queue/commit/history distributions, counts/sizes; границы ниже |

Новые component checks: protocol2 +wire-bound1, native1, storage3, transport2,
actor3 — **12 PASS**. rustfmt/diff whitespace и [locked/offline strict Clippy](lossless-sync-a1-bidirectional-evidence/final-typed-admission-clippy.log)
прошли. Полный workspace/mutation corpus и physical two-host/cross-network не запускались.

Последнее уточнение: ранние wire/scope/missing-author-record ошибки admission возвращают
typed Invalid, не StorageBlocked. Local IO/unknown failures остаются OperationalFailure.
[Missing credential](lossless-sync-a1-bidirectional-evidence/final-missing-author-credential.log) и
[cross-context ciphertext](lossless-sync-a1-bidirectional-evidence/final-cross-context-ciphertext.log)
проверены через настоящий admitted TLS: valid Device-signed metadata отклонены до
pending/native/ACK; затем исходный committed ciphertext доставлен ровно один раз.
[Адресные actor checks](lossless-sync-a1-bidirectional-evidence/final-typed-admission-components.log) и
[итоговый strict Clippy](lossless-sync-a1-bidirectional-evidence/final-typed-admission-clippy.log) повторены.
Предыдущий performance fingerprint `f32c6650…` сохранён в process manifests; последний
основной прогон — [candidate duplex](lossless-sync-a1-bidirectional-evidence/accepted-candidate-duplex.log), reopen и C3. Остальные process gates в таблице сохраняют fingerprint f32c6650…; последующий product diff ограничен typed ранним admission, проверенным адресно.

## Воспроизведение и измерения

Все Windows harnesses только stable runner после compilation без запуска.
Нужен native user token для ACL/DPAPI/loopback: sandbox PermissionDenied/10013 не
объявляются продуктовыми failures или PASS. Новые EXE имена фиксированы; SHA проверяется по содержимому.

```powershell
$env:CARGO_NET_OFFLINE='true'
& scripts/invoke-cargo-friendly.ps1 build -CargoJobs 2 --locked --offline --profile verification --package kilogram-cli --bin kilogram-cli
New-Item -ItemType Directory -Path target/field-tests/ordered-dialog -Force | Out-Null
Copy-Item target/verification/kilogram-cli.exe target/field-tests/ordered-dialog/kilogram-cli.exe
& scripts/test-ordered-dialog-process.ps1 -Scenario Duplex -MessagesPerLane 128 -Burst 8
# RunDirectory напечатан предыдущей командой; используется тот же реальный state:
& scripts/test-ordered-dialog-reopen.ps1 -RunDirectory <RunDirectory>
& scripts/test-ordered-dialog-process.ps1 -Scenario Gap
& scripts/test-ordered-dialog-process.ps1 -Scenario Partial
& scripts/test-ordered-dialog-process.ps1 -Scenario TwoContexts
& scripts/test-ordered-dialog-process.ps1 -Scenario BOnly
& scripts/test-ordered-dialog-process.ps1 -Scenario Expiry
& scripts/test-ordered-dialog-process.ps1 -Scenario ConsentExpiry
& scripts/test-ordered-dialog-process.ps1 -Scenario Conflict
& scripts/test-ordered-dialog-process.ps1 -Scenario UnsupportedSuite
& scripts/test-ordered-dialog-process.ps1 -Scenario Crash -KillPoint readable-primary
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter 'ordered_dialog::' -CargoJobs 2 -Run -NoCapture
```

Все14 kill points перечислены в скрипте. Fences script принимает завершённый normal-time
RunDirectory; old executable обязан иметь baseline hash выше. Test scheduler не создаёт
accounts/session/credentials/handshake вместо actors и не вызывает receiver put напрямую.

Windows laptop, unoptimized verification profile: setup/auth4.84s, queue/direct/ACK/polling
exchange207.49s, обе256-record histories13.94s, total226.28s. Queue CLI p50/p95=213/350ms.
1133 commits (293 native), commit p50/p95=79/139ms; max delta plan601,479B.
Сумма планов317,511,379B **не** измерение физических disk writes; redb files4,902,912 и4,493,312B.
Цена — несколько durable transitions/fsync на сообщение, signature checks и CLI/polling overhead.

После обоих relaunch: reopen1.92s, history+reauth13.08s, page p50/p95=125/135ms.
Ранее повторная проверка всей истории на каждой странице занимала143.92s только на history;
исправлены bounded metadata cache и лишняя перезапись shadow. Кеш не подменяет authority/AEAD.
Burst queue-return→sampled Delivered upper bound p50/p95=5.47/7.19s: overestimate до общей
проверки партии, не per-message RTT. Direct RPC full64 trace p50/p95=1293/1419ms (12 samples);
head-prefix RPC158ms (1 sample; последующий caller drain не включён). Это не release benchmark
и не утверждение daily-MVP throughput; старые1095s не переносятся в критерий приемлемости.

## Ограничения и остановка

L1:2 contexts/profile,1 epoch/context,512 encrypted/lane **включая genesis** →511 Text/lane.
Prepaid128MiB/context,334.25MiB conservative reservation/profile при logical384MiB,
physical staging2GiB. Sender128 unacked +128 queued; receiver64 ordinary +1 head.
При исчерпании lane новая materialization останавливается, old ciphertext/ACK repair
остаётся доступен. Нет silent eviction/reset/replacement epoch.

Нормативных развилок контракта не оставлено. Нет migration/default rollout, OD1 mailbox/relay,
GUI/onboarding, groups/multi-device, A3 или M1.2.63. Старые profiles/history не объявляются
мигрированными. **A2 остаётся security gate до внешнего использования.**

Один OTK/full-guard JSON dump отклонён автоматической проверкой безопасности и не добавлен:
используются public Head, opaque hashes и consistency tests без native dump; запроса доступа нет.

Чужой `ai-docs/development-environment.md` сохранён вне diff, SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Далее — содержательный callback архитектору и остановка до code review.
Продуктовый commit/push только после явной приёмки и поручения оркестратора.
