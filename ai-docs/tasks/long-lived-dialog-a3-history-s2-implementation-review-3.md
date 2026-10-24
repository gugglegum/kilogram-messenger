# A3-S2 implementation review 3

2026-10-24. **Attempt3 проверен: normative time FAIL, S2 не принят.** Сравнительная
release диагностика подтверждает высокую цену unoptimized сборки. Новый product
дефект не воспроизведён. Следующий шаг определён явным
[verification-profile amendment1](long-lived-dialog-a3-history-s2-verification-profile-amendment-1.md).
Ceiling2400s повторно не увеличивается; product algorithms не переделываются.

Вход: [report3](long-lived-dialog-a3-history-s2-implementation-3.md), SHA
`538f0bf501db47fa8bade58de03df51416596bce9031df57357a9bb49ae660a1`, blob
`c79e2115a670d4450587acc82c6f4f7af6ac2357`; quality SHA
`4c12c8d787e93f331c5a1ab5070c850830b420ea71c9e2295ef510ed78f1297a`.
HEAD61524df3a77203d73673f47d38decf6b817615e3, index пуст.

## Независимая проверка

Root сверил42 evidence files/19 current-source и snapshot pairs/6 current EXE и
snapshot pairs; hashes/bytes совпали. Library verification7ce75659… и release022a782a…
соответствуют before-run pins. Единственное изменение build inputs после repair2 —
test-only ceiling/timers в history_scale.rs. Product tests заново root не запускал:
неизменность inputs и предыдущий независимый review2 подтверждены.

Прочитаны harness diff, порядок full phases/budget calls, raw logs и оба profiler JSON;
root пересчитал суммы/средние, сверил все phase call counts, namespace counters,
cuts/cache hits/Work. Фактическая полная fixture только хэшировалась, не открывалась:
1182797824B, SHA bc3410a155a55b115f458f541b2fbd3ee6f9900ee33b34ccffd24358d39dbafb.

Root [verification JSON](long-lived-dialog-a3-history-s2-review-evidence-3/review-verification-3.json),
SHA `9be37a9fd511a5a885edfc6b4ad0ac1bf0931cd7c3658fa27e514518ec3b5dc4`.

## Статус full attempt

Ceiling event2400.1566769s/harness2400.18s подтверждены. Завершены100k exact range
и16 ordinary appends. После последнего memory marker budget проверяется только
в midpoint suffix loop; достижение first/point/old finished/3 checkpoint page checks
выводится из pinned control flow. Это не новый OS-process recovery proof.
Полный suffix и final100k C2 assertion не подтверждены.

Из raw memory lines root независимо пересчитал:

| Фаза |Raw growth B|Residual growth B|Residual headroom B|
|---|---:|---:|---:|
|Warm range|92053504|32088384|1466048|
|После ordinary append|92389376|33308672|245760|

Оба96/32MiB gate прошли по записанным samples одного PID29124. Запас240KiB мал;
это сохраняется как фактическое ограничение, без нового memory allowance.
100k local phase timers/p95/IO и numeric maxima не напечатаны до error; они
UNAVAILABLE. Новый run должен печатать completed phase summary до начала следующей
дорогой фазы, чтобы поздний timeout не терял уже измеренные результаты.

## Вывод из matched pair

В обеих instrumented выборках12 pages+12 points,10746 node opens,1399 value opens,
420 source checks,2011 cache hits,24 Work и одинаковые namespace vectors.
Mean page141.521→10.412ms, point12.190→0.477ms;13.592×/25.560× соответственно.
Product integrity/source проверки не выключались. Это реальное evidence влияния
сборки, но не unprofiled100k/UX benchmark. Cache state и backend reads различаются:
root прочитал debug-only redb4.2.0 open traversal. Простая смена всего gate на release
не сохранила бы diagnostic/debug configuration.

Выбрана optimization3 с явно включёнными debug assertions и overflow checks в
отдельном `verification-scale`, обычный verification не меняется. Это исправление
методики большого теста после измерений; старый FAIL не становится PASS. Relative
ratio≤4 и scale completion не заменяют будущую абсолютную UX latency приёмку.

## Следующее поручение и публикация

Тот же OD1, GPT-6.1 Sol Max/default без Fast. Выполнить amendment1: узкий Cargo
profile, сохранение phase evidence, короткие checks/precheck, один fresh full run.
Новый report/evidence4; предыдущие артефакты и весь исходный100k store сохраняются.
До отдельного root review — без commit/push и соседних wire/native/Root2/migration/
runtime/UI работ. development-environment.md остаётся clean53074ff… по human override.
