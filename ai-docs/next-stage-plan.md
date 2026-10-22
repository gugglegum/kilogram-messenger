# Текущая точка: M1.2.56 завершён, следующий M1.2.57

Дата: 2026-10-22.

M1.2.56 провёл existing typed host prepare/confirm/apply/cancel/refresh через
process-owned launch slot. Один private gate требует `HostInstalled` и
отвергает Empty/Pending до host/controller/durable work. Наружу выходит только
privacy-safe UX view; routed error сохраняет host opaque и не открывает
replacement/extraction/restart seam. Production CLI/desktop/runtime
IPC/network всё ещё не имеют entry point.

## Следующий этап: M1.2.57 — atomic issuer-to-slot launch staging

1. Добавить единственный crate-internal переход, который принимает M1.2.53
   one-shot issuer по значению и создаёт M1.2.54 authorization непосредственно
   внутри пустого M1.2.55 slot.
2. Удалить или сузить прежний seam, принимающий уже готовую authorization:
   вызывающий код не должен временно владеть complete launch grant между
   issuer и slot.
3. Occupied-slot, invalid lifetime, clock regression, authorization failure и
   admission failure должны поглощать incoming issuer/grant без возврата,
   replay, replacement или eviction. Pending/installed custody не меняется при
   rejected replacement.
4. Возвращать только safe slot state/preview. Issuer, authorization, owner,
   host, authenticated session, handoff, identity и membership наружу не
   выдавать.
5. Оставить путь dormant: без runtime IPC/CLI/desktop/timer/background/network
   registration; ordinary startup slot не создаёт, default остаётся
   `legacy-only`.

## Stop conditions

- готовая launch authorization передаётся между production modules или
  возвращается вызывающему коду;
- failed staging возвращает issuer/grant либо вытесняет pending/installed
  custody;
- public API выдаёт raw issuer, authorization, owner, host, authenticated
  session, handoff или identity material;
- slot/authorization/issuer можно clone/serialize/восстановить после restart;
- ordinary startup автоматически создаёт slot или запускает ceremony;
- peer/runtime wire, product network или public default меняется.
