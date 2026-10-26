# Текущая точка: S5.2 submission готовится к итоговой root-приёмке

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
Commit `73c499818ba9b699c331aa842e64e8432e90c59a` опубликован, sole parent3547cf5.
Root независимо проверил все230 accepted paths/blobs, raw hashes, clean tree/index
и actual GitHub master (identical, ahead0/behind0).
[S5.2 plan review2](tasks/long-lived-dialog-a3-migration-s5-l1-plan-review-2.md) принимает
mapping с исправлениями account scope, публичного Consent1 renewal и точного IPC3.
[Реализация](tasks/long-lived-dialog-a3-migration-s5-l1-implementation.md) indexed L1
owner/transport/history/receipt continuation на той же Native-сессии с одним реальным
exhausted origin A511/B16 готовится к root acceptance; [candidate allowlist](tasks/long-lived-dialog-a3-migration-s5-l1-candidate-files.json)
фиксирует точный состав. Root принял завершённые функциональные assertions boundary03
по связанному исходнику/фазам. Whole boundary03 остаётся FAIL649.592232s, parent-result
отсутствует; исторические whole FAIL/split outcomes не переименованы.
[Preservation принято отдельно](tasks/long-lived-dialog-a3-migration-s5-l1-preservation-review.md):
C whole PASS24.1463993s, F preservation-only whole PASS67.5439277s,6956 файлов
остановленного checkpoint сохранены под обоими OS locks; backup не активировался.
S5.2 actual C172.0310185/F3331.2897354/P0, потолки500/3650/0;39 contributing cases,
четыре явные поправки F без возврата расходов. Опубликованный S5.1 actual
C279.3257298/F1236.2886628/P0 сохраняется; combined actual C451.3567483/F4567.5783982/P0,
combined maxima C779.3257298/F4886.2886628/P0. Поздние estimates не являются бюджетом.
Полная реализация S5.2 ещё не принята, commit/push не поручены.
Handoff/whole P5 ещё впереди, whole P требует отдельного непрерывного run<=2400.
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
