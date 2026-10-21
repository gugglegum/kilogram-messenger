# Текущая точка: M1.2.55 завершён, следующий M1.2.56

Дата: 2026-10-22.

M1.2.55 добавил один non-cloneable process-owned launch slot. Он принимает по
значению максимум одну complete M1.2.54 authorization, запрещает replacement и
после exact approval сохраняет ровно один opaque host внутри себя. Cancel,
expiry, mismatch, clock regression и downstream failure очищают pending
custody. Production CLI/desktop/runtime IPC/network всё ещё не имеют entry
point.

## Следующий этап: M1.2.56 — opaque installed-host command router

1. Маршрутизировать существующие typed host prepare/confirm/apply/cancel/
   refresh команды через M1.2.55 slot, не возвращая и не заимствуя raw host.
2. Все host-команды должны требовать состояние `HostInstalled`; `Empty` и
   `PendingAuthorization` отвергаются до host/controller/durable work.
3. Возвращать только существующий safe M1.2.49 UX view либо узкую safe state
   projection. Path, Account/Device, signer, descriptor, coordinator,
   membership, issuer, authorization, owner, host, session и handoff наружу не
   выдавать.
4. Ошибки routed-команд не должны открывать replacement seam. Fail-closed
   очистка внутри host/controller сохраняется, сам installed host остаётся
   process-owned и не может быть извлечён, заменён или восстановлен после
   restart.
5. Оставить путь dormant: без runtime IPC/CLI/desktop/timer/background/network
   registration; ordinary startup slot не создаёт, default остаётся
   `legacy-only`.

## Stop conditions

- любой public API возвращает/заимствует raw host, installation owner,
  authorization, issuer, session, handoff или identity material;
- host-команда может выполняться в Empty/Pending состоянии;
- routed error позволяет заменить или извлечь installed host;
- slot/host можно clone/serialize/восстановить после restart;
- ordinary startup автоматически создаёт slot или запускает ceremony;
- peer/runtime wire, product network или public default меняется.
