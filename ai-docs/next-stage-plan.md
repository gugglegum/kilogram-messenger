# Текущая точка: M1.2.61 завершён, следующий M1.2.62

Дата: 2026-10-22.

M1.2.61 удалил raw installed recovery host из public runtime API. Host теперь
crate-private, возвращающий его installation transition также crate-private, а
единственным production owner остаётся crate-private launch slot. Public typed
host commands/errors, safe UX view и launch-service facade сохранены без
semantic или wire change.

## Следующий этап: M1.2.62 — transitional installation-owner public surface retirement

1. Сделать `RecipientImportGuidedRecoveryHostInstallationOwner` crate-private
   и убрать его public runtime re-export. Installation owner должен жить только
   внутри crate-private launch-authorization/slot path.
2. Сузить predecessor `issue`/`approve` transitions настолько, насколько нужно
   для закрытия private return type, не раскрывая issuer, authorization или raw
   host и не меняя one-shot custody semantics.
3. Сохранить public launch-service facade, safe preview/state, typed launch и
   host commands/errors и safe local UX view.
4. Оставить predecessor owner/issuer/authorization fixtures доступными только
   внутри copied-module test boundary с crate-private raw authority re-exports.
5. Не добавлять runtime IPC/CLI/desktop/timer/background/network registration,
   не менять peer/runtime wire и public `legacy-only`.

## Stop conditions

- installation owner либо возвращающий его raw transition остаётся public;
- другой production module получает owner field, constructor, accessor,
  extraction/replacement seam или прямой install path;
- retirement ломает safe launch/host commands, service facade или one-shot
  custody/error semantics;
- predecessor tests становятся product API или обходят slot/service в active
  path;
- ordinary startup автоматически создаёт owner/host/service или запускает
  ceremony;
- peer/runtime wire, product network или public default меняется.
