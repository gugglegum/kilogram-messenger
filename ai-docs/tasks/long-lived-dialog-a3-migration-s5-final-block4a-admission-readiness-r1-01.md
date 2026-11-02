# Block4A — R1 repairs ready for review

Baseline `97a5bfb9360a9c3a95dbb4b9f3d84e9b5540f54c`. Implements the two repairs dispatched by root review R1; publication is still gated. R0 source/evidence/manifest and migration harness are preserved, with the nine exact R0 source byte copies included as .txt evidence. Existing readiness/evidence remain unchanged. No commit/push, product change, dependency or unsafe code.

PowerShell now enforces an exact distinct artifact role/path bijection and expected-manifest/guard binding before acceptance. Adjacent checks match Rust for line-tables-only runtime debuginfo, unique package IDs, valid unique dep_kind tuples/features, executable target/profile containment and byte count, compiler target/output/source facts and tool basename. No shared historical helper semantics were weakened. Six copies of the first artifact and invented guard with null expected hash are both refused through the actual PS compiler validator and full admission with restamped public references.

During adjacent type review, the intermediate PS02 suite passed67 checks, but a separate read-only production compiler probe actually accepted singleton-array debuginfo. The probe, intermediate sources/image and receipts are preserved. Final policy requires an actual string equal to line-tables-only and applies compiler string limits in UTF8 bytes as Rust does. Identical array-debuginfo and multibyte-oversize mutations are now in both suites; this prompted the final format/Clippy/compile/test cycle.

| Public mutation | Rust refusal | PowerShell |
|---|---|---|
| duplicate_artifacts | duplicate compiler image path | direct + bound refusal |
| invented_guard | no invented frozen guard | direct + bound refusal |
| wrong_expected_hash | frozen expected-manifest identity | direct + bound refusal |
| missing_guard | frozen expected-manifest identity | direct + bound refusal |
| normal_guard | frozen expected-manifest identity | direct + bound refusal |
| runtime_debuginfo | runtime opt3/debug/overflow profile | direct + bound refusal |
| runtime_debuginfo_array | runtime opt3/debug/overflow profile | direct + bound refusal |
| oversized_compiler_string | bounded unique compiler string | direct + bound refusal |
| duplicate_package_id | unique package ID/name map | direct + bound refusal |
| invalid_dep_kind | bounded dependency kinds | direct + bound refusal |
| duplicate_dep_kind | bounded dependency kinds | direct + bound refusal |
| escaped_executable | compiler executable target/profile path | direct + bound refusal |
| duplicate_features | bounded unique compiler string | direct + bound refusal |
| empty_target_kind | bounded compiler target/output facts | direct + bound refusal |
| duplicate_output | unique compiler output paths | direct + bound refusal |
| unadmitted_workspace_source | workspace compiler target belongs to admitted live sources | direct + bound refusal |
| tool_basename | compiler executable role, not arbitrary data | direct + bound refusal |
| duplicate_graph_features | bounded unique compiler string | direct + bound refusal |

Each case first validates its baseline; negative tests check the named boundary reason, not just any failure. PS exercises all18 directly and through full bound admission, so an expired activation or stale outer pin cannot masquerade as compiler rejection.

Source membership now includes the exact capture fixture .ps1, memory fixture .ps1 and public historical M1 candidate JSON. The first two are added by reviewed exact roles inside apps, and the JSON by its one exact path; ai-docs/history is never recursively traversed. Required `Compiler.embedded` carries before/after pins and six selected test-image dep-info pins, supplementing the unmodified shared capture format. Runtime inspection checks both snapshots against the same current admitted source bytes, checks exact derived .d roles, reads pinned rustc dependency rules, rejects every unadmitted dependency and requires coverage of all3 embedded roles. Compiler include concat ../ spellings are normalized lexically before the ordinary nonreparse policy. Normal CLI provenance remains its separate role; this affected test build checks the actual six selected test outputs and does not require or claim a new normal CLI build.

Before each affected actual build, an immutable embedded-before receipt was written. All3 before/after hashes match, including across the frozen-run build. All six actual generated dep-info byte copies are preserved for review; their dependency sets contain exactly those3 non-code embedded inputs and no unadmitted input. Shared compiler captures retain 610 original-shaped source rows; supplemental live set has 505 pins. The historical files were read as public compiler inputs, never modified or executed. Public tests reject each missing/changed embedded file, missing before set, changed after pin, duplicate .d role, unadmitted dependency and incomplete embedded coverage.

Final actual checks: fmt-06 (Rust fmt/check plus PS syntax), strict Clippy04, compile-only02, exact stable pure03 **11/11 PASS**, PowerShell03 **71/71** (1valid+70 expected refusals). Six aggregate invocations selected0; no A1–B2 or historical replay. Windows stable runner, Jobs2/BelowNormal/offline/locked; bounded230/240s format/pure,590/600s Clippy/PS,1190/1200s compile. All final actual child/outer exits0, work_expired=false, cleanup_verified=true and no remaining members. The compile wrapper contained a remaining compiler helper; this is build containment, not whole-runtime success. Earlier strict Clippy01 rejected4 expect calls in the new test helper; failed source/log/receipt are retained, and errors now propagate without relaxing lint policy. PowerShell01 reached its230s work cutoff without a final result; owned cleanup verified. Original wrapper/launcher and raw public fixture state retained. Pure01 had passed11/11; pure02 repeated the same selection to produce a fresh public fixture for the justified590/600s PowerShell02 retry. No test/policy/source was weakened to fit the bound.

Stable migration harness: `E:\AI-Projects\kilogram-p2p-messenger\target\verification-scale\deps\kilogram-cli-test-migration_s5_process-tests.exe`, 19922432B/SHA256 `8fae5117c98371c55f2061c20ef8c3164fcf1ae912e145cd7329a46e78ca4a40`. Other five stable images match preimages. Current9 source pins, raw compiler pins, actual image pins, mutation results, original R0 pins and final receipts are in `review.json`. Candidate allowlist/raw hashes/normalized Git blobs are in `.tmp/s54-block4a-r1-01/handoff-manifest.json`.

Public synthetic fixtures cannot activate a real P attempt or establish whole success. Safe original clock, whole selector, frozen-only runtime launch and Job/result authority remain Slice B. No whole/feasibility, production inventory/freeze/A/L, normal CLI staging/build, private/profile/key/DB/network/messenger/relay operation was performed. Navigation3/index/baseline preserved. Stop after substantive callback for review.

Navigation comparison is explicit: README/next-stage match the R0 pins; chat-coordination has a pre-existing root dispatch update (last write14:19, before R1 directory14:21), raw165801B/SHA256 `bb02d660b905ffe74d3c5f51bd57bc048437261aef10d941470f79938e17f93b`. Its current bytes are preserved and excluded from the candidate, with observed metadata in navigation-observed-final.json. The stale R0 navigation snapshot was not restored.
