# Текущая точка: M1.2.47 завершён, следующий M1.2.48

Дата: 2026-10-21.

M1.2.47 связал existing guided import mutation с M1.2.46 actor через private,
non-serializable process-local completion hand-off. Только fresh successful
complete dispatch получает hand-off; final coordinator и exact live membership
set проверяются canonical реализациями. Raw context constructor crate-private,
actor повторно проверяет state/Account/Device. Loopback Plan/Commit/replay/
detach проходит, но ordinary runtime startup по-прежнему unattached.

## Следующий этап: M1.2.48 — runtime-owned explicit guided session controller

1. Собрать существующие read-only Plan, Device confirmation, bounded mutation,
   M1.2.47 hand-off и M1.2.46 attachment в один process-owned controller с
   явными фазами и максимум одной активной guided session.
2. Controller принимает только уже загруженный authenticated recovery context
   от будущего local UX owner; он не принимает user paths, serialized
   membership snapshots, secrets или произвольные coordinator objects.
3. Cancel, expiry, failed confirmation/mutation, Account/Device/state change и
   process restart обязаны оставить actor unattached и очистить ephemeral stage.
4. Успешный happy path должен прикреплять context один раз и передавать
   дальнейший explicit consumption существующему v34 Plan/Commit handler без
   повторного import/consume.
5. Не регистрировать новый IPC command, CLI/desktop control, timer или network;
   ordinary startup остаётся idle/unattached, public default — `legacy-only`.

## Stop conditions

- controller сам discover-ит recovery sources либо читает caller path;
- partial/failed/cancelled session оставляет hand-off/context/Plan stage;
- restart восстанавливает ephemeral authority из durable ledgers автоматически;
- controller принимает membership/coordinator по IPC, env или directory scan;
- import или consume повторяется после ambiguous response;
- ordinary runtime startup автоматически начинает ceremony;
- peer/runtime wire, product network или public default меняется.
