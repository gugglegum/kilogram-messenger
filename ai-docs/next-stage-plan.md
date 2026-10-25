# Текущая точка: S5.1 storage core принят; поручен exact commit/push

Актуально на2026-10-26. Принят fresh runtime двух процессов, перезапуски и
offline-доставка: [итоговое review S4](tasks/long-lived-dialog-a3-runtime-s4-implementation-review-5.md).
Полный P подтвердил1640 Text и точную историю после direct/own-relay restart;
F и выбранные component проверки пройдены. Это не завершение whole A3/MVP.

Пользователь поручил S5 existing L1 migration/handoff:
[задача на контракт](tasks/long-lived-dialog-a3-migration-s5-task.md), baseline
`0c1baa887008f784a86fd7ebabdcd5145fb4d1c0`.
[Review2](tasks/long-lived-dialog-a3-migration-s5-contract-review-2.md) принял
исправленный контракт: R1–R4 закрыты. Contract commit
`3547cf528f15c15a21cdb9e30b1d7ca35a01297a` опубликован; root независимо сверил
all9 accepted paths/blobs, sole parent и actual remote master.
[S5.1 review3](tasks/long-lived-dialog-a3-migration-s5-storage-implementation-review-3.md)
принимает production preflight, Root7/fence/copy/Ready, provenance/credits,
offline administration и focused integrity/version/kill checks. R1–R4 закрыты.
Исполнителю поручен один exact commit/push по accepted manifest, parent3547cf5;
root проверяет опубликованный diff/remote. После этого поручается S5.2 indexed L1
owner/transport/history/receipt continuation; existing handoff и whole P5 ещё впереди.
Текущая Ready — storage only. C279.3257298/600, F1236.2886628/1320 (явная поправка
review2 +120 только на R4), P0/2400. Для следующих live cuts нужен отдельный root
план проверок и бюджета; этот ledger не сбрасывать и не расходовать молча.
Каждая логическая часть сдаётся на review до commit/push; whole S5/P5 пока открыт.
Ниже сохранён отложенный recovery API-retirement план. Он не является текущим
приоритетом и не должен автоматически запускаться после S4.

## Отложенный план: M1.2.62 завершён, допустимый successor M1.2.63

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
