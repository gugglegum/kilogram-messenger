# CS1: независимая Fresh Consent2 реализована

2026-11-10. На baseline `f433f1e078474b590cac0ee43ef306a8d7115f5c` добавлены `ordered context2-propose` и `ordered context2-cosign`. Каждый получает ровно один собственный Root directory. [Рабочий CLI-рецепт](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs1-cli-recipe.md) описывает context IDs до enrollment, public exchange, обе роли, import и local messaging.

Canonical device role0 создаёт/проверяет прежний membership, role1 cosigns после проверки первой подписи. Оба требуют явные expected local/peer account/device/context и отдельно сохранённые signed authority floors. Credential pair сортируется по device ID; local credential authority должна совпасть с current Root, peer authority — не быть ниже retained floor. Existing-origin путь не изменён.

Новый **административный** proposal — bounded canonical CBOR с явным Fresh tag, обеими credentials, membership, прежним Consent2 body/signature и tagged witness signature над точными public bytes через существующий Root Consent2 signing primitive. Witness имеет отдельные tag/arity и не может стать Consent2 body. Cosigner сверяет proposal с независимо поданными public inputs/pins, проверяет membership, validity, первую подпись и witness **до** загрузки своего Root/подписи. Новых crypto primitives/domains нет. Final Consent2/context wire-format, verifier, runtime import, DB/runtime/defaults/UI не изменены. Proposal не является final artifact.

Output create_new резервируется перед authority mutation; conflicts не перезаписываются. Ошибка может сохранить пустой/неполный public evidence output: retry с новым именем. Membership другой пары не заменяется; точный повтор с теми же public bytes/time/TTL безопасен и byte-identical. Private Root/keys не попадают в proposal, final context или logs.

## Точный implementation diff: семь файлов

- [ordered2_cli.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/src/ordered2_cli.rs:42): аргументы/dispatch/вывод no-clobber.
- [runtime2/admin.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/src/ordered_dialog/runtime2/admin.rs:10): экспорт узкого offline helper.
- [fresh_consent.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/src/ordered_dialog/runtime2/fresh_consent.rs:1): proposal, verification, single-Root cosign.
- [fresh_consent_tests.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/src/ordered_dialog/runtime2/fresh_consent_tests.rs:60): final verifier interoperability, 12 tamper variants, malformed/incomplete/oversize, wrong pins/role/Root, authority rollback/currentness/revocation, exact-byte seal, membership conflict/idempotence.
- [runtime2_process.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/runtime2_process.rs:397): reuse existing Pair preparation with distributed ceremony; CS1 opt-in selects fixed test CLI path. Other tests retain their path/ceremony.
- [runtime2_process/fresh_consent.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/runtime2_process/fresh_consent.rs:1): actual CLI propose/cosign/import; peer Root's actual directory is unavailable while each signer runs; reverse public credential order; no-clobber before first membership; incomplete import refused; actual two-process delivery/history.
- [positive.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/runtime2_process/positive.rs:133): existing exact-history verifier exposed only to its sibling test module.

## Фактические проверки

Compile-only Cargo followed by **existing** `scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter cs1_ -Profile verification -CargoJobs 2 -Run -NoCapture -SkipCacheStatus`; process-local target `target/s54-diagnostics-source-checks`, NativeToken/BelowNormal/offline, fixed CLI `target/clean-start-runtime/kilogram-cli.exe`. CLI SHA256 `c06225fede6dd92e01a7284ca7fdf91c2cc4d328927f8144f4b1d0009fa68a1a`. No direct network `cargo test`, migration F/WholeP, new runner or production image staging.

|Step|Child / supervisor|Elapsed s|Result|
|---|---|---|---|
|check01|0 / 0|33.4692763|strict Clippy all-targets, first source version|
|build01|0 / **1**|38.7854809|CLI built; forced owned residual PID20172, after=[]|
|test01|0 / **1**|110.1624521|2 unit + 1 runtime tests passed; forced owned residual PID42688, after=[]|
|check02|0 / 0|10.4103790|final strict Clippy `--all-targets -- -D warnings`|
|test02|0 / 0|27.3389127|final 3 tests passed, nonforced cleanup before=[]/after=[]|

First build/run supervisors are preserved as failures, not hidden or reclassified. Residual individual executable identities were not captured; compiler-helper lifetime was the **hypothesis**, not an established identity. The justified cached rerun kept sources and deadlines unchanged, reused compiled artifacts and created fresh synthetic profiles. It cleanly passed. All five owners verified actual handles/Job membership, waits, bounded stdout/stderr EOF, no reader errors and final owned_after=[]. Watchdogs: static/build900/930s; combined test600/630s; inner smoke150s. No acceptance deadline was raised.

Final test02: both unit tests passed (5.19s total); actual CLI smoke passed (15.45s). In two contexts, both canonical-role commands succeeded without the peer Root's actual path; both repositories imported the unchanged final artifact; both Runtime6 processes reached Active2; four messages delivered bidirectionally with exact request IDs/UTF8 text, matching ciphertext Event IDs/history, no duplicates. Both runtime processes exited0 gracefully. This proves this short new-to-new slice, not the full offline/restart/rotation MVP matrix.

Scoped rustfmt check and diff whitespace check passed. Evidence/receipts retained in `.tmp/clean-start-cs1`; both fresh process fixtures remain under `.tmp/a3-s4/process/CS1-*` (generated data only; no private bytes exported/pinned). Final exact-image enumeration and source/evidence pins are in the companion manifest/admin result. No recursive historical graph or600-file audit.

## Review boundary

Two immutable slots/one peer pair remain; context IDs must be shared before device enrollment. Desktop and default onboarding are unchanged. Offline authority checking establishes the independently retained floor and local current Root, not knowledge of unseen remote revocations. Old co-located operator helper and Existing ceremony remain available unchanged. Root-controlled recovery/UI/default tasks remain separate.

The three paused BG dirty files stayed byte-identical, as did this executor's navigation baseline unless an external navigation change is separately observed. No source cleanup, migration attempt, unknown-format reset, legacy/profile deletion, implementation commit/push or system changes. Accepted clean-start map/pins and root scope document can accompany this result; their contents are preserved. Submit exact candidate for independent review, then stop.
