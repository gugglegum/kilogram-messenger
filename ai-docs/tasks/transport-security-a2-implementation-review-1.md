# A2 supporting implementation: review 1

Дата: 2026-10-24, Asia/Yekaterinburg. Reviewer: root, GPT-6 Astra.

**Решение: принять поддерживающую часть к commit/push.** Блокирующих замечаний к
предъявленному dependency/code/CI diff не найдено. Это не закрытие всего A2, не
приёмка отложенного security reproducer и не разрешение внешней эксплуатации.
Основание scope — [task, уточнение 2026-10-24](transport-security-a2-task.md).

## Проверенный вход

HEAD `bc86cd25518077cef8977522e0c750f3bee2c0d7`, индекс пуст. Отчёт
[исполнителя](transport-security-a2-implementation.md), SHA-256
`278d0b0cb2a48319353459ac6a9d4da6dae1cc9b0d9a0e8b411871e2acde8876`.
Code/CI patch из 8 файлов:
`7c506febccd357286d2bc06de935bf2132be8286f201a657ba55241baf98c28e`;
полный входной patch из 73 файлов:
`e45587ee97d8e2c262708529b363f4a1c95c3b7c2397374069c9d09ed79927b8`.
Оба reverse-check прошли. Все 1080 source hashes и 74 artifact hashes совпали;
набор source manifest независимо сопоставлен с tracked/new файлами вне ai-docs.
[Результат сверки](transport-security-a2-root-review-evidence/input-check.json).
Исторические manifests фиксируют вход review и не переписываются после этого решения.

## Оценка изменений

Iroh/base/dns/relay закреплены на официальной 1.1.0 с известными upstream fixes;
точечные совместимые исправления TLS/crypto/runtime transitives обоснованы найденными
advisories. Проверены lockfile/version diff, production feature tree и registry
checksums. Relay server dependency ограничена dev graph. Продуктовые Rust modules,
wire/ALPN, identity policy, TLS verification, schema и durable commit code не менялись.

Обычный local relay test действительно использует transport adapter, собственный
loopback relay и peer TLS; проверяет точные frames, route policies, peer identity,
reconnect и fresh/symmetric exporters. Изменение process harness выбирает отдельный
старый runtime для второй стороны и записывает оба executable hashes.

Advisory script проверен по коду и независимым запуском: exact database SHA/clean
checkout, pinned auditor, manifest floor, все Iroh lock entries, archives, fail-closed
итог и отсутствие ignore list. CI job manual workflow сохраняет полный строгий статус.
Он намеренно остаётся красным при известных предупреждениях; это рабочая диагностика,
а не пройденный security gate. Hosted CI не запускался, его результат не заявляется.

## Независимые проверки

- [Transport stable runner](transport-security-a2-root-review-evidence/transport.log):
  10 library + 1 ordinary relay test PASS; три reconnect с exact 8123-byte frames.
  CargoJobs2, BelowNormal, offline; deferred test не обнаружен и не запускался.
- [Advisory replay](transport-security-a2-root-review-evidence/advisory-gate.json):
  source/version PASS, 0 vulnerability records, 7 warnings, exit 1 / FAIL на той же базе.
- [Mixed runtime](transport-security-a2-root-review-evidence/mixed-result.json):
  новый EXE `d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`
  и сохранённый OD1 EXE `3f42245b899e678c423b3137b5d99dc225cb0c2bc36d99b5bfa99daf506a6626`;
  64 Text за 62.30s, generation8/8 на обеих сторонах, pending/unacked=0.
  EventId/requestId/plaintext и остальные history fields совпали на обоих endpoints.
- [Reopen обоих новым EXE](transport-security-a2-root-review-evidence/mixed-reopen-result.json):
  64 exact records и 272 immutable/native hashes, 4.47s, PASS. Review wrapper сначала
  прочитал неверный путь summary; фактический результат найден в `reopen/result.json`,
  дополнительных запусков и изменений продукта для этого не потребовалось.
- В пяти предоставленных process runs независимо сопоставлены histories обоих peers,
  уникальность EventId, counts и fingerprints: 64/64/67/81/64 records. C3 partial
  evidence действительно содержит full64 pending и requested32/saved17/partial=true.
  Остальные 94 tests, strict Clippy и static checks приняты по сверенным evidence;
  весь дорогой corpus/crash suite повторно не запускался.

## Непринятые границы

Специальная security runtime regression остаётся DEFERRED/unresolved. Её прежние
одинаковые timeout не доказывают ни наличие проблемы в исправленной версии, ни её
устранение. После safeguard review не разрабатывал, не отлаживал и не запускал её.

Семь предупреждений не подавляются. Шесть — maintenance notices; одно — `lru 0.16.4`
unsound. Source inspection rqrr подтверждает `LruCache<u8, ColoredRegion>` и `pop_lru`;
trigger из advisory (panicking key Drop в pop с продолжением после unwind) здесь не
найден. Это ограничение текущего consumer, а не доказательство безопасности всех
возможных paths. QR/GUI maintenance migration требует отдельной задачи.

Подтверждена только Windows loopback IP old/new совместимость и current/current
local relay. Другие платформы, old/new relay и внешние сети не проверены.
L1/fresh-profile ограничения OD1 сохраняются. Весь A2 и daily MVP остаются открыты.

## Сохранение принятого результата

Исполнителю разрешается один логически завершённый commit/push этой поддерживающей
части по отдельному accepted-files allowlist, включая root review и public evidence.
Предъявленный source/code/evidence не менять. README/coordination/status отражают
именно частичную приёмку. `ai-docs/development-environment.md` исключён и сохраняет
SHA-256 `330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
После callback root сверит committed blobs/path set, parent и фактический remote SHA.
Следующий функциональный этап автоматически не запускать.
