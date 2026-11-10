# CS3.1: fresh Windows invite/join — результат для review

2026-11-10. Baseline `f7fd176d33c93eebe79d4d6fa0df385eca111290`. Реализован принятый **CS3.1** из [контракта](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-desktop-contract.md) с [review decisions](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-contract-review-01.md). **Настоящий Windows UI и focused проверки пройдены. Commit/push не выполнялись.** [Пользовательский и воспроизводимый рецепт](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-1-onboarding-recipe.md).

## Что доступно пользователю

Явный first-dialog pilot в существующем desktop и `--fresh-home` создаёт recoverable CS2 аккаунт через **Create invitation** либо **Inspect → compare identity → Join**. Inviter генерирует два distinct CSPRNG contexts и invitation ID до helper; joiner принимает exact ordered contexts до создания своего устройства. Ни ручного hex/JSON, ни placeholder peer, ни account recreation, ни отдельного аккаунта на каждый новый контакт.

Настоящие one-time words поступают в изолированный typed result, скрыты по умолчанию и доступны для private reveal. Явный saved checkbox + Hide сохраняет acknowledgement и очищает phrase state; до этого export невозможен. Публичные invite/reply содержат только version/kind/mode/invitation/ordered contexts/sender credentials+authority и exact recipient для reply. Existing protocol verifiers проверяют Root signatures, device/account/authority bindings и floors. Envelope bounded512KiB, artifacts16KiB decoded, canonical hex, duplicate/unknown fields и будущие kinds отвергаются. Внешнее сравнение peer identity обязательно; файл не называется аутентифицированным согласием.

После ack сохраняется no-clobber `exchange/invite.kilogram` либо `reply.kilogram`; повтор разрешён только при совпадении существующих bytes. Alice может inspect/confirm reply и durable закрепить peer. Финал — честный **awaiting mutual consent**, connection/send кнопка disabled. Fresh state не передаётся в `apply_bootstrap_response`, legacy profile/prekeys, readers, IPC или runtime launch. Старый default entry и интерпретация существующих profiles сохранены; Profile2/CS1 signing/messaging остаются CS3.2/3.

## Secret/process и durable progress

- [Helper](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-windows/src/fresh_onboarding/helper.rs) запускает только явно выбранный trusted executable/argv. Concurrent async draining stdout/stderr, независимые64KiB caps во время чтения,60s deadline, cancellation probe20ms, owned kill/wait с reap до5s. Нет `output()`/generic wizard formatter или интерполяции private stdout/stderr в ошибки. Buffers заранее выделяются до cap и zeroizing; phrase-bearing UI state не derives Clone/Serialize/Debug. Существующий CS2 output codec переиспользован без изменения.
- Worker владеет одним child task; cancel/leave/app drop дожидаются его cleanup, stale/cancelled output drops. Local generation/workspace/contexts и ready metadata должны совпасть до принятия ответа. UI не имеет command/filename из приглашения.
- [Model](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-windows/src/fresh_onboarding/model.rs) пишет bounded nonsecret `SETUP/setup.json` **вне отсутствующего CS2 workspace до helper**. Одна setup folder — один account setup; file lock не даёт двум UI одновременно обновлять record. Atomic temp+sync/persist обновляет только собственный record; новый record/exports используют no-clobber. Никакого plaintext draft, recovery phrase или private key в progress.
- Reopen различает incomplete, ready-unacknowledged и acknowledged-awaiting. Ready/candidate должны совпасть с локальными path/context/сохранёнными pins; проверяются signed credential/certificate/authority/device-list. Это проверка публичных bootstrap bindings, не новый cryptographic UI manifest и не разрешение запускать DB без Runtime6 owner verification в следующем slice. Arbitrary receipt existence недостаточна. Unknown/occupied workspace не удаляется/переиспользуется.
- Lost result/закрытая phrase view не ставят acknowledgement и не восстанавливают слова. User может подтвердить только реально сохранённые ранее слова; иначе остаётся blocked. Checkbox не доказывает наличие backup. Recovery copy отделяет Root identity, authority package/witness и message backup. Zeroizing state не объявляется очисткой всех renderer/OS allocations или защитой от screen capture.

## Exact source/config scope —8 files

1. `apps/kilogram-windows/src/lib.rs` — narrow pilot selection/render branch и explicit option; отдельный model не изменяет legacy launch draft.
2. `apps/kilogram-windows/src/fresh_onboarding/mod.rs` — panel/view, isolated worker lifecycle и user actions.
3. `apps/kilogram-windows/src/fresh_onboarding/artifact.rs` — invite/reply codec, public bindings/verifiers.
4. `apps/kilogram-windows/src/fresh_onboarding/model.rs` — pending/ready/ack/frozen peer, record/export IO.
5. `apps/kilogram-windows/src/fresh_onboarding/helper.rs` — bounded secret consumer и controlled producer boundary checks.
6. `apps/kilogram-windows/src/fresh_onboarding/tests.rs` — actual bootstrap/model/view tests.
7. `apps/kilogram-windows/Cargo.toml` — existing protocol/getrandom/tempfile и Tokio process feature.
8. `Cargo.lock` — только две direct entries для windows: getrandom/protocol, без version changes.

Candidate также включает принятые contract/review, этот result, recipe и companion pins. Root task/navigation/paused BG и `.tmp` в публикационный состав не входят. Accepted CS1/CS2 implementation, crypto/state/transport/runtime, three paused BG и чужая navigation не менялись. No commit/push до exact review.

## Проверки и actual evidence

`.tmp/clean-start-cs3-1/actual-tool-returns.json`, `result-facts.json`, named `*.receipt.json`/stdout/stderr и `ui-observations.json` содержат прямые результаты, а не исторический граф. Cargo compile-only → existing stable runner, process-local target `target/s54-diagnostics-source-checks`, NativeToken/BelowNormal/Jobs2/offline, fixed EXE paths. Development build/test owners900/930s; UI owners600/630s. Product helper60s+cleanup5s не менялся ради pass.

Final strict Clippy affected package/all-targets `--locked --offline -- -D warnings`: **check04 child/owner0**,4.2412949s, verified nonforced cleanup/EOF. Scoped rustfmt/whitespace прошли (`ea43ca`). Финальная UI build03 и stable compile-only завершились child0; owner1 из-за owned residual cleanup — сохранено ниже, не объявлено clean build owner0.

Final **test03 child/owner0**,13.545248s, nonforced cleanup before=[]/after=[], bounded EOF и actual handle/Job/wait verified. **5 passed:4 substantive tests +1 synthetic producer entry**, не пять независимых пользовательских сценариев:

- Actual fixed-path CS2 Alice/Bob, common ordered contexts, real public credential exchange, wrong recipient/invitation/order refusals, frozen peer, no overwrite, public-only persistence/export, ready reopen с/без ack.
- Invalid/duplicate/unknown/oversize inputs и future kinds; invalid public identity/contexts до создания; existing pending/unknown paths сохранены; tampered progress refusal.
- Actual asynchronous panel/helper, cancellation/stale result drop, no ordinary notice/Debug/progress leakage, настоящие **egui text shapes** скрывают слова по умолчанию, показывают реальные слова после reveal и не содержат их после ack/clear. Reveal frame не печатался/не сохранялся как изображение.
- Controlled fixed-harness producers действительно превышают stdout и stderr caps, одновременно наполняют оба pipes, задерживаются для timeout/cancel и возвращают nonzero со private-marker stderr. Проверяется нужная причина отказа и отсутствие private text/cleanup error, а не только константа cap. Production60s не уменьшен; короткий watchdog передаётся только unit test в тот же collector.

Compatibility **compat01 child/owner0**,2.3642931s: прежний desktop explicit-path/unknown-option parser test прошёл через stable runner. Старый legacy `create` не перезапускался/не ремонтировался; его раскрытый CS2 failure остаётся отдельным известным вопросом.

Fixed final UI `target/clean-start-runtime/kilogram-windows.exe` SHA256 **`509a7870f0e27032819ad567eb72cc3d4f21019d38b1bd0f0744bf8f4963b0cd`**. Bootstrap unchanged `baa3de3dec6b4fa53421c1dd3c54f429a44425240423c9bbbed521e9d3abecb4`; accepted CLI unchanged `c06225fede6dd92e01a7284ca7fdf91c2cc4d328927f8144f4b1d0009fa68a1a`.

## Настоящий Windows UI

Computer Use `@oai/sky`, конкретный returned Kilogram window, не PowerShell UI Automation. Все accounts синтетические; пользователь дополнительно подтвердил отсутствие секретности тестовых слов. Несмотря на это, recovery words не выводились в tool text, не снимались/не экспортировались в screenshot files. Visual checks выполнялись на masked/cleared states; reveal rendering отдельно проверен actual egui shapes.

- UI02: Alice Create → actual CS2 → masked recovery screen. Capture approval/window recovery заняли время,600s owner истёк до acknowledgement; forced owned cleanup и этот failure сохранены. Это также реальная остановка UI с ready-but-unacknowledged account.
- UI03: тот же account reopened → честное «слов больше нет / сохранение не подтверждено» → explicit synthetic ack → cleared view → public invite export. Alt+F4, child/owner0,68.007481s, nonforced cleanup.
- UI04: Bob импортировал/проверил именно Alice invite, подтвердил peer, Join → actual bootstrap → masked words/ack/clear → public reply. Alt+F4,0/0,115.5888889s.
- UI05: Bob acknowledged-awaiting reopen на обновлённом UI, выбранный Alice тот же, words отсутствуют, messaging disabled.0/0,27.548817s.
- UI06: Alice awaiting reopen → inspect Bob reply/recipient → compare/confirm → durable peer selected.0/0,76.710563s. Визуально найден испорченный Unicode separator; исправлен на ASCII readable heading.
- UI07: exact final EXE вновь открыл Alice с тем же подтверждённым Bob; **Peer selected: awaiting mutual consent**, connected не заявлен, кнопка недоступна. Alt+F4,0/0,56.1551881s, nonforced cleanup. Наличие и exact public bindings обоих final UI records/exports независимо сверены в result-facts generation.

В начале UI automation capture был отклонён из-за возможных words; после read-only доказательства отсутствия setup выяснилось, что occluded capture показывает другое окно. Дальнейший ввод выполнялся только после fresh exact-app selection/activation. Посторонние приложения не управлялись; блокировки не обходились. Потребовались обычные delegated retries, отдельного architect callback не было.

## Сохранённые неуспехи и границы

- check01 Clippy child101/owner1: unwrap/panic warnings, исправлены без lint suppression; original scoped sources/вывод сохранены. Subsequent strict checks0/0. При cleanup terminated owned residual PID34856.
- build01/02/03: compiler + stable compile-only child0, owner1 из-за forced owned residual cleanup (PIDs32596/48076/42032), after=[] verified. Individual executable identities не захвачены, не называю их компиляторными helpers как доказанный факт. Final cached tests/Clippy/UI runs clean/nonforced, final native inventory0 exact-image survivors.
- ui01: административный PowerShell variable collision с read-only HOME, child/owner1 до app launch. Исправлено на task-specific variable; исходный failure не перезаписан.
- ui02:600s UI owner work expiry, child124/owner1, forced cleanup own job after=[]; не продуктовый helper timeout. Последующий reopen проверял тот же account; deadline не расширялся.

Ни один failed attempt не выдан за pass; `all_invocations_passed=false`. Original/private historical stores, migration/F/WholeP/endurance не открывались/не запускались. Final native inventory `f84cca`: successful enumeration пяти exact UI/helper/CLI/harness paths, survivors0. HEAD/index и preserved raw pins проверены до сдачи.

Remote communication, account-first arbitrary contacts и default-on volunteer role остаются открытыми требованиями; UI их прямо называет недоступными. CS3.1 не реализует durable message request schema, signed consent/Profile2/OD2 IPC, историю сообщений или full recovery UX. Готово к независимому review только в принятом invite/join/awaiting scope.
