# A3-S2: уточнение Cargo.lock

2026-10-24. Принято root в рамках разрешённой реализации S2.
Baseline: `61524df3a77203d73673f47d38decf6b817615e3`.

В [integration review2](long-lived-dialog-a3-history-s2-integration-review-2.md)
разрешена direct workspace dependency base64, но фраза «без обновления версии/lock»
неточна: новая связь зависимости должна появиться в Cargo.lock. Это уточнение
исправляет только запрет такого служебного изменения; версии не обновляются.

Root проверил Cargo.toml (`base64 = "0.22"`), WIP manifest kilogram-state и lock:
package base64 0.22.1 уже присутствует, но связи из kilogram-state ещё нет.
Другой существующий base64 0.23.1 не затрагивается. Первичный locked failure
сохранён в [check-initial.log](long-lived-dialog-a3-history-s2-evidence/check-initial.log).
Тесты этого уточнения root не запускал; это проверка точного требуемого diff.

Разрешённое изменение Cargo.lock: только строка ` "base64 0.22.1",` в dependencies
package kilogram-state, после argon2. Допустима точная правка этой строки без
unlocked Cargo; затем обычный check с `--locked --offline` должен подтвердить lock.
Если используется штатное offline обновление lock metadata, результат обязан
содержать тот же единственный diff. Cargo update и обновление package resolution
не требуются. Любой дополнительный lock diff — основание остановить этот шаг и
сообщить конкретное расхождение, не принимать его автоматически.

Все package name/version/source/checksum, состав package entries и остальные
dependency edges остаются прежними. Сверить exact diff и эти поля до/после;
включить результат в S2 implementation evidence. Последующие checks/tests остаются
`--locked --offline`; Windows harnesses запускаются только stable runner.

Принятые contract/review/manifests не переписываются. Это уточнение и служебная
lock edge входят в будущий S2 implementation diff, который ещё подлежит code
review. Отдельный product commit/push до приёмки реализации не поручается.
