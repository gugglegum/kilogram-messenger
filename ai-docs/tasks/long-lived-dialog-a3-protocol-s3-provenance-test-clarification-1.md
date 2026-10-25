# A3-S3: wrong-config provenance negative

2026-10-25, Asia/Yekaterinburg. Уточнение метода проверки к
[S3 plan](long-lived-dialog-a3-protocol-s3-plan.md) и
[implementation task](long-lived-dialog-a3-protocol-s3-implementation-task.md).

**Разрешён pure owner-provenance negative с явным неподдерживаемым config=2 fact.**
Это не испытание реального сохранённого config2 pickle и не смена crypto suite.
Production owner/importer по-прежнему вне S3 scope.

Root проверил pinned vodozemac0.10.0 `src/olm/session_config.rs`: public
SessionConfig::version_2() и enum variant V2 имеют cfg experimental-session-config.
Default features содержат libolm-compat, но не experimental-session-config.
Фактический [compile-10 FAIL](long-lived-dialog-a3-protocol-s3-evidence/compile-10.log)
подтверждает отсутствие version_2 в текущем build. SHA256 source session_config.rs:
`375ae882e0481a86d0e963c85b4421196438a3c797eaf561e015e320813838ad`;
SHA256 compile-10.log:
`80fc1e069904e7aa816b438ac19b6077029e144eddb0fc4c0ebf78d884eb2e2a`.
Cargo.toml/Cargo.lock/native adapter на момент проверки не изменены.

Условия принятого способа проверки:

1. Использовать вход настоящего test-only owner provenance verifier и generated L1
   fixture: один invalid origin/config fact проходит тот же verifier entrypoint, что
   positive config1. Нужен точный отказ UnsupportedOrigin/config до PrivateState::open,
   Guard2 import и durable effects, а не произвольный is_err от последующего parsing.
   Не добавлять фиктивный production configuration switch ради теста.
2. Показать контроль config1 на том же пути и наблюдаемые boundaries: при config2
   нет native restore/open, encrypt/decrypt/inbound transitions, import результата и
   writes. Guard/checkpoint/source bytes остаются прежними. Счётчик, который verifier
   принципиально не способен изменить, сам по себе не доказывает отсутствие calls.
3. Дополнительный public SessionConfig serde rejection для V2 допустим через уже
   имеющиеся serde/postcard средства. Он доказывает только отсутствие такого variant
   в текущем build. Не выдавать его за проверку чтения actual config2 SessionPickle.
   Не менять features/dependency graph и не редактировать private pickle representation.
4. Остальные категории negative и N03 остаются на generated L1 Head/AEAD/source/Seen/
   rotation fixture, а N03 продолжает настоящую прежнюю native session. Ясно различать
   восстановление checkpoint и mutating packet calls в отчёте.
5. Сохранить compile-10 FAIL и честно указать ограничение: actual config2-pickle
   regression не выполнен в этой конфигурации. Проверка станет релевантна при отдельном
   пересмотре native features/import scope, не считается автоматически покрытой сейчас.

Считать категории отдельно от фактических assertions: в текущем legacy negative test
missing Head и wrong AAD уже являются двумя calls одной категории, поэтому строка
«7 negatives» не должна скрывать реальное число cases. Новую config-проверку и public
serde check также учитывать раздельно. Это не причина менять protocol/native или
сокращать оставшуюся S3 matrix. Итоговое evidence проверяется в implementation review.
