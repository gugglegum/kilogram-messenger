# Block4M — sole F04 runtime FAILED; retained compiler/unknown advisory

2026-11-03. Task `long-lived-dialog-a3-migration-s5-final-block4m-f-runtime-task-04.md`. На опубликованном HEAD `9013f9cfb2e1803d279eaa0377b2865c835618ec` выполнен ровно один externally activated FeasibilityF Run. Production child и supervisory owner вернули **1**. `outer_checks=failed`, `failure=launcher_nonzero`, `whole_attempt_verdict=not_established`. F04 consumed и не подлежит повторному использованию. Retry, изменение source/state, новая activation, compilation/staging, P, commit/push не выполнялись.

Точная сохранённая advisory-строка:

```text
S54_ADMISSION_DIAGNOSTIC v1 stage=compiler reason=unknown
```

Это static code группы native `compiler::validate`, `authority=none`. Точная отказавшая проверка/underlying error **UNKNOWN**. Причина прежнего F03 по-прежнему UNKNOWN; диагностический результат F04 не восстанавливает утраченные F03 данные.

## Binding and actual invocation

- Case `F-s54-mixed-hooks-feasibility-01`, attempt `F-s54-mixed-hooks-feasibility-01-attempt-04`.
- Run ID `27f67783ab7bbcfd5b306178bc010a11317b213a8aba317d7230915ccccb95d2`.
- Selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`.
- Run `.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-04`; fresh administration `.tmp/s54-block4m-f-runtime-04`.
- Inventory116874B SHA256 `b548cf47be85fdae660bf7ed183438b5810f376bc2c3275b78f38e7ac824b85d`.
- Root activation1003B SHA256 `014e6a7c954e86720278c1ee0a259755a5f2b01edb438ee1907410d3f8a4700a`.
- Root validity `[1793664668341,1793671868341)`, `2026-11-03T00:11:08.341Z`–`2026-11-03T02:11:08.341Z`; receipt `.tmp/s54-block4l-f-preparation-04/root-activation-01.json`, issuance b2bee7. Activation не переписывалась.

Fresh installed executable:
`C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe`.

Actual production argv:

```text
-NoProfile -NonInteractive -File E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s54-whole.ps1 -Action Run -Purpose FeasibilityF -InventorySha256 b548cf47be85fdae660bf7ed183438b5810f376bc2c3275b78f38e7ac824b85d -ActivationSha256 014e6a7c954e86720278c1ee0a259755a5f2b01edb438ee1907410d3f8a4700a
```

Supervising entry: pinned pwsh `-NoProfile -NonInteractive -File .tmp/s54-block4m-f-runtime-04/owner.ps1 -Step run-01`. Actual launch104693/session92029; actual final tool return **c7e843/exit1**. Supervisor PID16396, production outer69736, frozen launcher52344. Native harness PID/known handle binding не установлен: harness=null и native claim отсутствует. Не выводится PID из одного test summary.

## Preflight and original clock

Cheap read-only fresh-child preflight: actual return4e80ac (launch23269c/session17283), exit0, stopwatch9.9373557s,270/300/30; child36696/owner52268. Полный relevant environment exact с inventory, PATH child `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`, pinned shell/resolutions, source519/image9, below-normal/offline, TOKIO_WORKER_THREADS absent/effective24. Original owner PATH `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`; после однократного Rust1.98 prepend parent `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`. Persistent settings не менялись.

Перед Run exact entries были inputs+activation; activation имела достаточное оставшееся окно. Successful native exact-image process inventory0, historical168 pins, F01/F02 empty, F03 eight artifacts и отсутствие P проверены. Full Inventory replay, Cargo build, profiling/matrices до Run не выполнялись. Production сохранил свои собственные fresh admission checks на прежних местах.

Original production T0 из `launch.json`: tick514947515, UTC `2026-11-03T00:14:13.503Z` (`1793664853503`). Actual child start `2026-11-03T00:14:13.0565524Z`.

| Событие | TickCount64 ms | От original T0 |
| --- | ---: | ---: |
| original T0 | 514947515 | 0 |
| controller deadline | 515397515 | 450000ms |
| work deadline | 515427515 | 480000ms |
| outer deadline | 515457515 | 510000ms |
| production checks observed | 515027859 | 80344ms |
| stdout/stderr EOF observed | 515027937 | 80422ms |
| actual child exit observed | 515028000 | 80485ms |

EOF UTC `2026-11-03T00:15:33.9210328+00:00`; exit observation UTC `2026-11-03T00:15:33.9818262+00:00`. Эти отметки записаны в owner polling loop **до** supplementary diagnosis/pin reads/cleanup. Owner stopwatch80.9692771s; supervisory original tick514946328,570/600/30 только для capture/containment. `work_expired=false`; F450/480/510 не расширялись и T0 не сбрасывался. Отказ произошёл задолго до всех исходных cutoff.

## Retained protocol, claims and diagnostic boundary

Run сохранил восемь top-level public artifacts: inputs, activation, started, launch, capture, outer-result, native stdout/stderr protocol. Native-started/clock-request/reply/join-PID/permit-PID/assertions/profile directories отсутствуют. Никакие profile/key contents не читались и не экспортировались.

`outer-result.json`:3904B SHA256 **`55ef06dbb6daf8ff1a01c1944ad946658779e9adacf265f73e5efe527148da44`**. Launcher52344 exited/waited=true/code1; harness=null; outer_checks failed/failure launcher_nonzero. Durable `actual_desktop_return=pending` оставлен неизменным; отдельно наблюдены actual child waited/code1 и фактический final tool return c7e843/1.

| Exact protocol cardinality | Count |
| --- | ---: |
| running 1 test | 1 |
| exact named invocation | 1 |
| named FAILED | 1 |
| aggregate FAILED /0 passed /1 failed | 1 |
| named ok | 0 |
| native functional marker | 0 |
| wrapper completion marker | 0 |
| S54_CAPTURE | 1 |
| retained admission advisory | 1 |

Native aggregate finished12.14s,304 filtered out. Это длительность test invocation, не whole F elapsed. Native stdout325B/stderr206B; capture EOF=true, overflow=false/failed=false. Production bounded persistence complete=true, source streams EOF/stopped, invalid_utf8=false, partial_tail=0, stored_truncated=false, malformed_protocol_lines0. Stderr advisory metadata: count1/bytes58/dropped0/classification advisory_failure_evidence/authority none, line128/count4/stream512 bounds. Successful persistence не означает admission/runtime pass.

Stored stdout214B SHA256 `ed8685567d4f4ffd4f163b8155fceee8932f7d26bf5d0865c12d61c2e9d1d771`; stored stderr281B SHA256 `1cb4190eb85fb94c3e8131ef8581e541efdec1fed0da82a7ff9c46fdd2c05ec8`. Nonprotocol output по-прежнему redacted: stdout8 lines, stderr10 lines. Captured launcher prefixes325B/723B с SHA доступны в outer-result; arbitrary raw native error не сохранялся и не рассекречивался. Raw **production outer** stdout0B/stderr325B сохранены supervisor; stderr содержит общий outer failure, SHA256 `956bcf512b1efaba92d2ad5b434a21c975ebedc907278849546bad7b9ac83f64`.

Read-only source finding: в опубликованном `migration_s54_admission.rs:856` единственный `D::CompilerUnknown` оборачивает `compiler::validate(p,&i)`. Этот check group читает normal/test captures и проверяет literal argv, capture boundaries, shared inputs, units/artifacts, embedded/dependencies и frozen manifest/normalized units. Advisory не различает эти внутренние проверки. По validation order это ранняя native admission до native claim; `load_for` не завершился. Functional mixed/hooks/peer/relay/restart/offline/recovery scenario не запущен, final success-tail admission/revalidation не выполнен. Подготовленный PS inventory ранее прошёл31.378297s; это отдельное свидетельство, не замена отсутствующему native admission. Никакой конкретный compiler invariant/error и причина F03 из этих фактов не утверждаются.

## Cleanup, preservation and stop

Production cleanup verified=true/forced=false/failed=false, before=[]/after=[]; supervisor verified Job membership/child wait/complete EOF, reader_failed=false, cleanup verified=true/forced=false/failed=false, before=[]/after=[]. Чужие процессы не завершались.

Read-only postcheck a98e35/exit0/8.1782232s подтвердил: source519/image9/input pins/current navigation неизменны, HEAD тот же/index пуст, successful native exact-image inventory0. Historical168 pins (F01/F02/F03 admin/public/reports/preimages и H/K source-check images) сохранены; F01/F02 runs empty/F03 eight failure artifacts unchanged/P absent. Полная production inventory/compiler validation не переисполнялась для диагностики и source workaround не делался.

Pin manifest рядом с отчётом ссылается на оригинальные admin/public/source/image/compiler/history artifacts. Большие captures/binaries не копировались. F04 attempt сохранён без state cleanup/rewrite/reuse; дальнейшее исследование требует решения root. После одного callback этап остановлен, без polling/commit/push/новых запусков. Whole S5.4 не принят.
