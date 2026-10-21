# Текущая точка: M1.2.52 завершён, следующий M1.2.53

Дата: 2026-10-22.

M1.2.52 сделал one-shot process-local owner единственной installation point
для M1.2.51 host. Owner принимает только opaque full assembly, а host
constructor/install стали private. Однако сам
`AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery` пока
остаётся crate-private loose-fragment assembly constructor; единственный
upstream producer полного assembly ещё не оформлен отдельной границей.

## Следующий этап: M1.2.53 — authenticated full-assembly issuer

1. Добавить один non-cloneable process-local issuer полного authenticated
   recovery assembly поверх существующего canonical signed Plan результата,
   exact coordinator и already-loaded ordered memberships.
2. Перенести loose-fragment assembly construction за private issuer boundary;
   общий crate-private caller не должен собирать session напрямую.
3. Issuer должен потреблять one-shot authenticated input до fallible assembly
   validation и выдавать максимум один M1.2.52 installation owner, не сам
   session, host, handoff или отдельные authority fragments.
4. Повторно проверить exact state/Account/Device/signer, signed Plan,
   coordinator plan+sequence, ordered references и membership digests на
   переходе issuer → owner.
5. Оставить issuer dormant: без runtime IPC/CLI/desktop/timer/background/
   network entry point; ordinary startup не создаёт issuer, default
   `legacy-only`.

## Stop conditions

- issuer принимает loose fragments через public API или ищет их в ambient
  filesystem;
- прежний loose assembly constructor остаётся callable вне issuer/tests;
- issuer возвращает raw session, host, handoff, controller или memberships;
- issuer/grant можно clone/serialize/replay или восстановить после restart;
- ordinary startup автоматически создаёт issuer/owner/host ceremony;
- peer/runtime wire, product network или public default меняется.
