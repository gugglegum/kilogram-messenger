# Текущая точка: M1.2.54 завершён, следующий M1.2.55

Дата: 2026-10-22.

M1.2.54 поместил exact M1.2.53 issuer в short-lived one-shot local launch
authorization. Safe preview показывает только membership count, expiry и
domain-bound phrase; exact typed approve, cancel, expiry, mismatch, clock
regression и downstream failure всегда поглощают authorization. Production
CLI/desktop/runtime IPC/network всё ещё не имеют entry point.

## Следующий этап: M1.2.55 — process-owned pending launch slot

1. Добавить один non-cloneable process-owned slot, который может владеть не
   более чем одной M1.2.54 pending authorization и не восстанавливается после
   restart.
2. Install должен принимать только complete authorization, проверять expiry и
   отказывать, если pending authorization либо установленная host ceremony уже
   существуют; silent replacement и eviction запрещены.
3. Typed preview/approve/cancel должны идти через slot без выдачи issuer,
   authorization, installation owner, raw host/session/handoff или identity
   material наружу.
4. Approve должен извлекать pending authorization до fallible M1.2.54 work;
   success устанавливает ровно один host, а mismatch/expiry/failure очищают
   pending custody. Cancel также очищает её явно.
5. Оставить путь dormant: без runtime IPC/CLI/desktop/timer/background/network
   registration; ordinary startup создаёт максимум пустой slot либо вообще его
   не создаёт, default остаётся `legacy-only`.

## Stop conditions

- slot допускает больше одной pending authorization, replacement или eviction;
- public API принимает issuer/authorization/path/signer/descriptor/coordinator/
  membership либо возвращает installation owner/raw host/session/handoff;
- approve/cancel/expiry/failure сохраняет reusable pending authority;
- preview раскрывает identity/authority material или обходит M1.2.54 phrase;
- slot/authorization можно clone/serialize/восстановить после restart;
- ordinary startup автоматически устанавливает или подтверждает ceremony;
- peer/runtime wire, product network или public default меняется.
