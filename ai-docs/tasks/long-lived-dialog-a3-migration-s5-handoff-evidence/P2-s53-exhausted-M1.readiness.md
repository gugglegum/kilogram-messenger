# M1 prerequisite candidate

2026-11-01, Asia/Yekaterinburg. Только публичные evidence/source; новых private
O/S/BKP/R accesses, signing, runtime, Cargo или правок исходников нет.
[M1 candidate](P2-s53-exhausted-M1.candidate.json) ограничен 64KiB и ожидает root
review. Он не активирует runtime и не объявляет себя root-approved. M1 не
коммитился. Принятый Cold-B02 evidence опубликован отдельно: `fa0867181de096458cbd9eb79c65b71a84fef4f5`.

| Поле M1 | Принятое evidence и точный смысл |
| --- | --- |
| `context`, `nodes[].role/producer_side/account/device` | [Origin](../long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-origin-01-result.json), [checkpoint](../long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-boundary-03-checkpoint-info.json), [Cold-B02 acceptance](Cold-B-attempt02-root-accepted.md): original A=two, B=one, без нового enrollment. |
| `root_decision_sha256` | Существующий [D1](P2-s53-exhausted-roots-01-attempt-01.D1.json), SHA `17cb30232565f8d59c7467c9512a519cbfeac4a49ee6759951f4f13f2af06f72`. Отдельные ссылки на уже завершённые activation/acceptance Cold-B02. Нет ссылок на будущие D2/A1 или собственный hash. |
| `source_preservation_sha256` | SHA всего завершённого публичного [Cold-B02 aggregate](P2-s53-exhausted-cold-b-01-attempt-02.aggregate.json), включая исторический BKP и раздельные entry/final S observations; не утверждение о raw S==BKP. |
| `copy_inventory_sha256` | SHA публичного [RootCopies P2 report](P2-s53-exhausted-roots-01-attempt-01.p2.json): O accounts→R до typed load, 9 файлов/6 каталогов/768B, copy equality и O preservation. Защищённый copy-before-load receipt указан отдельным digest. Это не hash опубликованного private inventory и не S restore. |
| `historical_preservation` | [Preservation report](../long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-preservation-01-result.json) и [P1](P1-s53-exhausted-feasibility-01-attempt-01.p1-observations.json): private inventory `c2a56327…28b3`, 6,765,525B, 6,956 файлов. Исторический boundary03 whole FAIL не переименован. |
| `nodes[].base/profile/exact_state_path/own_root`, profile/config pins | [P1](P1-s53-exhausted-feasibility-01-attempt-01.p1-observations.json), prior D1 и Cold-B02 exact-path preflight до typed open. S/two и S/one; R/two.account и R/one.account. Profile1153B/config525B каждого узла; непрерывность non-DB bytes — strict historical audit и повторный B0. |
| `nodes[].authority`, `current_own_roots_available` | [r41 acceptance](S5.3-r41-rootcopies.root-review.json): обе оригинальные R roots usable/already-current Windows DPAPI; signed snapshots revision1, revocations0,100B. Cold-B02 проверил candidate≥stored floor, exact bytes при равной revision и original certificate. Это ограниченные observations, не бессрочные права. |
| `nodes[].native/logical/witnesses` | Cold-B02 revalidated Native517/88 и исходные hashes; next[512,18], receipts[511,17], sources530, queue/debt/pending0, L1/Q-absent. Полные 528 Text/17 pages, включая первую страницу, signatures/AuthRecords/Receipt1/local requests, повторный B1 и unchanged B0. |
| `nodes[].logical.own_future_request` | Accepted full reader: A600 absent в own A index, B601 absent в own B index; оба ID отсутствуют во всех original signed sources/history. Peer local indices не выдумываются — уточнение [r33](../long-lived-dialog-a3-migration-s5-handoff-r32-acceptance-and-p1-source-task-r33.md). |
| `current_physical`, `cleanup` | Cold-B02 historical/BKP tuple `7099e525…b3b5`, entry `4b703f74…7efe`, final `7fbde6c6…f8f1`, повторные digests совпали. Final/history=false, final/entry отличаются, 2 changed slots. 8 hash passes плюс 40,181,760 normalization bytes; handles0, known cleanup, actual leases released. |
| `single_active_copy_per_native_domain` | Наблюдение двух actual S leases, удержанных до финального аудита, и inactive opaque BKP. Это named-path scope, не глобальный поиск/доказательство отсутствия иных копий. Для будущего runtime требуется снова установить отсутствие второго owner. |
| `temporal_limits`, `public_value_gaps` | Genuine Consent2 ещё не создавался: повторно проверить current snapshot/floor, membership, permissions, revocations, expiry и реальное время. Locator import не доказывает live family2 admission. |

Каждый evidence/source reference в JSON имеет path/bytes/SHA256. D/E и некоторые
profile literals (claim/source_id/migration_id/config_revision/quotas) не
экспортированы отдельными публичными значениями: M1 честно оставляет их
неизвестными, ссылаясь на проверенный signed scope, whole-profile/config pins и
accepted stored-binding checks. Если потребителю понадобятся именно эти
литералы, существующих bounded public exports для их заполнения недостаточно;
нового private чтения здесь не выполнялось.

Snapshot observations one-active-copy и usable/current roots представлены
фактами с областью действия, а не универсальными booleans старого controller.
Этот candidate не передавался в старый boolean guard. Отдельные root review и
позднейшая activation остаются обязательными; уже согласованные case-local
исправления controller/adapter этим этапом не реализуются.
