# Block4K — safe early native admission diagnostics, candidate for review

2026-11-03. Baseline `e15492a9b9733ba43afc9c14b79bcbb754efb6c0`. Task: `long-lived-dialog-a3-migration-s5-final-block4k-admission-diagnostics-task-01.md`. Реализован узкий источник типизированной диагностики для `AdmittedRun::load_for` и её сохранение через настоящий native forwarder и production protocol writer. Проверки завершены; commit/push и production Prepare/activation/Run не выполнялись.

F03 остаётся FAILED/consumed. Его первопричина **UNKNOWN**: новые публичные проверки не восстанавливают потерянный текст старого отказа. В candidate входят неизменённые [F03 report](long-lived-dialog-a3-migration-s5-final-block4j-f-runtime-report-03.md) и [F03 pin manifest](long-lived-dialog-a3-migration-s5-final-block4j-f-runtime-report-03.pins.json):9852B/SHA256 `4e3b1dc7de3d2746f3a8b17102dac6acd701e344b66e93e93f8733b43cc749ae`;158296B/SHA256 `8aef1971e8843515707a8a7cc6a2bb2c1f5567e4fe3a246d183cdebc213dc575`.

## Contract and implementation

Exact advisory grammar:

```text
S54_ADMISSION_DIAGNOSTIC v1 stage=inputs reason=required_input
```

Новая Rust helper `migration_s54_admission_diagnostic.rs` содержит конечный enum из29 допустимых пар и возвращает только compile-time static line. Никакие Display/Debug исходной ошибки, path/env/status/message/peer/nonce/key/JSON в эту строку не подставляются. `at(code, closure)` выполняет исходную проверку один раз на прежнем месте, сохраняет оригинальный anyhow error chain и уже найденный более конкретный typed context. `load_for` сохраняет тот же Err и при ошибке отдельно пишет одну статическую строку в stderr. Ошибка sink не заменяет исходный Err. Public inspection получает тот же контекст, но сама его не печатает и не приобретает runtime capability.

| Stage | Закрытые reason codes | Проверяемый контекст |
| --- | --- | --- |
| inputs | required_input, digest, pin_read, document | обязательные четыре env inputs; digest; фактические pin/doc reads |
| policy | unknown, binding | production policy construction; запрет fixture capability |
| namespace, directory | binding | fixed namespace/purpose/deadline/references; допустимый состав/claim directory |
| source | unknown, head, set, pin_read | reviewed HEAD; точный source set; source pins |
| images | unknown, roles, pin_read | девять ролей/duplicates; фактические image pins |
| runtime | unknown, policy, environment, config | resource flags; actual frozen env; Cargo config |
| selection | binding | точный selector/list/isolated selection |
| activation | binding, validity | внешняя activation и её окно |
| launch, capture | binding | исходные identity/PID/input/clock/capture отношения |
| compiler | unknown | shared compiler/embedded closure validation |
| native_claim | binding, create | started/caller/claim binding и exclusive native claim creation |
| clock | binding | request/reply/read/wait/original-clock binding |
| admission | unknown | непомеченная ошибка раннего пути |

Коды обозначают группу отказавшей проверки, а не воспроизводят arbitrary underlying error. Например source pin read/hash refusal даёт `source/pin_read`; underlying IO не классифицируется отдельно. Непомеченные ошибки внутри source/images/runtime/compiler остаются explicit `unknown` с известным stage; остальные без контекста — `admission/unknown`. По коду `unknown` нельзя утверждать более точную причину.

В `KilogramS54Output.Protocol` добавлен конечный allowlist тех же29 пар; проверяется точная версия, поля, регистр и окончание строки. Обычная token-shaped строка не становится допустимой. На stream допускаются максимум4 диагностические строки/512 bytes, строка не длиннее128 символов; все допустимые строки ASCII и короче этого byte bound. Unknown pairs/extra tails/oversized lines/лишние CR/неполные строки и invalid UTF-8 не сохраняются как допустимая диагностика. Redaction/drop/malformed/encoding/completeness учитываются в существующем bounded transcript. Новые поля `admission_diagnostics` имеют `classification=advisory_failure_evidence`, `authority=none`, count/bytes/dropped и явные bounds. Лимит stream/transcript1MiB и прежние success marker/cardinality проверки сохранены.

Закрыт случай на границе forwarder: его перевод строки перед `S54_CAPTURE` мог завершить оборванную native stderr строку. Теперь к такому хвосту сначала добавляется фиксированная отметка неполноты, поэтому diagnostic без своего newline не проходит точную грамматику. Произвольный текст по-прежнему редактируется; raw stderr file/side channel не добавлен. Проверка проходит через настоящий `Invoke-S54SingleExecutable -ForwardOutput`, `ForwardNew` и `Write-S54Transcript`, а не через отдельный имитатор writer.

Диагностика не включена в functional/native/wrapper marker sets. Она не создаёт claim, readiness, permit или success. Результат с одним diagnostic marker отвергается существующим `Assert-S54NativeOutput`. Ошибка/ненулевой exit/unfinished validation остаются ошибкой; clock origins, операции проверок, порядок, caps, workload и deadlines не изменены.

F identity обновлена на fixed `F-s54-mixed-hooks-feasibility-01-attempt-04` в PS/Rust и соответствующих exact tests. F01/F02/F03 явно отвергаются actual PS inventory/output policy и Rust purpose/shared admission. Case/selector/F450-480-510/P01/P2350-2375-2400 сохранены. Новый PS test helper зарегистрирован в обоих source closure helper lists (17). Фактических F04/P run/public directories нет; ни новой activation, ни allocation task этот candidate не создаёт.

## Actual focused checks

Администрация: `.tmp/s54-block4k-admission-diagnostics-01`. Fresh stable target: `target/s54-diagnostics-source-checks`, ownership записан до сборки. Исторический H source-check target не менялся, его четыре image pins входят в protected set. F03 nine runtime images и verification-scale Cargo origins/captures/dependencies сохранены. Использованы pinned installed pwsh/Rust1.98, Jobs2/BelowNormal/offline/locked, только process-local environment; `CARGO_TARGET_DIR` не устанавливался. Wrapper передаёт Cargo `--target-dir` в owned nonproduction target и ограничивает все stable copies этим target.

Общий фактический entry каждого шага:

```text
pinned pwsh -NoProfile -NonInteractive -File .tmp/s54-block4k-admission-diagnostics-01/owner.ps1 -Step <step>
```

Внутри compile/rust используется `scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Profile verification -CargoJobs 2 -SkipCacheStatus`; compile-only перед test execution. Для exact tests добавлены Run/NoCapture/IsolatedTestsFile/BeforeAggregate и skip aggregate filters. Cargo argv/copy paths записаны в step cargo.json. Сетевой harness через прямой `cargo test` не запускался. Четыре завершающих aggregate invocations имели0 selected tests; остальные tests не запускались.

| Шаг | Фактический return | Exit / elapsed | Содержание |
| --- | --- | --- | --- |
| format-02 | 16de60 | 0 /1.4888597s | scoped Rust format + PS parse |
| compile-01 | bdaee4 (launch5adbef/session20088) | 0 /362.706601s | compile-only,1170/1200/30 |
| format-03 | d01c67 | 0 /1.2375402s | окончательные Rust error-chain tests + PS parse |
| rust-01 | 40ed22 (launch233ea2/session88432) | 0 /74.9309326s | 11 exact tests passed; F03 shared-admission branch NOT executed |
| ps-02 | 404319 (launch6b84f8/session65892) | 0 /37.5698002s | purpose30 + diagnostics13 |
| rust-02 | b37a10 (launcha3442a/session98121) | 0 /11.4332553s | 2 exact successful public fixture exports; F03 shared-admission branch NOT executed |
| ps-03 | bc2c17 (launchc2776e/session11589) | 0 /10.1537646s | 36 shared preparation-order/actual PS inventory checks, fresh exported F04/P01 fixtures |
| seal-02 | e2095f | 0 /7.8621392s | scoped format/diff, protected/source/native inventory |

Все некомпиляционные bounded шаги570/600/30; work_expired=false, native child waited=true, actual handle/Job verified=true, complete bounded stdout/stderr EOF, reader_failed=false. Compile-01 и rust-01 cleanup были **forced** для оставшихся owned Job PID10408 и42640 соответственно; точная identity этих residual процессов не установлена. Cleanup verified=true/after=[]/failed=false; это административная source-check очистка, не успешный production runtime verdict. Последующие rust-02/ps-02/ps-03/seal-02 cleanup clean/nonforced. Итоговая native process inventory — relevant processes0.

11 exact Rust tests:3 новых diagnostic/shared-check tests; существующие successful P/F fixture admission и fixed purpose; missing/changed/oversized/duplicate inputs; source/config/env/tool drift; strict JSON/caps; compiler parity и embedded compiler closure. Два последних test assertions теперь ищут прежнюю причину в сохранённой error chain, поскольку top-level Display содержит typed context; test output использует fixed expected label. Все18 compiler parity и5 embedded refusals прошли. Assertions/validators не ослаблены.

PS diagnostics13: parity всех29 producer codes с actual filter; successful stored marker/cardinality; **реальный missing-input `AdmittedRun::load_for` child exit101** до любых F04 allocation/claim; forged tokens/tails/newline/private-canary; count/byte bounds; oversized/partial/invalid UTF-8; existing normal/tail/nonzero/partial/overflow owner/transcript regressions. Сохранённый реальный ранний refusal:

```text
S54_ADMISSION_DIAGNOSTIC v1 stage=inputs reason=required_input
```

Его native capture: stdout344B/stderr224B, EOF=true, overflow=false/failed=false; final transcript complete=true и advisory count1. Это ожидаемый отказ публичного child, не реальный F scenario. Canary — фиксированные synthetic test values; arbitrary private-canary text отсутствует в сохранённых protocol files. Полные public benign case outcomes/пины/cleanup доступны в `ps-02/diagnostics/results.json`; fresh existing regression roots перечислены там явно, исторические fixtures не клонировались.

Успешные public F04/P01 exports созданы заново существующими Rust factories. Actual PS inventory принимает F04 и P01, отвергает F01/F02/F03. Это schema/guard tests на tiny fake files, не production Prepare.

Сохранены три служебных неуспешных шага с exit1, waited/EOF/nonforced cleanup: format-01 (`7bcb7a`) остановился на navigation hashes из предшествовавшего J вместо актуального root dispatch; текущие navigation pins вынесены в отдельный immutable observation. ps-01 (`4b6132`) остановился на конфликте локальной переменной `purpose` с validated parameter `Purpose`; имя исправлено. seal-01 (`a19a2b`) после сохранностных checks остановился на передаче целого array source paths как одного path; исправлен только служебный enumerate. Их receipts/stdout/stderr и прежние work scripts сохранены. Продуктовые проверки и попытки F не повторялись из-за этих отказов.

## Review 01 correction — actual shared F03 refusal

Root review `long-lived-dialog-a3-migration-s5-final-block4k-review-01.md` выявил ошибку покрытия: в F shared-admission test оставался `0..8`, поэтому добавленная ветвь mutation8/F03 не выполнялась. Вместо этого расширен был цикл другого activation/launch/capture test, повторявший последнюю мутацию. Прежний report неверно приписывал Rust shared-admission выполнение F03. Purpose и PS refusals были выполнены, но это отдельные проверки и не замена отсутствовавшему случаю. Исходные receipts не переписывались; coverage statement выше исправлен.

Изменён только `migration_s54_admission_tests.rs`: в `s54_admission_rejects_activation_launch_capture_stale_and_clock_reset` восстановлено `0..8`, в `s54_f_admission_full_public_fixture_binds_fixed_purpose_and_refuses_p_substitution` установлен `0..9`. Счётчики с assertions требуют ровно8 и9 выполненных мутаций соответственно. Статический F03 marker печатается только после реального отказа `f.inspect()` для mutation8. Это test observability, не production diagnostic/success marker.

Fresh evidence: `.tmp/s54-block4k-admission-diagnostics-01/review-01-fix`. Raw preimages прежних Rust test source, report, pins, candidate и source diff сохранены с original→saved pins. Родительские receipts, failed steps, scripts и candidate/source-diff bytes сохранены; новый current candidate — `review-01-fix/candidate.json`. Прежние stable nonproduction image pins являются историческими наблюдениями: только разрешённый source-check cache перестроен, актуальные image pins взяты из нового `rust-01.images.json`. F03 runtime images/origins и исторические H images не менялись.

| Fresh bounded step | Фактический return | Exit / elapsed | Результат |
| --- | --- | --- | --- |
| review-01-fix/format-01 | 829ab6 | 0 /1.2896689s | один исправленный Rust test file |
| review-01-fix/compile-01 | fa2880 (launch7d5b0f/session3961) | 0 /36.1389086s | compile-only, existing target,1170/1200/30 |
| review-01-fix/rust-01 | e4f5c3 (launch9682e2/session9500) | 0 /13.3774382s | только два порученных exact tests,570/600/30 |
| review-01-fix/seal-01 | 5aa987 | 0 /6.8460296s | scoped preservation/output/format checks |

Actual focused output:

```text
s54_activation_launch_capture_mutations_executed=8
s54_f_shared_admission_refusal=F03;mutation=8;inspect=refused
s54_f_shared_admission_mutations_executed=9
```

Оба exact tests имеют по одному native invocation и по одному aggregate `1 passed; 0 failed` (2.28s и2.62s). Stable runner дополнительно завершил четыре пустых aggregate invocations; других selected tests не было. Неизменённые PS checks и прежние11 exact tests не повторялись. Реальное Rust shared-admission покрытие F03 получено **этим** исправленным запуском, а не историческими rust-01/rust-02.

Новые receipts: actual handle/Job verified, native waited/exit0, complete EOF, reader_failed=false, work_expired=false. Compile-only cleanup forced для owned residual PID68520 (identity не установлена), verified=true/after=[]; это административная source-check очистка. Новый actual rust run и final seal — clean/nonforced, relevant processes0. Seal подтвердил136 protected pins, остальные518 source files и остальные16 прежних candidate files до обновления report/pins; текущая root navigation сохранена, HEAD/index прежние, F04/P allocations отсутствуют. F03 cause всё ещё UNKNOWN. Production Prepare/activation/Run, commit/push не выполнялись.

## Review boundary

Итог: source closure519 files; source allowlist13 paths. Seal проверил136 deduplicated protected pins (F03 public/admin/report/manifest, исторические F01/F02/H, F03 images/compiler origins/dep-info/captures), неизменённые вне allowlist source pins, текущие root navigation3, HEAD/index, отсутствие F04/P allocation и native relevant process inventory0. Source diff и exact candidate manifest находятся в `.tmp/s54-block4k-admission-diagnostics-01`; соседний report pin manifest ссылается на оригиналы, без копий больших binaries/captures. Root navigation/task/review files не входят в executor allowlist.

Candidate включает13 source/test files,2 неизменённых F03 report files, этот report и его pin manifest. До явной приёмки root commit/push не выполняется. Настоящий F04 требует accepted publication, новой preparation и внешней root-issued identity/activation; whole S5.4 не принят.
