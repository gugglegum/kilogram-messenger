# Block4J — единственный FeasibilityF F03 завершился неуспешно

2026-11-03. Задание: `long-lived-dialog-a3-migration-s5-final-block4j-f-runtime-task-03.md`. На опубликованном HEAD `e15492a9b9733ba43afc9c14b79bcbb754efb6c0` выполнен ровно один фактический `Run`. Нативный дочерний pwsh и supervising owner вернули **1**. `outer_checks=failed`, `failure=launcher_nonzero`, `whole_attempt_verdict=not_established`. Повтора, новой activation, изменения исходников, сборки, staging, P, commit/push не было. Попытка сохранена и повторно использоваться не должна.

## Запуск и входы

- Case: `F-s54-mixed-hooks-feasibility-01`.
- Attempt: `F-s54-mixed-hooks-feasibility-01-attempt-03`.
- Run ID: `7663dc595e6c4f6d219cd3ee3351741d7e94d687ab27cb5cf86b454d7a03b3be`.
- Selector: `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`.
- Run: `.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-03`.
- Административные доказательства: `.tmp/s54-block4j-f-runtime-03`.
- `inputs.json`:116473 bytes, SHA256 `36ad2dfbdf3e83f31328b62970e127119e688174892426e478cff42a904a012d`.
- Root-issued `activation.json`:1002 bytes, SHA256 `d6ac5fb8810c39e0f18a0e294b0fdf85acb0ff840b569de8a81ac56dbc08bbb6`; окно `[1793659489240,1793666689240)` не менялось.

Фактический executable:
`C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe`.

Фактические аргументы свежего дочернего процесса:

```text
-NoProfile -NonInteractive -File E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s54-whole.ps1 -Action Run -Purpose FeasibilityF -InventorySha256 36ad2dfbdf3e83f31328b62970e127119e688174892426e478cff42a904a012d -ActivationSha256 d6ac5fb8810c39e0f18a0e294b0fdf85acb0ff840b569de8a81ac56dbc08bbb6
```

Внешний owner запускался тем же installed pwsh с `-File .tmp/s54-block4j-f-runtime-03/owner.ps1 -Step run-01`, NativeToken. Фактический tool launch: `bdd7b5`, session32165; фактический окончательный tool return: `25f7ab`, exit1. PID owner10804; production outer64788; launcher62452. PID harness не был установлен через native claim; выдумывать его по одному test summary нельзя.

## Preflight и часы

Свежий read-only дочерний preflight37120, owner58988, вернулся0 за8.1781681s (фактический tool return `08f432`), complete EOF и clean nonforced Job cleanup. Сравнены полные relevant environment rows с inventory, resolution paths и installed shell, BelowNormal, offline=true, TOKIO_WORKER_THREADS отсутствует/effective24; source517/image9, внешний input binding, отсутствие P, точные две исходные run entries, успешная CIM exact-image inventory0, неизменность historical85. Полный admission replay и сборка preflight не выполнялись.

Воспроизведена граница parent→fresh pwsh child. PATH SHA256: исходный owner `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`; parent после однократного prepend Rust1.98 `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`; фактический child `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`. Постоянное окружение не менялось. Preflight stderr121B — существующее предупреждение Git о CRLF в root navigation.

Исходный production T0 взят из фактического `launch.json`, а не из административного owner:

| Событие | Windows TickCount64 ms | От исходного T0 |
| --- | ---: | ---: |
| production T0 | 509990281 | 0 |
| controller cutoff | 510440281 | 450000ms |
| work cutoff | 510470281 | 480000ms |
| outer cutoff | 510500281 | 510000ms |
| production checks observed | 510067890 | 77609ms |
| stdout/stderr EOF observed | 510067968 | 77687ms |
| child exit observed | 510068031 | 77750ms |

T0 UTC: `2026-11-02T22:51:36.268Z` (`1793659896268`). Production child start: `2026-11-02T22:51:35.8552639Z`. EOF observation UTC `2026-11-02T22:52:53.9606543+00:00`; child-exit observation UTC `2026-11-02T22:52:54.0199738+00:00`. Обе монотонные отметки записаны в polling loop **до** cleanup, supplementary reads и диагностики. Supervisory origin509989187, elapsed78.2054119s, budget570/600/30 применён только для capture/containment; `work_expired=false`. F450/480/510 не расширялись.

## Результат и граница диагностики

Production сохранил восемь top-level файлов: inputs, activation, started, capture, launch, два protocol log и outer-result. Все это публичные contract outputs; profile/key directories не читались и не экспортировались.

`outer-result.json`:3487 bytes, SHA256 `cde709e9ef7c8d6a23d05d60d117dce455558806315d47718aa052d27a206410`.

`started.json` — фактический exclusive outer claim64788. Launcher62452 waited=true/code1; `harness=null`. `native-started.json`, clock-request/reply, `join-<harness_pid>.json`, `permit-<harness_pid>.json`, assertions и observer outputs отсутствуют (это также следует из полного top-level списка). Native admission completion и native handle binding **не установлены**. Вход в сценарий требует завершения `AdmittedRun::load_for`; он не завершился до создания native claim. Конкретная отказавшая проверка внутри раннего пути не установлена. Functional mixed/hooks/restarts/offline/recovery результаты отсутствуют; итоговый success-tail admission/revalidation не завершён.

Сохранённый stdout protocol214B имеет SHA256 `22a08ce2457e7b59b56461bdc501aaf25b2b88e507f7041e406d935c8c9002af`. Stderr protocol222B — `6310717bd8fb3b602e35f7c8b74f103972a7c7b55f76c2f2b01c034db3f9d3a5`. Кардинальности:

| Публичный факт | Количество |
| --- | ---: |
| running 1 test | 1 |
| точный named selector | 1 |
| named FAILED | 1 |
| aggregate FAILED / 0 passed / 1 failed | 1 |
| named ok | 0 |
| native functional marker | 0 |
| wrapper completed marker | 0 |
| S54_CAPTURE | 1 |

Rust aggregate: `finished in 12.18s`, 300 filtered out. Это длительность test invocation, не полный F-clock.

`S54_CAPTURE` подтверждает native stdout325B/stderr74B, оба EOF=true, overflow=false, failed=false. Production output persistence complete=true, bounded streams complete=true, malformed protocol0. Но опубликованный `Write-S54Transcript` сохраняет **только отфильтрованный protocol**: stdout redacted_lines8, stderr redacted_lines7, retained captured-prefix325B/590B с хешами в outer-result. Непротокольный текст ошибки Rust и launcher diagnostic не сохранён как raw файл и после выхода недоступен. Raw stdout/stderr самого production outer сохранены owner без фильтрации:0B и325B соответственно; stderr содержит только общий `S54 outer checks failed; retained attempt is not reusable`. По этим доказательствам нельзя честно назвать точную первопричину. Исходники для восстановления диагностики не менялись, дополнительный запуск не делался.

Durable `actual_desktop_return=pending` оставлен неизменным. Отдельное доказательство фактического возврата — native waited/code1 в `run-01.receipt.json` и фактический tool return `25f7ab`; это не превращает durable pending в success.

## Очистка и сохранность

Production cleanup verified=true/forced=false, before=[]/after=[]; supervisor actual handle Job membership verified=true, child waited=true, reader_failed=false, complete EOF, cleanup verified=true/forced=false/failed=false, before=[]/after=[]. Чужие процессы не завершались.

Read-only postcheck фактически вернулся0 (`da383e`,6.180629s): успешная native CIM inventory exact nine images0; исходники517 и images9/input pins/navigation3 неизменны, HEAD прежний, index пуст; historical85 pins проверены, F01/F02 run по-прежнему пусты, P отсутствует. Отдельная проверка сохранности после неуспешного Run не заменяет незавершённую production final-tail validation.

Pin manifest рядом с этим отчётом ссылается на оригинальные public/admin artifacts, source517/current image9/history85. Большие артефакты не копировались. Дальнейшая диагностика/изменение контракта или новая попытка требуют решения root; текущий этап остановлен на review. Whole S5.4 не принят.
