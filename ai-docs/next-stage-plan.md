# Текущая точка: M1.2.51 завершён, следующий M1.2.52

Дата: 2026-10-22.

M1.2.51 объединил controller, pending handoff, producer и local-UX facade в
одном dormant process-owned host. Public API больше не переносит handoff или
start command. Однако crate-private installation пока принимает уже собранный
`AuthenticatedGuidedRecipientImportSession`; его реальный единственный producer
и ownership boundary ещё не оформлены.

## Следующий этап: M1.2.52 — authenticated host-installation owner

1. Добавить одного non-cloneable process-local owner-а, который создаёт host и
   является единственной точкой установки authenticated recovery authority.
2. Owner должен принимать только полный результат существующей canonical
   recovery assembly: signed Plan, exact coordinator и уже загруженный ordered
   membership set; public loose-fragment constructor запрещён.
3. Установка должна быть one-shot: authority извлекается до fallible host
   install, а success/failure/replay/replacement не возвращают reusable grant.
4. Public local UX остаётся только на host; owner не выдаёт authority, handoff,
   start command, controller или membership snapshots.
5. Оставить owner dormant: без runtime IPC/CLI/desktop/timer/background/network
   entry point; ordinary startup не создаёт owner, default `legacy-only`.

## Stop conditions

- owner получает path/signer/descriptor/coordinator/membership отдельными
  public аргументами или восстанавливает их из ambient filesystem discovery;
- install grant можно clone/serialize/replay или вернуть вызывающему при ошибке;
- owner экспортирует host internals либо второй UX control surface;
- restart восстанавливает ephemeral grant/pending/active authority;
- ordinary startup автоматически создаёт owner или host ceremony;
- peer/runtime wire, product network или public default меняется.
