# A1 F1/F2 — результат независимого review

2026-10-23. Implementation HEAD: `3e82e6380c30d0f9443b6692ea34faccaacddef7`.
Вход: [задание](lossless-sync-a1-review.md), [spike](lossless-sync-a1-spike/README.md),
[proposal §5](lossless-sync-a1-proposal.md#5-f1f2-feasibility-spike--уточнения-после-review).

Последующее решение пользователя: первый slice из последнего раздела одобрен
и отправлен на реализацию GPT-6.1 Sol Max; разрешены автоматическое review и
циклы исправлений в его пределах. [Текущее состояние координации](../chat-coordination.md).
Фразы ниже «не запущена/не отправлена» сохраняют состояние на момент самого review.

## Решение

**F1/F2 принимается как завершённая проверка осуществимости. A1 остаётся открыт.**
Общий ordered ingress с сохранением pending и отдельным readable commit —
обоснованное направление. Полная миграция/новый wire/default rollout не утверждены.
Ещё один общий design milestone не нужен. Следующий полезный шаг — ограниченная
интеграция с реальным vault/runtime с условиями ниже; в этом ходе она не запущена.

Исполнитель честно отделил реальные Olm/signature проверки от trusted handshake
fixture и арифметической F2 модели. 512 exact plaintexts, DH transitions,
persist/reload и 14 конкретных immediate rejection/replay случаев воспроизведены.
Legacy unknown tail и hash-page starvation действительно требуют явной блокировки
или нового протокольного механизма. Их нельзя выдавать за успешную legacy migration.

Найдены три ограничения прототипа. Они не требуют повторять весь spike, но
ограничивают перенос его кода/гарантий в production и задают обязательные regressions.

## S1. Invalid после Pending требует отдельного результата и состояния

[Future ordinal](lossless-sync-a1-spike/src/f1.rs#L278) принимается в pending до
проверки encoding/MAC/inner binding. Это ожидаемо для части проверок, которым
нужен predecessor. Однако [accept/drain](lossless-sync-a1-spike/src/f1.rs#L218)
сначала коммитит корректный predecessor, затем возвращает `Err("invalid pending
fixture")` на испорченном pending. Уже сохранённый корректный prefix не откатывается,
невалидный successor остаётся в очереди; повтор predecessor возвращает Replay
без drain. Другой объект в том же slot отклоняется как fork.

Контрпример с реальной подписью отправителя и испорченным MAC переживает reload
и воспроизведён в [review-probes.log](lossless-sync-a1-spike/evidence/review-probes.log).
Это требует ключа авторизованного отправителя; не утверждается возможность
постороннего участника подделать сообщение. Все 14 исходных negative checks
проверяют текущий/старый slot, а не переход `Pending → reload → invalid head`.

Для интеграции: явно разделить результат отдельного события и результат drain/round.
Корректный committed prefix может сохраниться при ошибке round. Bad pending получает
durable blocked/quarantine reason, не ACK; retry не должен превращаться в busy loop.
Не удалять/не заменять молча ранее принятые bytes, не пропускать ordinal и не
продвигать ratchet ради обхода ошибки. Политика разрешения авторского fork отдельно
от признания correct predecessor; универсального rollback всей `accept` здесь нет.
Требование «Invalid не меняет crypto/readable state» относится к отклоняемому
событию, а не отменяет успешные commits других событий этого batch.

## S2. Ordinal не заменяет точный EventId

[F2 stage](lossless-sync-a1-spike/src/f2.rs#L132) считает duplicate любой объект
с уже известным `n`; possession/completion также содержат `n`. При stored
`{n:3, hash:3}` новый `{n:3, hash:99}` получает completion `[3]`, хотя сохранён
первый объект. Контрпример воспроизведён; это ограничение fake model, не найденная
ошибка существующего production SyncComplete. F1 отдельно проверяет fork.

В реальной интеграции нужны разные типы/значения EventId и lane ordinal.
Duplicate означает тот же immutable signed event; success SyncComplete должен
содержать ровно фактические IDs всего batch. Другой EventId с тем же ordinal —
конфликт, а не доказательство possession. Partial round сохраняет subset в
durable inventory, но не выдаёт subset SyncComplete.

## S3. Full lease текущего head не доказывает reserve следующего

[Stage](lossless-sync-a1-spike/src/f2.rs#L150) учитывает reserve, но
[drain](lossless-sync-a1-spike/src/f2.rs#L175) меняет head, а invariant не включает
новый head reserve. При cap=150, pending `[3,4,5]` и приёме `0`, затем `1`,
committed+leases=138. Head `2` требует ещё 30 и блокируется, хотя итоговая history
всех шести занимает 144. Этот конкретный trace также воспроизведён.

Консервативный StorageBlocked безопасен, если не потеряны bytes и нет ложного ACK.
Он опровергает обещание постоянно доступного head reserve, а не безопасность
отказа. Интеграция обязана учитывать transient commit/staging budget, не только
размер конечной history. Либо доказать сохранение moving reserve с учётом общего
budget, либо явно возвращать exhaustion после предыдущего commit. Нельзя заявлять
convergence лишь потому, что итоговая history формально помещается.

## Проверка и границы

- Независимая locked/offline сборка исходного spike через
  `scripts/invoke-cargo-friendly.ps1`, jobs=2/BelowNormal, exit 0.
- Исходный постоянный EXE запущен повторно, [review-run.log](lossless-sync-a1-spike/evidence/review-run.log),
  exit 0. SHA-256 `b3e4a5721db868955f92d833912a1c8f56f879c447c02cda95bf9d6c6f4d29c0`
  совпал с результатом исполнителя.
- Три дополнительных контрпримера: [reproducer](lossless-sync-a1-spike/evidence/review-probes.ps1),
  [build](lossless-sync-a1-spike/evidence/review-probes-build.log),
  [run](lossless-sync-a1-spike/evidence/review-probes.log), exit 0 означает, что
  ограничения воспроизведены. Скрипт копирует исходники в `.tmp/a1-review-probe`,
  добавляет assertions и использует отдельный постоянный
  `target/debug/kilogram-a1-review-probe.exe`. Submitted f1/f2 не изменены.
- Отдельные crypto/runtime reviewers проверили код и границы утверждений.
  Реальные vault crash windows, production epoch negotiation, network/GUI и
  физические устройства этим review не проверялись. Full workspace/mutation
  corpus не запускался. Product code и workspace dependencies не менялись.

## Следующий конечный шаг: реальный durable ingress, узкий внутренний scope

Предложенный в §5 первый slice пригоден с уточнениями. Не создавать отдельную
цепочку dormant facades: результат должен проходить через существующие runtime,
vault и history/projection API на синтетическом профиле.

1. **Изоляция state.** Новый disposable профиль, известный genesis, один выбранный
   Olm SessionId/recipient, single receiving chain. ConversationId недостаточно:
   текущий ratchet общий для peer device. Для защищённой session каждый разрешённый
   receive path проходит общий ingress; неподдержанные direct/mailbox/другая беседа
   и параллельный writer явно запрещены. Нельзя оставить обходной legacy decrypt.
   Не менять реальные пользовательские профили. Не обещать arbitrary migration.
2. **Реальная транзакция.** Durable pending содержит исходные event/auth bytes,
   отдельно от readable/causal state. Пробный decrypt, ratchet/frontier,
   LocalTextProjection, pending removal и ACK intent фиксируются согласованно
   существующим vault механизмом; сообщение ACK доступно только после readable
   commit. Ни test-only maps, ни regular-file reload не являются acceptance.
3. **Один положительный результат.** Два runtime/реальных session paths:
   256 небольших подписанных ciphertext по hash pages, restart после первой
   страницы, затем все 256 exact plaintext через history API, без MissingMessageKey,
   duplicate decrypt/ACK и потери ciphertext. Budget явно достаточен для этого
   сценария; не переносить гарантию на произвольные размеры/профили.
4. **Конечные отрицательные проверки в том же change-set.** Partial admission и
   restart inventory с точными EventId; тот же ordinal/другой EventId; bad future
   pending → reload → valid predecessor → явный blocked outcome; moving reserve
   trace выше; quota starvation/full history; unknown DH tail. Принятые ciphertext
   сохраняются, неподтверждённый readable commit не создаёт ACK/полного completion.
5. **Перезапуск на границах записи.** Проверить хотя бы после pending commit,
   после trial decrypt до readable transaction и после readable commit до ACK.
   После reopen — согласованный pre/post state, без double decrypt; использовать
   существующие fault seams, явно назвать непроверенные power-loss windows.
6. **Scope/ресурсы.** Новые domains, protocol activation/migration, GUI и default
   rollout не включать. Single-chain slice — интеграционный шаг, не готовая обычная
   двусторонняя переписка: DH transition пока блокируется. Targeted tests,
   cargo jobs=2/offline/locked; сетевые Windows harnesses только через стабильный
   runner. После одного reviewable diff и evidence остановиться на code review.

Это конкретное предложение следующей задачи, не свидетельство её отправки или
выполнения. Автоматическое уведомление F1/F2 завершилось этим review; новая
production-задача исполнителю в данном ходе не отправлена.
