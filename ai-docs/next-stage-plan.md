# Текущая точка: M1.2.49 завершён, следующий M1.2.50

Дата: 2026-10-21.

M1.2.49 добавил dormant process-local UX facade поверх M1.2.48 controller.
UI получает только безопасные phase/count/deadline/confirmation данные.
Stale и double submit отсекаются до durable work; потерянный ответ
восстанавливается read-only refresh. Однако one-shot start command пока
создаётся только crate-private test helper-ом, а production owner отсутствует.

## Следующий этап: M1.2.50 — authenticated one-shot session-start producer

1. Выделить существующему in-process authenticated recovery ceremony owner
   sealed hand-off, который уже содержит exact approved source set,
   coordinator, Plan request и live memberships; loose fragments не принимать.
2. Producer должен выдать ровно один non-cloneable/non-serializable
   `LocalUxStart`, после чего исходный hand-off считается consumed в памяти.
3. До выдачи повторно проверить state/Account/Device, consent/source binding,
   coordinator sequence, membership digests и lifetime максимум 300 секунд.
4. Cancel, expiry, mismatch, failed production и process restart не должны
   оставлять start authority. Replay того же hand-off не создаёт вторую session.
5. Оставить producer dormant: без IPC/CLI/desktop registration, filesystem
   discovery, background start или network; public default — `legacy-only`.

## Stop conditions

- producer принимает path, signer secret, coordinator или membership snapshot
  отдельными public аргументами;
- hand-off можно clone/serialize/debug или восстановить из durable ledger;
- один hand-off создаёт более одного start command;
- stale consent/source/coordinator/membership всё равно получает authority;
- ordinary startup автоматически создаёт session;
- peer/runtime wire, product network или public default меняется.
