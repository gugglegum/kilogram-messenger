# Текущая точка: M1.2.59 завершён, следующий M1.2.60

Дата: 2026-10-22.

M1.2.59 поместил полный guided-recovery launch lifecycle под один dormant
process-owned service facade. Service владеет private slot, принимает opaque
authenticated assembly по значению и маршрутизирует safe state/preview,
typed approve/cancel и existing host commands. Raw slot, host, issuer,
authorization и authority fragments через service не выдаются.

## Следующий этап: M1.2.60 — raw launch-slot public surface retirement

1. Сделать сам `RecipientImportGuidedRecoveryLaunchSlot` crate-private и убрать
   его из public re-export runtime crate. Production callers должны видеть
   только M1.2.59 service; raw slot нельзя назвать, сохранить или использовать
   как параллельный owner.
2. Сохранить public safe vocabulary, нужный facade: typed approve/cancel,
   state и typed error. Не менять их semantics или wire representation.
3. Оставить predecessor slot fixtures доступными только внутри copied-module
   test boundary, чтобы доказать неизменность atomic staging, one-shot approve,
   cancellation и host routing без повторного открытия production API.
4. Проверить, что только service production source владеет/создаёт raw slot,
   а CLI/desktop/runtime startup/IPC/env/filesystem не получают constructor,
   accessor или alternate custody seam.
5. Не добавлять runtime IPC/CLI/desktop/timer/background/network registration,
   не менять peer/runtime wire и public `legacy-only`.

## Stop conditions

- raw launch-slot type или constructor остаётся public/re-exported;
- другой production module получает slot field, constructor или raw accessor;
- retirement ломает safe command/state/error facade либо меняет one-shot
  custody semantics;
- predecessor tests становятся product API или обходят service в active path;
- ordinary startup автоматически создаёт service/slot или запускает ceremony;
- peer/runtime wire, product network или public default меняется.
