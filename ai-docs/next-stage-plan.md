# Текущая точка: M1.2.50 завершён, следующий M1.2.51

Дата: 2026-10-21.

M1.2.50 закрыл прямое создание UX start-команды: sealed authenticated handoff
поглощается one-shot producer-ом до fallible preflight. Однако будущему
process owner пока пришлось бы отдельно удерживать controller, pending handoff,
producer и UX facade, что оставляет слишком широкую orchestration surface.

## Следующий этап: M1.2.51 — dormant process-owned guided-recovery host

1. Добавить одного process-local host, владеющего M1.2.48 controller и максимум
   одним pending M1.2.50 handoff; authority и `LocalUxStart` наружу не выдавать.
2. Crate-private authenticated owner сможет установить handoff только в пустой
   host. Replacement, replay, foreign binding и second pending authority должны
   fail closed без вытеснения живой ceremony.
3. Public local UX surface host-а должна принимать только безопасные typed
   prepare/confirm/apply/cancel/refresh команды и возвращать M1.2.49 view.
4. Prepare атомарно поглощает pending handoff через M1.2.50 producer; failure,
   cancel, expiry и process restart очищают всю ephemeral custody.
5. Оставить host dormant: не регистрировать runtime IPC/CLI/desktop/timer/
   background/network entry point; public default остаётся `legacy-only`.

## Stop conditions

- host экспортирует handoff, start command, path, signer, coordinator либо
  membership snapshot;
- pending authority можно заменить или сериализовать;
- UI получает способ миновать one-shot producer или explicit confirmation;
- restart восстанавливает pending/active authority из durable state;
- ordinary startup автоматически создаёт host ceremony;
- peer/runtime wire, product network или public default меняется.
