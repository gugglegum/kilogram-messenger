# C2-F attempt-02 source review

2026-10-31. SOURCE ONLY, pending root review. The [accepted isolated regression](oracle-cardinality-r75-accepted.md) establishes the three-byte array correction; its92 checks were not repeated. No scenario, helper/compiler/Core/Rust code, fixture/private/network access or allocation was run/created. Old r73 remains failed.

| File | Bytes | SHA256 |
| --- | ---: | --- |
| [old Core snapshot](C-s53-cold-core-named-streams-01-attempt-01.core-source.ps1) | 74407 | c19606410e62f52bdf9383674d3e944a74ad5558264e9de7ca8b025e20dc4012 |
| [shared Core](../../../scripts/migration-s53-cold-preservation.ps1) | 74407 | 9f52540e35baf01fa69caab1d195069e796e95dbbd074d1120a4e196c8bb19fa |
| [new worker](C-s53-cold-core-named-streams-01-attempt-02.preflight.ps1) | 110837 | 99b3d342af5406e5ae8ef25c823937022207284cc1cefa14bdfa4ebb9ea63a20 |
| [new supervisor](C-s53-cold-core-named-streams-01-attempt-02.watchdog.ps1) | 35546 | 9c1f7867e14e3c317e1518e351e7f52eeb948f711a33e41e7b58a73b34d87a9a |

Note pin returned separately.

Exact delta: shared Core changes only the two named-stream root/negativeAttempt literals at150-151 from attempt-01 to attempt-02. Snapshot is byte-exact to the old74407/c1960641... source and is never executed. Controllers change the closed family token13(worker)/2(supervisor) times, Core SHA once each, each of two inventory SHA literals once, and ONE Assert-CoreSnapshot assignment each: outer @(if...), +3 ASCII bytes. No other logic changes.

Inventory raw bytes remain3578/3598 including LF; new SHA256:
- file: 3856b3e0e2c0f66ef470510782408516b9269decdc74aaafc2ac559548fa256d
- directory: cc91d2fc957908488adf2f7fb401818219bfdd196214f426ea91bd6a5dc8ecd1

Only root strings change in the raw controls. Row data/default payloads, historical tuple H54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5, all numerical expectations/resources,38-field assertions, semantic refusals and both cases are unchanged. Existing native lifecycle/containment, work20/whole30 and compact report caps remain unchanged.

Inverse checked against actual disk bytes: reverse the listed literal/hash substitutions and remove only the outer-array addition. Worker restores110834/SHA2562c375e0092ba96784ae72ccaa847e30fe851f3c8c8f26f26108812d7384aac00; supervisor restores35543/SHA2562f4f3fe968d20ef3bd332c853b8c88d8f3b2e673b94f0ba601ce4b3b8a5b34f4. Reverse the two Core tokens: restores74407/c19606410e62f52bdf9383674d3e944a74ad5558264e9de7ca8b025e20dc4012. Encoding/endings/trailing bytes preserved.

Both new checker functions exactly match the accepted2223-byte/a57bf1b7465ea8a25616b0d36d5331743d5f433aac8bc5cc170db018906509d8 candidate. AST errors0: Core/snapshot3 functions/3 statements; worker50/111; supervisor28/69. No runtime pass inferred. Scope is four new public files plus this Core edit; no navigation/helper/product edits or commit/push/S5.4. STOP for focused source review before any separate runtime.
