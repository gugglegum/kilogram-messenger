# CS3.2a: независимое взаимное согласие из desktop — review result

2026-11-10. Baseline `b0026aeaac76bc39859e36531417f53d7105a70f`. Реализован [принятый контракт CS3.2a](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-2a-consent-task.md): proposal/cosign/final exchange, два offline imports и honest reopen в существующем Windows pilot. **Готово к review, commit/push нет.** [Пользовательский и воспроизводимый recipe](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-2a-consent-recipe.md).

## Результат и границы

Пользователи продолжают уже созданные CS3.1 accounts с сохранённым recovery acknowledgement и подтверждённым первым peer. Canonical role0 определяется по настоящим device ID bytes независимо от inviter/joiner. Он явно создаёт proposal для двух immutable contexts; role1 проверяет оба proposal и явно подписывает каждый только своим Root. Публичные finals возвращаются role0, оба пользователя независимо импортируют их локально. Честный итог: **Mutual consent installed locally; connection is the next step**. Peer import и сетевая сессия этим не доказываются.

Invite/reply сохранили прежнюю форму. Отдельные strict proposal/final envelopes version1 содержат exact invitation/context/sender/recipient bindings и ровно два ordered opaque CS1 blobs. Нет remote filenames/commands, Root paths/keys, local claim или IPC tokens в обмене. Envelope512KiB, каждый decoded blob96KiB; field/aggregate bounds с hex и credential/floor overhead проверяются до decoding. Unknown/duplicate fields, trailing data, future kinds, swapped/missing contexts или неправильный peer/recipient отвергаются.

Actual CS1 body/domain/membership/Root signatures и final wire bytes не изменены. Небольшой read-only [shared validator](E:/AI-Projects/kilogram-p2p-messenger/crates/kilogram-protocol/src/ordered_dialog_v2/fresh_proposal.rs) выделен из прежнего `fresh_consent::cosign`: проверяет tag/exact credentials, Fresh body/suite/permission/time, membership, первую Root signature и administrative seal. CLI вызывает его **до** открытия своего Root; собственные account/device/role/floor/current-Root проверки CLI остаются на месте. UI использует этот же helper для двух proposals до выбора/подписи, без копии signature logic в desktop.

Finals дополнительно проверяются existing `ParsedConsent2::parse/verify/interval`, RootVerifiedCredential и membership verifier: exact frozen credentials/floors, canonical roles, context, Fresh origin, interval. Membership/body/первая signature должны совпасть с exact retained proposal. Оба final проверяются **до первого импорта**. Fresh policy: TTL1..30дней, not-before допускает прежний120s skew, `now < expires` строго; успешный старый import не заменяет эту проверку. Нет automatic renewal/credential refresh/floor downgrade.

## Durable execution и reopen

- `setup.json` получает optional bounded consent state; отсутствие поля у CS3.1 означает not-started. Новый аккаунт не создаётся. `consent/inputs.json` сохраняет точные public inputs/not-before/TTL до первой подписи; credentials/floors копируются только в локально назначенные публичные файлы вне Root/device state.
- По каждому слоту отдельно сохраняются Started/Accepted/Failed, exact output references/digests и import outcomes. Signing retry использует прежние inputs/time и новое собственное `attempt-<random>-<slot>.bin`. Неудачные/пустые и ambiguous outputs остаются. Существование файла не является success marker. Успешные slot outputs повторно не подписываются другим временем.
- Selected proposal/final bytes замораживаются, conflicting later input не заменяет active attempt. Outputs/refs проверяются на reopen, включая подписи частично сохранённого final. State file lock и прежние atomic/no-clobber IO patterns сохранены; двуслотовая установка **не** объявляется одной атомарной транзакцией или power-loss proof.
- Imported flags на диске — прежние acknowledgements. После reopen in-memory `consent_confirmed=false`; UI честно просит явный **Install or recheck both local permissions**. Повтор импортирует те же finals через существующий idempotent command и не заменяет другой stored consent. Only two current successful CLI imports дают текущий local-install success label. Expired data сохраняется, но действия блокируются и permission не называется текущим.
- Directory lock refusal — recoverable conflict: остановить использующий state runtime и повторить exact imports, без reset/kill чужих процессов. Partial/ambiguous completion сохраняется, explicit Resume восстанавливает доказанный результат.
- Isolated worker владеет Setup/lock и short CLI commands; stale/cancelled result не меняет новый selection. Переиспользован CS3.1 collector: concurrent stdout/stderr draining,64KiB каждый,60s watchdog, cancellation и owned wait/reap до5s, sanitized errors. Root secrets не попадают в progress/UI. Старые bootstrap protection/tests сохранены.
- Rendering не читает/проверяет все public files20 раз в секунду: refresh не чаще1Hz и invalidation после действий; каждое действие независимо проверяет всё заново. Expiry проверяется на каждом кадре даже между refresh, без продления permission. Это UI projection, не authority cache для signing/import.

## Изменённые области

**13 source/config paths CS3.2a:** семь fresh_onboarding modules (`artifact`, `helper`, `model`, `mod`, новые `consent`, `consent_artifact`, `consent_tests`), narrow Windows `lib.rs` CLI-path plumbing, Windows Cargo/dev-only state dependency, Cargo.lock, CLI `runtime2/fresh_consent.rs`, protocol `ordered_dialog_v2.rs` и новый `fresh_proposal.rs`. Cargo.lock — одна direct dev dependency на существующий `kilogram-state`, без version changes. Runtime/store/transport/crypto implementation и CS2 factory не менялись.

**Отдельное прямое уточнение пользователя во время работы:** компьютер сильно тормозил при компиляции; будущие builds запускать с меньшим приоритетом, текущий не прерывать. Уже выполнявшийся build01 завершился сам (compiler child0). После этого default в `scripts/cargo-resource-policy.ps1` изменён на **Idle / один Cargo worker**, explicit settings сохранены; если Idle нельзя выставить, compile не продолжает молча с Normal. `ai-docs/verification-workflow.md` фиксирует решение. Системные настройки не менялись. Эти два дополнительных paths входят в candidate на основании прямого пользовательского поручения, отдельно от CS3.2a product scope.

Native read-only observation `ff9188`/`build02-priority-observed.json` подтвердило **Idle у реальных cargo.exe и rustc.exe**, не только текст конфигурации. Новые checks/builds/runs использовали Jobs1/Idle; старые receipts не переклассифицированы. Настройка снижает приоритет и parallelism, но отсутствие любых UI stalls на машине пользователя не объявляется доказанным.

Ни Profile2/create/start/IPC/locator/Active2, ни messaging/durable request schema CS3.3 здесь не реализованы. First-dialog/one-peer, remote/volunteer и full recovery gaps остаются. Известный legacy create failure не перезапускался и не ремонтировался. Three paused BG и unrelated navigation сохранены.

## Проверки: actual результаты

Прямые evidence — `.tmp/clean-start-cs3-2a/actual-tool-returns.json`, `result-facts.json`, named receipts/stdout/stderr, `ui-observations.json`, `final-inventory.json`. Cargo harnesses compile-only, execution только existing stable runner/fixed EXEs. Build/check/test owners900/930s, UI600/630s; per-command product60s/cleanup5s не расширялись ради pass. Failed attempts сохранены; **all_invocations_passed=false**.

- **check06 strict Clippy:** windows/CLI/protocol all-targets, locked/offline `-D warnings`, Jobs1/Idle; child/owner0,7.7692843s, verified EOF/wait/Job и nonforced empty cleanup. Scoped rustfmt, PowerShell AST и whitespace passed (`88c261`).
- **test02:** exact final sources/images, stable package set windows+CLI, child/owner0,258.7028403s, nonforced before=[]/after=[], bounded EOF/wait verified. **9 tests passed:7 substantive test functions + synthetic producer entry + UI-preparation no-op entry.** Actual UI fixture был создан отдельной настоящей preparation invocation внутри test01, а не этим no-op.
- Genuine inviter=role0 и inviter=role1 получены на fresh synthetic accounts; final run logs: fixture attempts1/3. Каждый signer работал при отсутствии peer Root по ожидаемому пути. Actual CS1 propose/cosign/import оба slots; actual RuntimeRepository installed0/1; byte-identical retry не увеличил repository generation.
- Reopen selected proposal, retained successful slot после второго uncertain signing result, реальный no-clobber output conflict, пустой зарезервированный output после crypto refusal; actual directory-lock conflict; actual partial import (slot0 есть,slot1 нет), потерянный acknowledgement/Started на reopen и exact retry только недостающего commit. Это controlled fault hooks, не power-cut test.
- Wrong sender/recipient/invitation/context/order/floor, malformed/duplicate/unknown/trailing/oversize, bad Fresh body/origin/Root/seal signature; negatives до selection/signing не изменили прежний progress. Оба finals проверены до первого import; exact expiry/future outside Fresh skew отказали без удаления installed state. Worst-case two96KiB blobs+hex+sender overhead проверены под512KiB.
- CS3.1 regression: real direct bootstrap/invite/join/reopen/ack/frozen peer и original collector stdout/stderr flood, simultaneous drain, timeout/cancel/sanitized failure; actual egui reveal/clear shapes проверены без screenshot/export words.
- **compat01:** два прежних CS1 Fresh verifier/authority/membership tests после shared extraction passed, child/owner0,13.4175973s, nonforced cleanup. Это узкая проверка затронутой границы, не повтор historical crypto/endurance campaign.

Final fixed images:

| EXE | SHA256 |
|---|---|
| `target/clean-start-runtime/kilogram-windows.exe` | `1a0e3e6a1c137564fcd36792784512f181297a007f88e09db82997729102d496` |
| `target/clean-start-runtime/kilogram-cli.exe` | `4744de03d535d6d2179dfc69b284ed1c1c217fbeb65313191ae6295e5a87b2e2` |
| unchanged CS2 bootstrap | `baa3de3dec6b4fa53421c1dd3c54f429a44425240423c9bbbed521e9d3abecb4` |

Предыдущие accepted UI/CLI images сохранены в ignored `before-images` с исходными raw hashes; backup paths не исполнялись. Final successful native inventory `3f0ea1`:9 exact UI/helper/CLI/stable harness paths, **0 survivors**. Migration harness был только filtered startup с0 matching tests; migration/F/WholeP не выполнялись.

## Настоящий UI walkthrough

Computer Use `@oai/sky`, exact returned/activated Kilogram window. Fresh synthetic CS3.1 fixture уже имела acknowledgement/peer selection, без ceremony field; words не показывались/не фотографировались/не экспортировались. Inviter оказался role1, joiner — role0; ручной смены identities/roles нет.

| Run | Реально наблюдённый результат | child/owner, seconds |
|---|---|---|
| UI01 | Role0 joiner создал два proposal, увидел fixed30-day/remaining lifetime, exported public proposal; closed |0/0,80.1428382|
| UI02 | Role1 inviter inspected/froze proposal, explicitly cosigned both, exported final, imported locally; success says connection next, network/message disabled |0/0,274.6108338|
| UI03 | Signer reopen показал только прошлые import acknowledgements, не свежую DB proof; explicit identical recheck дал local success |0/0,142.8049569|
| UI04 | Proposer reopen сохранил исходные proposal/time; проверил returned final и установил оба contexts локально |0/0,134.5652168|
| UI05 | Proposer снова reopened, explicit recheck тех же final, honest local-only success; closed |0/0,168.0686508|

Все закрытия Alt+F4, receipts clean/nonforced/owned_after=[]; helper errors/words в UI stdout/stderr отсутствуют. Direct public-record/inputs/final pins сверены: общий invitation/ordered pair/not-before/TTL, Accepted по обоим imports у обоих, byte-identical retained final envelope. Не создавались `ui-unused.launch.json` или IPC descriptor; runtime/network session не запускалась. Generated private stores не читались административным отчётом; tests читали только свои fresh repositories для фактической проверки installed/generation.

## Неудачные attempts — сохранены

- check01 Clippy command child0, owner1 forced owned residual PID46144. check02 и check05 child101/owner1 из-за collapsible-if в test/view; исправлены без lint suppression, исходные участки/вывод сохранены. Subsequent final strict checks clean0/0.
- build01/02/03/04 compiler+stable compile-only child0, owner1 из-за forced owned residual cleanup: PIDs48156/48808/5472/49320; after=[] verified. Их individual executable identities не захвачены, не называю их доказанными compiler helpers. Первоначальный --bins build позже сужен до двух нужных binaries. Новые compile/run package sets согласованы для уменьшения лишних feature-unification rebuilds.
- test01 все4 выбранных tests passed, включая реальную подготовку UI fixture, но owner1 после forced residual47016. Final cached test02 clean0/0. Ни одного work timeout в этом этапе не было; потеря результата внутри fault tests не выдаётся за отдельный successful runtime run.

Exact candidate передаётся на независимый review: CS3.2a + прямое пользовательское изменение compilation policy + фактический task contract/result/recipe/pins. No commit/push и никакого перехода к CS3.2b/3 до дальнейшей приёмки.
