# OD1 bidirectional: implementation accepted after repair

Дата: 2026-10-23. **Реализация принята в границах согласованного OD1 этапа. R1–R4 закрыты.**
Поручение следующего действия — сохранить принятые code/docs/evidence через commit/push
тем же исполнителем. Новый функциональный этап этим review не запускается.

## Точный принятый candidate

Baseline HEAD `47d19c41be449deea9c1af003f7a27cb6992710c`; index пуст до поручения.

| Артефакт | SHA-256 |
|---|---|
| [Сданный report](lossless-sync-a1-bidirectional-implementation.md) | `aca3886b99fed0f005777a65c7b753be04320a793e2824e091aa8bd9090a2eee` |
| [Полный diff, 30 product/test/script files](lossless-sync-a1-bidirectional-evidence/review-code.patch) | `c6c7d6b42e421bb7ca86860274defa914d611d8c705032c4a7e82209e76fa13a` |
| [Repair относительно review1, 7 files](lossless-sync-a1-bidirectional-evidence/repair1-code.patch) | `2bdf56c2b3858ad3c9dfbf376dce2635e0d0f981224616bea233dc5b5d4b997c` |
| [Source manifest, 545 files](lossless-sync-a1-bidirectional-evidence/frozen-source-manifest.json) | `602b4982ef739ba68315a0e17ef480611f12d4218e4743efe075987d6aa8d4af` |
| Field executable | `3f42245b899e678c423b3137b5d99dc225cb0c2bc36d99b5bfa99daf506a6626` |
| [Final checks](lossless-sync-a1-bidirectional-evidence/final-review-checks.json) | `3b9c33eb88f835ece1dc4dc2258e6d7c5726bbd7e0feac01de38bd78f595d772` |

Оркестратор самостоятельно сверил 545 source hashes, все 30 artifact hashes и все50
архивных process result hashes: **0 drift**. Оба patch прошли reverse-check against
worktree; whitespace check прошёл. Сданный report/final checks остаются историческими
снимками до принятия, их pending-review формулировки не являются текущим статусом.

## Решения по замечаниям

- **R1:** первый выбор A привязан к exact current consent. Historical pair требует
  совпадающих consent refs обоих Hello, обеих подписей, scope/roles/keys/membership,
  transcript T и обоих Device Proofs. Собственный proof B подтверждает прежний admission,
  включая случай old Offer после renewal до первого Grant. Он не заменяет fresh lease:
  каждый caller проверяет current Root consent/authority, revocation и expiry.
  Это соответствует §5 принятого контракта, без изменения wire/schema или доверия
  произвольному peer consent. Selected pair/Offer/Grant/E/SID сохраняются.
- **R2:** fence-only root без durable claim теперь fail closed с CreationConflict.
  Fence не является удостоверением владельца. Resume собственного committed claim
  проходит существующий open/enrollment путь и использует сохранённые keys; другой
  owner получает отказ. Стирания, adoption или ослабления old-binary fences нет.
- **R3:** новое enqueue учитывает оставшийся lifetime и уже принятую очередь;
  за пределом возвращается UpgradeRequired. Проверка ранее принятого request ID
  остаётся раньше capacity gate. Status и actor явно показывают exhaustion, но receive
  противоположной lane, old ciphertext/prefix/ACK repair и history продолжают работать.
  Старые принятые queue bytes не удаляются. Никакого нового epoch/reset.
- **R4:** receiver проверяет canonical genesis request16 и пустые user parents
  до загрузки native state/decrypt. Согласованные malicious Core/Inner с валидной
  подписью отвергаются; исходный genesis после отказа принимается.

## Проверки и применимость

Оркестратор повторил на финальном source через stable runner:

```powershell
$env:CARGO_NET_OFFLINE='true'
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter 'ordered_dialog::' -SkipTest 'ordered_dialog::repair_tests::review_r3_engine_ipc_lifetime_capacity_reopen_receive_and_repair' -CargoJobs 2 -Run -NoCapture
```

**6 PASS, 0 failed,40.32s**: R1/R2/R4 и прежние три guard/phase/native components.
[Независимый лог](lossless-sync-a1-bidirectional-evidence/root-review2-fast.log).
Harness hash `71cba39751729de597809fa3d725fbe8a3bed948d2a7b525d4625bd8735e306c`
совпал со сданным. Использован native Windows token; network harness paths стабильны.

Тяжёлый R3 принят по проверенному коду теста и evidence исполнителя: настоящий engine/
IPC/native/vault на511 Text, reservation двух последних queue slots, отказ нового request,
idempotency/old queue byte equality, reopen, opposite receive, history512, exact retries,
prefix serving и ACK. Его full fixed run4/4 прошёл за828.80s. После него менялась только
обработка ошибок test fixture (два unwrap), exact test-only diff/provenance сохранены;
product EXE не изменился. Повтор14-минутного R3 после этих правок не требовался.
Component admission lease явно не выдается за настоящий TLS.

На финальном EXE исполнителем выполнен настоящий duplex64 (32/сторону), минимум8
receiving DH transitions/сторону, same SID и exact history; reopen64 проверил272
immutable/native hashes. Old/live binary fences, storage3, strict Clippy7 crates/all
targets и rustfmt прошли. Полный256/C3/full64/crash/2C/expiry corpus сохраняет прежние
fingerprints, как перечислено в report. Самостоятельный root full64/DH/kill/reopen PASS
из review1 относится к532a5a76, не к3f42245b. Unchanged ingress/guard/repair/history и
адресные новые regression gates обосновывают reuse этих результатов. Не заявляется,
что все исторические прогоны были повторены на одном EXE или покрывают все power-loss окна.

## Сохранение и границы результата

Сохранить только принятые30 code/test/script paths и связанные документы/evidence.
Файл `lossless-sync-a1-bidirectional-accepted-files.json` рядом с review фиксирует
allowlist и ожидаемые Git blob IDs с учётом нормализации line endings. Состав и blobs
индекса проверить перед commit, а оркестратор сверит их с committed tree и remote SHA.
Сам allowlist не содержит собственный hash; оркестратор фиксирует его отдельно.
Новые commit-result записи допустимы только отдельным последующим docs change;
принятые source/evidence перед commit не переписывать. Force push не разрешён.

Чужой `ai-docs/development-environment.md` исключён, неизменный SHA-256:
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
В evidence только текстовые logs/JSON/patch/Markdown с синтетическими test histories;
private profiles, IPC bearer, native pickles, secret dumps и EXE в commit не входят.

Принят **opt-in local two-process OD1**: до2 независимых C, одна epoch/C,
511 Text/lane, operator-assisted locators. Это завершение данного протокольного и
runtime этапа, не всего lossless-sync/user MVP. Нет миграции обычных profiles,
публичного включения, внешнего cross-network acceptance, GUI/onboarding, multi-device,
groups или A3. A2 остаётся gate до внешнего использования; M1.2.63 не запускается.
