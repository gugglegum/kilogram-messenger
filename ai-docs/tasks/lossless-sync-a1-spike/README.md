# A1 F1/F2: offline feasibility spike

2026-10-23, implementation HEAD `3e82e6380c30d0f9443b6692ea34faccaacddef7`.
Задание: [review R1–R4](../lossless-sync-a1-review.md).
Один standalone executable, две секции: [F1](src/f1.rs) и [F2](src/f2.rs).
Никаких sockets, GUI, production crate edits, registry patches или migration.
Это component/model evidence; весь A1 этим не закрыт.

Последующее [независимое review](../lossless-sync-a1-spike-review.md) подтвердило
исходный run и воспроизвело три ограничения: invalid после pending/reload,
ordinal вместо exact EventId в F2 и отсутствие постоянного moving head reserve.
Команды дополнительных probes: `./ai-docs/tasks/lossless-sync-a1-spike/evidence/review-probes.ps1`
из корня repository. Они не меняют submitted sources и не являются исправлением A1.

## Воспроизведение

Из корня repository, Windows PowerShell:

```powershell
$env:CARGO_NET_OFFLINE = 'true'
New-Item -ItemType Directory -Path '.tmp' -Force | Out-Null
$env:TEMP = (Resolve-Path '.tmp').Path
$env:TMP = $env:TEMP
& .\scripts\invoke-cargo-friendly.ps1 -CargoCommand build -CargoJobs 2 `
  -CargoArguments @('--locked', '--offline', '--manifest-path', `
    'ai-docs/tasks/lossless-sync-a1-spike/Cargo.toml', '--target-dir', 'target')
if ($LASTEXITCODE -ne 0) { throw 'Spike build failed' }
& .\target\debug\kilogram-a1-feasibility.exe
if ($LASTEXITCODE -ne 0) { throw 'Spike invariant failed' }
```

EXE path постоянный: `target/debug/kilogram-a1-feasibility.exe`. Build использует
jobs=2/BelowNormal resource policy; тестовый harness через `cargo test` не запускается.
Свой [Cargo.lock](Cargo.lock) закрепляет resolution; workspace lock не изменялся.
Единственный первоначальный `cargo generate-lockfile --offline` сохранён в
[логе](evidence/lock-generation.log); при воспроизведении он не нужен.
Olm точно `=0.10.0`. Синтетические Olm keys случайны, порядок действий/ожидаемые
состояния фиксированы. Два финальных запуска имеют одинаковый stdout/exit.
Нарушение любого `ensure!` возвращает nonzero; отрицательные случаи проверяют
конкретный исход и неизменность state, а не произвольное исключение.

## Фактические результаты

- [Build](evidence/build.log): exit 0; [rustfmt check](evidence/format.log): exit 0.
  Единственный build warning — копирование вместо hardlink в incremental cache.
- [Финальный run](evidence/run.log), [повтор](evidence/run-repeat.log): оба exit 0,
  `status=passed; production-a1-closed=false`.
- F1: по 256 проверенных plaintext на context (всего 512), дополнительно по
  8 проверенных reply plaintext; 8 реальных DH transitions на recipient/context,
  7 задержанных new-chain successors на context, 48 guard pickle/reloads.
  Во время ожидания canonical ratchet/frontier/readable bytes не меняются;
  после predecessor pending осушается. Native retention остаётся 5 chains,
  skipped keys после ordered drain отсутствуют.
- F1: 14 rejection/replay checks: nonzero new-chain index, same-chain gap,
  подписанный wrong ordinal/inner binding, wrong context/epoch/predecessor,
  bad signature/MAC, competing session, foreign session с подменённым outer SID,
  duplicate, fork, old-chain ciphertext в новой позиции. Ни canonical state,
  ни очередь не меняются; inner-binding failure проверяется после real decrypt
  в отброшенной копии Session.
- Legacy известного genesis: 256 exact plaintexts, reverse-first delivery,
  последовательный single-chain drain и native reload. Это **не** новый прогон
  production `SyncServer` baseline и не миграция произвольного старого профиля.
- Legacy `K0/K1/B0/L0`: `UnresolvedLegacyTail`, unchanged native pickle. Unsafe
  native control принимает L0, через следующие переходы вытесняет K chain;
  delayed K1 даёт ожидаемый `InvalidMAC`. Native reload другого existing pickle
  экспортирует local next=6 и retained skipped `[1,2,3,4]`; один skipped ciphertext
  действительно читается. Unknown serde schema отклоняется.
- F2: interrupted admissions `[5,4]` переживают restart как inventory `[4,5]`,
  без readable/causal/ACK. После `[3]` inbox полон; legacy снова присылает `[2,1]`,
  genesis `0` остаётся последним по hash: `CompatibilityBlocked/UpgradeRequired`.
- Prefix-capable модель дочитывает все 6 за 2 response с exact completion IDs;
  mailbox дочитывает 6 после 3 quota-rejected scans и 2 wraps. Cursor/deadline
  сохраняются; 9 virtual ticks пропущены throttling. Delete разрешён только readable.
- Full history → `StorageBlocked/FullCommitBudget`, хотя event bytes сами помещаются.
  Simulated physical write failure сохраняет accepted head, без ACK; после явного
  возврата capacity drain проходит. Oversized legacy → explicit StorageBlocked,
  без переподписания/удаления/ACK. Invalid input не меняет состояние.

## Что именно проверено

F1 использует реальные Olm sessions, MAC, `DeviceIdentity` signatures, public
message getters, `Session::pickle/from_pickle` и encrypted pickle disk roundtrip.
Test transcript связывает context/epoch, роли, Device IDs, обе Olm identity keys,
base key, OTK, SessionId и suite, подписан обоими test Devices до data writer.
Alice handshake занимает lane n=0, Bob confirmation — свою n=0; data начинается
с n=1, которое не совпадает с Olm chain index. Ровно одна agreed session;
второй candidate не получает права писать в тот же slot.

Trusted fixture задаёт credential admission, election initiator/responder и
OTK reservation. Production membership/certificate/revocation validation,
capability negotiation, races двух настоящих initiators и writer fencing
**не тестировались**. Подписи здесь не являются готовым wire protocol.
F1 readable map не является `LocalTextProjection` или vault transaction.

Выбран **version-pinned serde adapter** только для native local cursor inspection:
проверка shape/типов SessionPickle, ≤5 receiving chains, ≤40 skipped keys/chain,
index bounds и uniqueness; затем публичный `Session::from_pickle`.
Adapter знает поля `receiving_chains.inner`, `hkdf_ratchet.index`, skipped key
indexes; closed library fields читаются через их serde representation.
Изменение библиотеки/shape требует нового review; upstream API здесь не добавлялся.
`Debug`/max author_sequence не используются. Serialized native secret bytes
служат oracle в RAM, не пишутся в logs. Temporary synthetic state удаляется
при выходе; фиксированный test pickle key не предназначен для production.

Importer классифицирует `SameChainReady`, `RetainedSkippedKey`,
`MissingSameChainPrefix`, `UnavailableKey`, `UnresolvedLegacyTail`.
Он не доказывает sender tail length, application ordinal, global genesis или
существование вытесненных chains/keys. Unseen legacy DH key блокируется: никакой
презумпции «previous chain завершена» из native frontier нет.

F2 — детерминированная модель с fake ciphertext. Полная запись = 10 units,
projection=8, ratchet/frontier=4, ACK intent=2, staging/shadow=6; full lease=30.
3 ordinary slots + отдельный head slot и full lease. Общий model vault budget=200;
full-history case использует 185 занятых units. Model byte frame cap=28 включает
base=4 и item framing=2: два обычных события ровно помещаются. Production page
width 64 масштабирован до 2; моделируется правило первых missing hash IDs.
Новую byte/prefix negotiation старому sender модель не приписывает.
Один encoded event может не поместиться сам — нужен blocked outcome, не пустой retry.

F2 reserve покрывает accepted pending до полного readable commit; possession,
crypto next, readable и causal frontier разделены. При incomplete batch:
**round error/no SyncComplete**, durable subset только в checkpoint/следующем
inventory. Полный success требует exact equality sent IDs. Не подписанный отдельно
legacy SyncComplete не заменяется subset ACK. Mock Invalid — только queue-state
proof, не доказательство cryptographic admission. Disk/restart checkpoints здесь
regular file roundtrips; power-loss atomicity/512 MiB real vault accounting,
transport encoding 8 MiB и OS physical allocation этим не проверены.

## Границы и следующий review

[Уточнение proposal, R1–R4 и первый production slice](../lossless-sync-a1-proposal.md#5-f1f2-feasibility-spike--уточнения-после-review).
Два retained blocked epochs означают отказ третьей активации даже с consent;
owner может ждать, разрешить измеримое увеличение quota или отдельно решить
судьбу собственных данных. Автоматического erase/reset нет.
Проверка старого уже запущенного writer остаётся отдельным process gate.
Полный workspace/mutation corpus, runtime, GUI и physical/network tests не запускались.
Продуктовые изменения и полная миграция не начаты. Остановка для review результатов.
