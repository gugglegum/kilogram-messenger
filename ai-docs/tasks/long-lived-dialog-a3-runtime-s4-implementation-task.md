# A3-S4: реализация двухпроцессного runtime после принятого контракта

2026-10-25, Asia/Yekaterinburg. **Реализация разрешена оркестратором.**
Следующая сдача — законченный S4 production path и evidence на implementation review.
Commit/push кода — только после явной приёмки root и отдельного поручения публикации.

## Основание, baseline и исполнитель

Пользователь прямо разрешил диалог двух процессов с перезапусками/offline-доставкой,
contract review, implementation review/repair и commit/push после приёмки.
Действуют AGENTS.md и постоянное разрешение на обычную публикацию принятой работы.
Нового human approval для реализации или согласованных repair cycles не требуется.

Исполнитель — прежний OD1 «Подготовить baseline lossless sync»,
`01a1653c-0154-7611-8e8c-044fbe1e4d7a`, GPT-6.1 Sol Max, обычный режим без Fast.
Новые чаты/субагенты не создавать. Root `01a164ec-589c-7b41-bc34-3ba158d1f22b`
проверяет implementation и затем exact опубликованный diff.

Baseline **9547ed171c64bf5ea529b4ba487acf6aa1a61b5b**, sole parent
`7f82036ee5803edd64be98b73572604250f5961c` (S3 product baseline).
Документация S4 опубликована: root независимо сверил7 changed paths/blobs,
6 raw documents+manifest, parent/subject, clean index/worktree и GitHub master
(compare exact SHA→master: identical, ahead0/behind0/files0). Новый этот task и
изменения README/coordination принадлежат implementation, не принятой публикации.

Нормативны:

- [S4 contract](long-lived-dialog-a3-runtime-s4-contract.md):97759 bytes,
  SHA256 `5e21324167573281e1668cd6091c5a6e63ff72b0894009c57c0a122b6b0a688c`;
- [review2/acceptance](long-lived-dialog-a3-runtime-s4-contract-review-2.md):8138 bytes,
  SHA256 `33844c4d2c749f5d4d84865f4398abb419eeb7c8a60d812dd1e63f36b25d034d`;
- [documentation manifest](long-lived-dialog-a3-runtime-s4-contract-accepted-files.json):
  2890 bytes, SHA256 `15a804b51209aa00efb2a89858f07a118b2f7edfffe6765f661dc8349f6fffcc`,
  Git blob `50d318c2119315cf5b607e67ff302892167f4eb0`;
- [whole-A3 contract](long-lived-dialog-a3-contract.md), принятые S1–S3 interfaces и
  [S3 implementation acceptance](long-lived-dialog-a3-protocol-s3-implementation-review-1.md).

Contract/reviews/manifests и старые evidence frozen. Не исправлять нормативные решения
молча в процессе реализации: новое существенное противоречие/API gap вынести root
с точным evidence и предлагаемым решением, продолжая независимую разрешённую работу.

## Результат и порядок реализации

Реализовать весь принятый **fresh production S4**: ordinary enrollment/profile/CLI,
authenticated IPC, dedicated single owner, current-policy/Fresh2/Ready2 activation,
closed indexed writer, offline disk materialization, ordered ingress/repair/receipts,
реальный transport2 direct/own relay, bounded scheduling и restart/recovery.
CLI, сеть и P/F acceptance вызывают одинаковые production functions. Small component
PASS не заменяет runtime результат. `main.rs` остаётся dispatch, cfg(test) не прячет
реальную реализацию. Не запускать S5, UI/onboarding/groups/multi-device/mailbox,
M1.2.63, public rollout или deferred A2 reproducer.

Следовать dependency order §12 контракта:

1. Typed local records/RuntimeRoot6/bootstrap/readers/fences и common atomic lowering.
   ComponentRoot5 encoding сохраняется; RuntimeRoot6 prefix/marker обязателен уже
   в первом committed root. Closed operation включает Head/Ready/native/history/
   credits в одном commit; generic/component/retry API не обходят owner-purpose guard.
2. Production NativeState2/Policy2/Fresh2/Ready2 и ordinary CLI/IPC. Account/Device/
   D/E/SID/Q и selected proof сохраняются между retry/restart. Current permission
   не заменяется historical proof. До history Init новая очередь NeedHandshake;
   local encryption только после durable Active2 и valid local Root2 policy.
3. Offline queue/ingress/repair/ACK и transport/locator. Disk backlog>128 допустим,
   RAM refs bounded128 с64/64 при двух C; возврат RAM slots не удаляет debt. ACK
   только после readable commit. Последняя policy/time/actual lease проверка перед
   commit, clean explicit abort отдельно от IO/ambiguous poison/reopen.
4. Production Minimal/explicit operator route config, URL-only RelayOnly locator,
   per-restart ephemeral endpoint/new durable revision до export. Reconnect меняет
   transport binding, не native session/source bytes. Bounds действуют до admission.
5. После локальных checks — полный P/F и выбранные regressions/static на final diff,
   implementation report → root review/repair → только потом accepted commit/push.

Existing V4/L1 поддерживается прежним путём и safe version refusals; реальный upgrade/
handoff остаётся обязательным S5. Не объявлять весь A3/MVP готовым после fresh S4.

## Verification и ресурсы

Матрица §11 обязательна, включая review repairs F10–F12 и14 F01 cases. Сначала
компиляция/короткие адресные проверки; долгий P запускать после готовности затронутых
seams. Перед final run зафиксировать точные selectors и source snapshot. Тестировать
поведение/инварианты, не создавать избыточные tests, повторяющие implementation.

- **P≤2400s:** реальные два application процесса и два C; C1 A640 one-way с kill256/
  restart512,20 duplex DH rounds и256 offline encrypted Text при действительно
  остановленном B, restart A offline, full64+head/≥8 pages/restarts/lost ACK/late chain,
  C2 duplex32/сторону, own relay/reconnect и exact cursor history. Final counts по
  контракту: C1 A936/B640, C2 A32/B32,1640 Text+4 genesis,3288 successful packet calls;
  aborted attempts/duplicate retries считать отдельно. Нет mock lease/counters/rows.
- **F≤1200s:** все policy/renewal/Ready/bootstrap/queue/ingress/receipt/IO/version
  gates, clean post-staging abort, old schema5 writer refusal, repeated locator kills,
  actual route/admission bounds. Kill/graceful/injection/clock hook различать явно.
- **Components/regressions≤600s:** concrete selectors по изменённым boundaries,
  bounds/atomics/negative paths и нужные прежние regressions. Strict Clippy all-targets,
  fmt/locked build/helper syntax/diff checks отдельно. Не повторять heavy S1/S2 100k
  или S3 long probes без затронувшего изменения/конкретного нового сомнения.

Эти ceilings — суммы final runner elapsed соответствующего набора, не per-test и
не параллельный wall time; static/compile-only отдельно. Budget превышен — сохранить
timings/failures и вынести root, не уменьшать counts/не менять profile/caps молча.
Измерять setup/materialize/catch-up/history/IO/latency отдельно; не выдавать полный
scenario elapsed за скорость чтения истории. Source-scoped reuse объяснить явно.

Все новые P/F/components/current CLI/predecessor helper — Cargo **verification**,
locked/offline, Jobs12/BelowNormal. No Fast, release/verification-scale override нет.
На Windows compile harnesses `--no-run`, запуск только stable runner. Process CLI
fixed `target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe`; F10 helper
fixed `target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe`.
Hash/revision в evidence, не в имени EXE. Независимые проверки можно параллелить,
но не writers одного fixture/runner target и не перезапись используемого EXE.

F10 строит публичный probe из isolated exact7f82036 sources/Cargo.lock/features,
а не текущей S4 state library. Positive ComponentRoot5 control/correct owner/config
обязательны; old-version refusal не подменяется ошибкой setup. Основной checkout
не переключать/reset-ить ради predecessor; сохранить source provenance helper отдельно.

## Сохранность и сдача

Все fixtures новые isolated workspace temp. Frozen S1/S2 DB не открывать/не менять;
принятые historical evidence/manifests не переписывать. L1 compatibility EXE
d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1 сохраняется.
`ai-docs/development-environment.md` clean SHA53074ff… исключён; удалённую чужую правку
не восстанавливать. Любые unrelated изменения сохранять. Secrets/живые пользовательские
данные и временные state vaults не включать в отчёты/публикацию.

Сдача: `long-lived-dialog-a3-runtime-s4-implementation.md`, bounded evidence directory,
точный changed-file/source/config/test manifest, hashes source/EXE/artifacts, commands/
exit codes/PIDs, final matrix с counts и пределами claims. Сохранить промежуточные FAIL,
явно связать reused results с неизменными исходниками. Root должен иметь возможность
проверить результат и адресно повторить существенные checks, без полного corpus по кругу.

Один содержательный callback root с baseline/path/hash, результатами всех suites,
неразрешёнными вопросами и manifest, затем stop до implementation review. Существенную
блокировку/необходимую поправку контракта сообщать раньше. Прямое human разрешение
callback в твоём чате `01a16faf-e3a0-76a3-912a-1249ed4ca1e5` действует до отзыва;
при callback не менять model/thinking root. Commit/push implementation не выполнять
до отдельной явной приёмки и exact allowlist от оркестратора.
