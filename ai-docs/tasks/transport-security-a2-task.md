# A2: безопасная версия Iroh и transport regression gate

Дата выбора: 2026-10-23. Статус на 2026-10-24: поддерживающая часть принята
[review 1](transport-security-a2-implementation-review-1.md) к commit/push;
весь A2 остаётся открытым (7 warnings и отложенная security runtime regression).
Модель: GPT-6.1 Sol, reasoning Max, обычный service tier default; Fast запрещён.
Исполнитель: `01a16a10-6a33-7823-9242-d1581fab824b`, host local,
«Закрыть A2: безопасность транспорта Iroh». Model/effort подтверждены по первому
turn_context; service tier проверен по действующей конфигурации, отдельного override нет.

## Уточнение продолжения 2026-10-24

Первый turn исполнителя прерван платформенным cybersecurity safeguard. Квота аккаунта
доступна. Сохранённые изменения и обычные transport tests остаются исходной точкой.
Оркестратор снимает из текущего поручения разработку, запуск и отладку воспроизведения
DoS. Не повторять заблокированную операцию, не переносить её в другой чат/инструмент
и не менять защитные настройки ради её выполнения. Дальнейшая работа — обновление
зависимостей, advisory/source review и обычные функциональные проверки приложения.

Уже полученный отрицательный результат специального теста не удалять из evidence:
на обеих версиях получен timeout, причина не установлена. Это не доказательство
сохранения уязвимости в исправленной версии и не успешная проверка исправления.
Runtime-проверка конкретного advisory остаётся незавершённой и явно указывается в
отчёте; нельзя называть весь A2 закрытым или утверждать готовность к внешней эксплуатации.

Новые незакоммиченные `tests/relay_batch.rs` и `scripts/test-iroh-relay-baseline.ps1`
не должны случайно запускаться агрегатными проверками и не входят в принимаемый diff.
Сохранить их без изменений вне Cargo test discovery в выделенном каталоге под
`.tmp/transport-security-a2/deferred/`, сверив исходные/конечные SHA-256 и абсолютные
пути внутри workspace перед перемещением. Не перемещать каталоги целиком. Существующие
логи сохраняются; не дорабатывать и не исполнять эти заготовки. Обычный local relay
test с корректным трафиком и стандартные transport/OD1 проверки остаются в scope.

Дополнительные RustSec findings нужно классифицировать. Точечные совместимые patch
updates реально используемых зависимостей разрешены с обоснованием и соответствующими
обычными regression checks. Не подавлять findings и не объявлять красный gate зелёным.
Если их устранение требует новой архитектуры/major migration, подготовить конкретное
предложение root. Сдать конечный проверяемый результат в доступных границах с явными
ограничениями; commit/push по-прежнему только после review.

## Основание и результат

Прямое указание пользователя в чате оркестратора
`01a164ec-589c-7b41-bc34-3ba158d1f22b`: выбрать следующий приоритет и передать тому же
исполнителю либо создать новый чат; модель/effort выбрать по сложности, кроме GPT-6
Astra, без1.5x Fast. Прежняя схема review/repair/callback и commit/push после явной
приёмки сохраняется. Root остаётся ревьюером, дополнительные Astra исполнители не нужны.

Предыдущий OD1 этап завершён: code `f80aabc30c5a9406ce147b96ed27c2ede5801697`,
docs `bc86cd25518077cef8977522e0c750f3bee2c0d7`; remote/706 committed blobs проверены
оркестратором. Начальный HEAD этой задачи — `bc86cd25518077cef8977522e0c750f3bee2c0d7`.
Полезный минимум контекста: AGENTS.md, этот task, раздел A2
[аудита](../reviews/2026-10-22-architecture-mvp-audit.md),
[OD1 acceptance](lossless-sync-a1-bidirectional-implementation-review-2.md),
[OD1 report](lossless-sync-a1-bidirectional-implementation.md).
Не читать все исторические RFC и большие логи без конкретной необходимости.

В Cargo.toml остаётся `iroh = "1.0.3"`, Cargo.lock разрешает iroh/base/dns/relay1.0.3.
Оркестратор заново открыл официальный
[GHSA-7cq4-mhxw-xw78](https://github.com/n0-computer/iroh/security/advisories/GHSA-7cq4-mhxw-xw78):
на2026-10-23 affected Iroh>=0.91.0,<=1.0.3, patched1.1.0. Special relay batch способен
заблокировать relay receive и вызвать CPU busy loop до restart. Это DoS, не доказанная
утечка E2EE. Обычный транспорт поддерживает relay; OD1 пока loopback-only. Поэтому A2
закрывается до расширения внешних сетевых испытаний, независимо от продолжающихся A1/A3.

**Пользовательский результат этого этапа:** проект собирается с обоснованной исправленной
совместимой версией transport stack; обычные relay regression и существующие
пользовательские пути проходят; есть воспроизводимая проверка security dependencies.
Никакого утверждения, что весь проект теперь безопасен или готов к daily MVP.

## Работа

1. Заново подтвердить upstream advisory, актуальные совместимые исправления и release
   notes. Выбрать минимально необходимый поддерживаемый compatible target, не вслепую
   latest. Составить краткую before/after таблицу Iroh и необходимых transitives с
   источниками и причинами. Проверить Cargo tree для actual/default features/targets:
   уязвимый Iroh не должен остаться достижимым второй копией. Проверить registry checksums.
2. Точечно обновить manifest floor и lockfile, чтобы обычное разрешение зависимостей не
   возвращало известную affected версию. Не делать общий cargo update всего workspace.
   Только необходимые API adaptations; сохранить ALPN/wire/route policies, exporter,
   identity binding, TLS verification и crypto/storage semantics.
3. Добавить/подключить воспроизводимый advisory check с датой/ревизией базы и понятным
   failure status в подходящий existing CI/script workflow. Проверить, есть ли этот GHSA
   в выбранной advisory базе: отсутствие записи не равно исправлению. Для A2 сохранить
   явный source/version gate. Нельзя blanket-ignore findings ради зелёного результата.
   Другие найденные advisories классифицировать по реальной достижимости и scope;
   не превращать задачу в произвольное обновление всего проекта.
4. Проверить штатный local relay transport с корректными сообщениями, reconnect и
   сохранением прогресса. Test-only локальный relay fixture не должен ослаблять
   production TLS verification или включать публичные сервисы. Проверку конкретного
   advisory описать по upstream evidence и точной версии; прямой runtime reproducer
   исключён из продолжения по уточнению выше. Не подменять его обычным smoke test и
   не заявлять отсутствующее runtime доказательство.

Нормальный compatible upgrade можно реализовывать сразу в этих границах. Если требуется
смена wire/ALPN, identity/authority model, migration, снятие проверки TLS, переход на
неподдерживаемый fork либо заметное расширение scope — сначала конкретное предложение
на review оркестратору. Не просить пользователя подтвердить уже разрешённую работу.

## Проверки приёмки

- Fresh advisory/version/Cargo tree evidence до/после; narrow lockfile diff объяснён.
- Штатный local relay delivery/reconnect; классифицировать test-only fixtures и
  реально проверенный transport. Runtime-проверка конкретного advisory остаётся
  отдельным незакрытым пунктом, не входит в продолжение после safeguard.
- Тесты kilogram-transport-iroh, включая настоящий TLS exporter/reconnect и mixed ALPN
  rejection; обычный direct/relay transport и relevant signed-envelope/runtime smoke.
- OD1 final EXE: compact duplex64 (32/сторону, >=8 receiving DH transitions/сторону),
  точные EventId/requestId/plaintext и история после обоих restart; C3 partial/full64
  prefix repair для транспортного изменения. Не считать fixture-only тест реальной сетью.
- Сохранённый до upgrade рабочий EXE с известным hash и постоянным именем позволяет
  проверить reopen прежнего OD1 state новым binary. Если Iroh upgrade влияет на peer
  interoperability, проверить actual old/new endpoints или явно доказать границу;
  не выводить wire compatibility только из semver. Старый pre-OD1 fence EXE не путать
  с baseline этого upgrade.
- Locked builds/checks, strict Clippy затронутых crates/targets, rustfmt/diff-check.
  Полный дорогой corpus, lifetime511 и14 crypto crash cases не повторять автоматически,
  если их код/зависимости не изменились и нет новой причины. Обосновать applicability.

## Ресурсы и сдача

CargoJobs2/BelowNormal. Network Rust harnesses на Windows только через
`scripts/run-cargo-tests-stable.ps1` (compile no-run, stable alias); field EXE также
по постоянному пути, revision/hash в evidence, не в имени. При downloads/advisory refresh
сеть разрешена по задаче; после resolve проверки locked, offline где возможно.
Тесты только на локальных временных данных/собственных endpoints. Sandbox ACL/DPAPI/
loopback отказ отделять от product failure и при необходимости применять штатную
escalation, не ослабляя security policies. Никаких privileged secret dumps.

Начальное постороннее изменение `ai-docs/development-environment.md` не трогать и не
включать в commit. SHA-256 `330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Новый task и изменения README/coordination принадлежат этой задаче. Исторические OD1
evidence/accepted manifest не переписывать. M1.2.63, A3, GUI, OD1 external rollout,
discovery/onboarding, mailbox/recovery и группы в эту задачу не входят.

Сдать `ai-docs/tasks/transport-security-a2-implementation.md` и компактный
`transport-security-a2-evidence/`: source/lockfile/artifact hashes, полный task diff,
upstream/advisory sources+date/database revision, before/after package tree, commands/
results/failures, build/process fingerprints и непроверенные границы. Только публичные
текстовые evidence; без private state, IPC bearer, secret keys или EXE в Git.
Сначала review/repair; commit/push только по явной приёмке оркестратора. После сдачи
один содержательный callback через send_message_to_thread в root chat выше, host local,
и остановка. Human authorization на callbacks подтверждается предыдущими прямыми
поручениями и `ai-docs/chat-coordination.md`; не создавать heartbeat и пустые ACK loops.
