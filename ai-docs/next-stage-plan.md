# Текущая точка: M1.2.53 завершён, следующий M1.2.54

Дата: 2026-10-22.

M1.2.53 запечатал one-shot full-assembly issuer. Он владеет exact state,
Account, Device, signer, signed Plan request, coordinator и ordered loaded
memberships, поглощает input до fallible validation и выдаёт только M1.2.52
installation owner. Loose session constructor private; production
CLI/desktop/runtime IPC/network всё ещё не имеют entry point.

## Следующий этап: M1.2.54 — explicit local launch authorization

1. Добавить один non-cloneable process-local launch authorization, который
   удерживает ровно один M1.2.53 issuer и короткоживущую безопасную preview для
   явного локального решения пользователя.
2. Не выдавать issuer наружу: typed approve должен потреблять authorization до
   fallible work и возвращать максимум один installation owner; cancel,
   expiry, mismatch и любой failure должны поглощать authorization без reusable
   grant.
3. Preview не должна содержать path, Account/Device IDs, descriptor, signer,
   coordinator, membership snapshots, request/nonce или иной authority. Для
   подтверждения использовать отдельную domain-bound phrase/ceremony ID.
4. Повторно связать authorization с exact issuer/session lifetime и запретить
   replacement уже pending/active ceremony.
5. Оставить весь путь dormant: без runtime IPC/CLI/desktop/timer/background/
   network registration; ordinary startup не создаёт authorization, default
   `legacy-only`.

## Stop conditions

- authorization имеет public или loose-fragment constructor;
- approve принимает issuer/path/signer/descriptor/coordinator/membership как
  отдельные аргументы либо возвращает issuer/raw session/host/handoff;
- cancel/expiry/failure возвращает reusable authority или допускает replay;
- preview раскрывает identity/authority material;
- authorization можно clone/serialize/восстановить после restart;
- ordinary startup автоматически создаёт/подтверждает ceremony;
- peer/runtime wire, product network или public default меняется.
