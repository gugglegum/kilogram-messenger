# Текущая точка: M1.2.48 завершён, следующий M1.2.49

Дата: 2026-10-21.

M1.2.48 собрал полный explicit guided import/consumption lifecycle в один
process-owned controller. Единственная short-lived session проходит
`Idle -> Planned -> Confirmed -> Attached`; cancel, expiry, failed confirm/apply
и restart не оставляют ephemeral authority. Контроллер пока dormant: ordinary
runtime не создаёт token и не регистрирует CLI/desktop/IPC/network entry point.

## Следующий этап: M1.2.49 — dormant local-UX guided-session adapter

1. Добавить process-local adapter, который получает M1.2.48 token только от
   существующего authenticated recovery ceremony owner и не умеет собирать его
   из path, IPC payload, environment или serialized fragments.
2. Выдать local UX безопасную view-модель Plan/confirmation/progress: только
   понятные пользователю counts, deadline, confirmation phrase и phase; без
   signer secret, memberships, coordinator object, state path или opaque
   authority token.
3. Команды UX должны быть типизированы как explicit start/confirm/apply/cancel
   с exact session ID; stale view, double-submit и смена Account/Device/state
   обязаны fail closed.
4. Потерянный UI response должен восстанавливаться только чтением controller
   phase/result в том же процессе, без повторной mutation или consumption.
5. Оставить adapter dormant: не добавлять runtime IPC/CLI/desktop registration,
   timer, background start или network. Public default остаётся `legacy-only`.

## Stop conditions

- view-модель или command раскрывает token, path, signer, membership snapshot
  либо coordinator authority;
- UI может миновать explicit confirmation или вызвать apply до Confirmed;
- retry повторяет durable mutation/consumption вместо чтения exact phase;
- restart восстанавливает ephemeral session из durable ledgers;
- ordinary startup автоматически создаёт ceremony или attachment;
- peer/runtime wire, product network или public default меняется.
