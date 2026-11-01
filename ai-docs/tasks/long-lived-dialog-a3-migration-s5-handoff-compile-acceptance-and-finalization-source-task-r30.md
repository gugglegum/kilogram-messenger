# S5.3 r30: compile принят; отдельное поручение на исходники metadata finalization

2026-10-29. Root 01a164ec-589c-7b41-bc34-3ba158d1f22b; исполнитель 01a1653c-0154-7611-8e8c-044fbe1e4d7a, GPT-6.1 Sol Max, без Fast. Общий checkout E:/AI-Projects/kilogram-p2p-messenger. HEAD64c2359788ead360314793e47741adacb83b8d4f, index пуст.

## Приёмка завершённой compile-only preparation

[Readiness исполнителя](long-lived-dialog-a3-migration-s5-handoff-compile-readiness-r30.md) принят именно в scope compile artifacts и preservation. Root прочитал весь command, проверил actual app native return, полный stdout и пять receipts; независимо пересчитал raw compiler records, tuples и текущие файлы. Проверено1570 file-row comparisons/1061 unique files:32 artifact pins,839 canonical,68 protected,95 prior-unit,487 compiler source inputs,9 old archives,9 exact pre-compile mappings и2 before-copies. AST parse0, шесть utility declarations exact, конфликтующих Cargo/Rust/Kilogram процессов при финальном наблюдении0.

Native exec-6f195b3e-897a-4f98-aec8-3a14c5766824 exit0/untruncated, app45.468s; script44.9780429s, observed whole45.4704761s; wrapper36.1730617s/Cargo26.78s. Max receive gap10.8571139s<=20. Restore/owned cleanup/terminal/Finish true. Отдельная preparation закрыта success, C/F/P charge/reservation0.

Current controller344341B/68fea9a6b25acfcd4960609271343e858eb81a58fc45d79230bd06cb5a9d5564 сохранён. Actual501 normalized units/386 runtime/524 packages; historical r29 unit identity точно совпадает. Six actual source/stable roles и отдельный normal CLI bin/test=false проверены. Из9 current images изменился только process owner:9997824B/86d813f98e2840c358915c314bff033cbedbba1eb739adfc1e53b2d7a5d5730d. Прежний owner4927fa0d… сохранён в exact r30 pre-static copy; остальные8 images неизменны.

Агрегат provenance.actual_freshness=[{fresh:"",count:501}] непригоден. Root независимо проверил raw bools:500 true/1 false/invalid0, false только migration_s5_process. Это ошибка вспомогательной summary. Command/provenance/capture не переписывать; для r30 использовать реальные raw flags и отдельный readback. Capture frozen_guard_verified=false/expected_manifest_sha256=null честно отражает compile без входного manifest; новый manifest принят после отдельного readback. Static native failure остаётся историческим failure; его successful fmt/Clippy и preservation уже отдельно приняты, повторять их не нужно.

| Принятый вход | Байты | SHA256 |
| --- | ---: | --- |
| [Root independent verification](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-compile-01.root-independent-verification.json) | 16126 | 8f301e9788c480cc4d1390085bf17466e2b60a124a6df3dac5b34d2e70d33ae7 |
| [Closed-success preparation](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-compile-01.root-closed-success.json) | 2000 | 5f6f16d31e65d33e484041acb3d138e668f2a3e64919e49362a5640e5c572f8a |
| [Post-compile source baseline](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-post-compile-baseline-01.json) | 220627 | f1e8272ec9d17b5484c43200f39925087a7b6ed476eea887b89242bb99034f25 |
| [Current compile manifest](long-lived-dialog-a3-migration-s5-handoff-evidence/S5.3-r30-compile-expected-artifacts-01.json) | 1279146 | e61cd91ba482499b90c311e3bb7e3209e84f7930fe3b03dc6c9318c9735869cd |
| [Current raw capture](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-compile-01-capture.json) | 2680568 | 32f6b7da79c6e8bb83f5b11ac7c7bee3b003702f17b978427ca9da94fdf68838 |
| [Inherited r29 binding](long-lived-dialog-a3-migration-s5-handoff-evidence/S5.3-integration-binding-r29.json) | 1377570 | 1f010b4008e7d5409d8cb691130442e7302bb1ac067f0691cbe7ad77426b82f5 |
| [Inherited r29 seal](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-integration-submission-seal-r29.json) | 286536 | 917b5892baa891809d44b284304c9ae9923995f575b828b9be1236411b53c61e |

## Следующий маленький scope: только четыре исходника и сдача

Выполнить шаг5 из [принятого r30 proposal](long-lived-dialog-a3-migration-s5-handoff-committed-before-publication-proposal-r30.md), разделы7–9. Создать scripts/prepare-migration-s53-profile-r30.ps1 и scripts/freeze-migration-s53-integration-r30.ps1 на основе неизменных r29 versions. Изменить scripts/invoke-migration-s53-integration-case.ps1 и scripts/invoke-migration-s53-integration-outer.ps1 только указанным ниже минимальным образом. Это поручение на source-only реализацию и review, не на исполнение producers.

Дополнительно разрешены один новый ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-finalization-source-readiness-r30.md и свежие публичные source-submission/preflight/diff records с prefix S-s53-r30-finalization-source- в evidence directory. Старые файлы и receipts не перезаписывать. Root indexes принадлежат root. Rust controller, production, shared helpers и EXEs не менять. Никакой сборки, Cargo metadata, formatter/Clippy, --list, тестов, CLI, fixtures, runtime, final binding/seal/archive/candidate/admission/activation сейчас не выполнять и не создавать. Commit/push и S5.4 не разрешены этой частичной приёмкой.

Перед правкой сверить все actual baseline fields, разрешённые paths и реальные schemas, затем работать в allowlist. Текущий baseline содержит839 canonical. После двух новых .ps1 ожидается841; из прежних839 меняются только case/outer. По отношению к r29 binding меняются3 прежних canonical: controller/case/outer, и добавляются2 scripts. Остальные837 post-compile canonical неизменны. Записать полные before/after tuples, новые paths отдельно и доказать отсутствие удалений.

## Минимальные adapter changes

Case: заменить один binding filename S5.3-integration-binding-r29.json на r30 и добавить одно имя F-s53-committed-before-publication-01 в explicit verification-scale list. Outer: только тот же binding filename. Сравнение по обратной проекции должно давать точные нынешние bytes.

Receiver Read-KilogramS53InventoryAdmission сохраняется побайтно:12096B/38364f7af1d2d90b9d8520805231b00cd1c5b0990c79c0319360e2783d3cedb0. Metadata helper26930B/d2b65d9b6abdfc6a8d73d0d0ec6749958ed2a80fb8fdd6dc9a23d82a154de70e также exact. Суффиксы r29 в admission classification и serializer — действующие версии schema: s53-full-inventory-admission-r29 и powershell-json-depth30-pretty-utf8-no-bom-no-final-newline-r29 сохраняются. Не делать глобальную замену r29 на r30 и не менять receiver comments/diagnostics.

## Producers: перенос действующей схемы на текущие факты

Новая prepare использует те же шесть exact utility declarations, deadline/resource/restoration/terminal/Finish control. Для будущего отдельного metadata invocation сохранить work100/whole120, jobs1/BelowNormal/one lane/retry0, NativeToken, process-only offline и exact presence/value restore, owned cleanup5. Сначала clock; отсутствие root window или expiry означает fail-closed. Будущие root window/source review не фабриковать и не запускать. Нельзя скрывать ошибки finally/restoration или считать terminal до Finish окончательным PASS.

Определить будущие paths:
- S-s53-r30-finalization-01.root-preparation-window.json в evidence;
- S-s53-r30-finalization-source-review-01.json: будущая root-owned schema_version1/classification s53-r30-finalization-source-only-review, baseline, canonical841, metadata, helper_pins и current artifacts; root создаст после review;
- source readiness с именем, указанным выше;
- final outputs с r30 и fresh attempt F-s53-committed-before-publication-01-attempt-01; runtime allocation остаётся false.

Freeze должен читать реальную post-compile baseline schema из таблицы. Там canonical839, current_images9, protected/prior_unit_files, old_preserved_images, existing_before_copies, prior_current_image_archive_mappings и immutable_pins. Не переносить механически отсутствующие r29n fields. Если нужна будущая root record field, явно описать её контракт в readiness; существующие поля проверить по всем элементам сейчас.

Inherited r29 binding имеет code_source_count839/inputs5761, seal archive844. Новый source count841, будущий source archive846 =841 canonical +3 текущих root indexes + source readiness + pre-freeze readiness. Preserved historical live replacements — controller/case/outer и3 old indexes:6 exact archive mappings из r29 seal. Новые два scripts не имеют old tuple. Сохранить r29 prepare/freeze и остальные старые source files. Полный прежний metadata union, включая failed runs, accepted r29 attempt02 с14 logs/root proof/actual charge/closure, текущие r30 contract/source/static/compile records и настоящие публичные dependencies, сохраняется. Исторические доказательства остаются в своих scopes, не становятся automatic predecessors.

Для current provenance использовать только принятый r30 compile capture/manifest:487 build source inputs,501 units/386 runtime/524 packages, шесть current source/stable tuples, normal CLI test=false, nine current images. Четыре script paths не входят в487 build inputs; доказать полное совпадение input membership и SHA, compiler/shims/config/environment без Cargo/rustup/rustc запуска. Final r30 expected-artifacts manifest — byte-for-byte copy принятого compile manifest e61cd91b…, с исходным capture SHA, без новой компиляции.

R29 reader captures и manifest63e2f05d… остаются историческими. Нельзя требовать от них current controller68fea/current owner86d813, утверждать, что старые reader tests исполнялись на новом owner, либо хешировать перезаписанный compiler output как прежний owner. Исторические tuples проверяются через exact сохранённые archives; current source/stable pairs — из current r30 capture. Freshness считать по raw bool, без Group-Object dictionary-property summary.

При переносе image preservation использовать фактическую r30 preservation schema: current_records9 — именно pre-compile r29 current tuples (owner4927), old_records9, existing_before_copies2, new_copies0. Сопоставить каждую old r29 image с exact current_records archive; old archives и before-copies сохранить также. Новые current_images брать из root post-compile baseline. Нельзя путать old/current archive sets или брать owner160deb из ещё более старого шага.

Сохранять все public path checks. Для .tmp source archives нужны ровно две специальные строки ordinary source migration_s5_l1_exhausted.rs: существующая .tmp/migration-s53-r29-final-source-01/apps/kilogram-cli/tests/support/migration_s5_l1_exhausted.rs и новая .tmp/migration-s53-r30-final-source-01/apps/kilogram-cli/tests/support/migration_s5_l1_exhausted.rs. Это исходники теста, не private data. Проверить все846 planned destination strings и все реально используемые inherited archive paths до producer execution. Absolute child не соединять повторно с workspace. Никогда не читать/перечислять private migration-s5-storage/l1-exhausted-observed-NCjBHb, migration-s5-preserved/F-exhausted-preservation-01, root-gen511 или private exhausted data.

Новый inactive case policy имеет прежние14 fields; меняются case/loss на F-s53-committed-before-publication-01 / s53-Committed-before-publication. Selector live_l1::handoff::s53_control_loss_reopen_public, expected_count1, verification-scale, setup45/first-M15/recovery45/delivery30/controller160/work180/whole200 без сокращений. Production checkpoints и cold oracles уже реализованы/приняты в source; текущая работа их не меняет.

Порядок будущих writes остаётся acyclic: final manifest/normal provenance/pre-freeze readiness → full source archive/index → binding → seal → complete candidate → admission. Candidate/admission/seal не попадают в собственные binding ancestors. Runtime inventory/run при metadata preparation не создаются. Сохранить exact14-field candidate<=2097152 actual UTF-8 bytes, exact16-field receipt<=65536, array counts/digests и sole activation SHA64-zero placeholder. Full candidate измеряется только после binding/seal; прогноз headroom не является доказательством. Никаких pruning/compression/reference substitutions/cap increase, схемных исключений или фиктивных fixtures. Одна future attempt, predecessors0/prerequisites0.

## Premise checks и сдача source review

Сдать полный diff двух новых producers к r29 templates и минимальные case/outer diffs. Проверить parser/AST, exact шесть utilities, exact receiver, immutable shared helpers, новую canonical membership841 и837 неизменных prior canonical (839 минус2 adapters); отдельно9 images/487 compiler inputs/archives/evidence. Все schema checks проходят по полным массивам, включая names/policy differences, nested profiles и обязательные types; не проверять только первый элемент. Не dot-source целые prepare/freeze/case mains для инспекции. Разрешены read-only хеширование/AST/проверки paths и необходимых чистых declarations, без producer writes.

Подготовить S-s53-r30-finalization-source-submission-01.json с canonical841, точными четырьмя deltas, metadata union (path/bytes/SHA каждой фактически прочитанной публичной зависимости), helper pins, current9 images, accepted manifest и реальными результатами AST/schema/path checks. Не включать собственный будущий digest и не фабриковать root review/window. Все late root-owned receipts позднее добавит root через отдельный bounded additional_evidence, без удаления прежних dependencies. Readiness содержит полные diffs либо точные проверенные ссылки, declared future input schemas и честное разделение completed source checks / not executed metadata.

Нельзя выдавать список планируемых source/archive tuples за созданный archive или оценку размера за actual candidate. Если выявится несовместимость, не устранять её расширением scope; привести конкретную причину и минимальное предложение в callback. После сдачи — один callback root с readiness bytes/SHA, четырьмя source tuples и результатами premise checks, затем stop до root source review.

## Учёт и последующие границы

S5.3 actual C1433.7664909/F1441.9749663/P0; combined C2021.2151018/F6158.2951152/P0; ceilings1550/1600/0; remaining C116.2335091/F158.0250337/P0. Никаких активных резервов. Source-only и отдельная принятая compile preparation не списываются в C/F/P.

Будущий F200 не помещается в remaining158.0250337. Минимальный достаточный ceiling_F1641.9749663 (увеличение41.9749663) остаётся только расчётом. Сейчас ceiling1600; metadata accounting/proposed-only должны отражать необходимость будущего решения root, не подразумевать разрешённый runtime. Сначала source review, затем отдельное metadata window, затем полный artifact/admission review и явное budget/allocation решение. 11/12 cuts приняты; полный S5.3, commit/push и S5.4 ещё открыты.
