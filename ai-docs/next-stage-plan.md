# Текущая точка: M1.2.60 завершён, следующий M1.2.61

Дата: 2026-10-22.

M1.2.60 удалил raw launch slot из public runtime API. Slot теперь crate-private,
единственным production owner и constructor user остаётся M1.2.59 service, а
copied-module fixtures видят slot только внутри test crate. Public safe
state/approve/cancel/error и facade сохранены без semantic или wire change.

## Следующий этап: M1.2.61 — raw installed-host public surface retirement

1. Сделать `RecipientImportGuidedRecoveryHost` crate-private и убрать raw host
   из public runtime re-export. Installed host должен называться и храниться
   только внутри crate-private slot implementation.
2. Сохранить public typed host commands, typed host/service errors и safe local
   UX view, используемые launch service facade. Не менять их semantics или wire
   representation.
3. Оставить predecessor host/installation-owner fixtures доступными только
   внутри copied-module test boundary с crate-private raw host re-export.
4. Проверить отсутствие второго production owner, public host accessor,
   extraction/replacement seam и прямого host construction из product surface.
5. Не добавлять runtime IPC/CLI/desktop/timer/background/network registration,
   не менять peer/runtime wire и public `legacy-only`.

## Stop conditions

- raw host type или constructor остаётся public/re-exported;
- другой production module получает host field, constructor или raw accessor;
- retirement ломает typed host commands/safe UX/service facade либо меняет
  one-shot custody semantics;
- predecessor tests становятся product API или обходят slot/service в active
  path;
- ordinary startup автоматически создаёт host/service или запускает ceremony;
- peer/runtime wire, product network или public default меняется.
