# Текущая точка: M1.2.58 завершён, следующий M1.2.59

Дата: 2026-10-22.

M1.2.58 удалил временное caller ownership готового assembly issuer. Complete
authenticated signed Plan/coordinator/ordered-membership assembly существует
как opaque process-local staging input; process-owned slot принимает его по
значению, создаёт issuer и authorization внутри одной transition и сохраняет
только прошедший admission grant. Production CLI/desktop/runtime IPC/network
по-прежнему не имеют entry point.

## Следующий этап: M1.2.59 — dormant process-owned guided-recovery launch service

1. Ввести один non-cloneable process-owned service, который сам владеет ровно
   одним M1.2.58 launch slot; raw slot, host, issuer, authorization и authority
   fragments не должны покидать service.
2. Провести opaque assembly staging, safe preview/state, typed approve/cancel и
   existing host commands через один facade. Возвращать только уже определённые
   safe state/preview/UX projections и typed errors.
3. Запретить replacement, extraction, arbitrary second slot, serialization и
   restart recovery. Fallible calls должны сохранять или уничтожать custody
   строго по существующим one-shot правилам predecessor layers.
4. Оставить construction crate-private и dormant: ordinary runtime startup не
   создаёт service и не получает authority из IPC/env/filesystem discovery.
5. Не добавлять runtime IPC/CLI/desktop/timer/background/network registration,
   не менять peer/runtime wire и public `legacy-only`.

## Stop conditions

- caller получает raw slot/host/issuer/authorization либо может создать второй
  service/slot через public API;
- facade принимает loose path/identity/signer/descriptor/coordinator/membership
  вместо opaque M1.2.58 input;
- failure возвращает authority, silently replaces/evicts custody или допускает
  повторную команду после one-shot consumption;
- service/slot можно clone/serialize/восстановить после restart;
- ordinary startup автоматически создаёт service или запускает ceremony;
- peer/runtime wire, product network или public default меняется.
