# Текущая точка: M1.2.57 завершён, следующий M1.2.58

Дата: 2026-10-22.

M1.2.57 убрал передачу готовой launch authorization между crate-internal
владельцами. Process-owned slot принимает one-shot issuer по значению, создаёт
grant внутри себя, валидирует до retention и при любом отказе не возвращает
authority и не вытесняет существующую pending/installed custody. Production
CLI/desktop/runtime IPC/network всё ещё не имеют entry point.

## Следующий этап: M1.2.58 — atomic authenticated-assembly-to-slot staging

1. Ввести один opaque process-local staging input, который может быть создан
   только из exact authenticated signed Plan/coordinator/ordered-membership
   assembly и не раскрывает loose authority fragments.
2. Добавить единственный crate-internal переход, который принимает этот input
   по значению, создаёт M1.2.53 issuer и сразу передаёт его в M1.2.57 slot;
   вызывающий код не должен временно владеть готовым issuer.
3. Удалить или сузить прежний caller-facing issuer-construction seam. Assembly,
   issuer и grant должны поглощаться на failure без return/replay/replacement;
   occupied slot не меняет прежнюю custody.
4. Возвращать только safe slot state/preview. Assembly input, issuer,
   authorization, owner, host, authenticated session, handoff, identity и
   membership наружу не выдавать.
5. Оставить путь dormant: без runtime IPC/CLI/desktop/timer/background/network
   registration; ordinary startup slot не создаёт, default остаётся
   `legacy-only`.

## Stop conditions

- готовый issuer передаётся между production modules или возвращается
  вызывающему коду;
- public/caller-facing API принимает loose path/identity/signer/descriptor/
  coordinator/membership fragments для сборки authority;
- failed staging возвращает assembly/issuer/grant либо вытесняет
  pending/installed custody;
- slot/input/issuer можно clone/serialize/восстановить после restart;
- ordinary startup автоматически создаёт slot или запускает ceremony;
- peer/runtime wire, product network или public default меняется.
