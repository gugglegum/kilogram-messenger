# Текущая точка: M1.2.62 завершён, следующий M1.2.63

Дата: 2026-10-22.

Статус передачи на 2026-10-23: M1.2.63 не начат, частичного implementation
change-set нет. Этот план сохранён как узкий допустимый successor, но не должен
запускаться автоматически до выбора приоритета по
[`project-handoff-2026-10-23.md`](project-handoff-2026-10-23.md).

M1.2.62 удалил transitional host installation owner из public runtime API.
Owner и возвращающие его `issue`/`approve`/`install` transitions теперь
crate-private и доступны только one-shot launch-slot path. Public safe launch
service, preview/state, typed commands/errors и UX view сохранены без semantic
или wire change.

## Следующий этап: M1.2.63 — transitional assembly-issuer public surface retirement

1. Сделать `RecipientImportGuidedRecoveryAssemblyIssuer` crate-private и убрать
   его public runtime re-export. Issuer должен создаваться только внутри
   crate-private authenticated-assembly-to-slot staging path.
2. Сохранить private full-assembly input, launch binding и consuming `issue`
   transition без constructor/accessor/extraction/replacement seam.
3. Сохранить public launch-service facade, safe preview/state, typed launch и
   host commands/errors и safe local UX view.
4. Оставить predecessor issuer fixtures доступными только внутри copied-module
   test boundary с crate-private raw-authority re-export.
5. Не добавлять runtime IPC/CLI/desktop/timer/background/network registration,
   не менять peer/runtime wire и public `legacy-only`.

## Stop conditions

- assembly issuer либо его public runtime re-export остаётся externally
  nameable;
- другой production module получает issuer field, constructor, accessor,
  extraction/replacement seam или caller-owned ready issuer;
- retirement ломает atomic assembly-to-slot staging, safe launch/host commands,
  service facade или one-shot custody/error semantics;
- predecessor tests становятся product API или обходят slot/service в active
  path;
- ordinary startup автоматически создаёт issuer/authorization/owner/host или
  запускает ceremony;
- peer/runtime wire, product network или public default меняется.
