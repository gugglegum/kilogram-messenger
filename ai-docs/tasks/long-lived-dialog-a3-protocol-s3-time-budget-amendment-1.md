# A3-S3: явная поправка к бюджету времени проверок

Дата: 2026-10-25. Статус: root разрешил только изменение verification budget;
реализация S3 ещё не принята, commit/push не разрешены до её отдельного review.
Baseline: `e6f9b8ff6c7e9305d7216a94b142f0220b0c4a25`.

## Исходный предел и решение

[A3 contract §9](long-lived-dialog-a3-contract.md) задаёт
`read/codec/component checks≤10min`. [Принятый S3 plan](long-lived-dialog-a3-protocol-s3-plan.md)
и [implementation task](long-lived-dialog-a3-protocol-s3-implementation-task.md)
переносят этот ceiling и требуют анализа при превышении. Исходный текст не задаёт
самостоятельный десятиминутный бюджет каждому тесту. Поэтому три отдельных PASS
с длительностью каждого менее 600 секунд не доказывают соблюдение исходного
совокупного ограничения. Frozen contract/plan и прежние результаты не переписывать.

Для текущего S3 разрешён **совокупный бюджет 1200 секунд** на один итоговый комплект
component/codec/guard/state tests плюс выбранные relevant Codec1/Guard1/S1/S2 regressions.
Считать сумму фактических elapsed seconds финальных runner commands, включая уже
попавшие в эти замеры incremental compile/copy/launch overhead. Не вычитать накладные
расходы, не объявлять эту сумму временем одного непрерывного запуска и не подменять
её wall time параллельного выполнения. Compile-only, Clippy и fmt учитывать отдельно.
Все неудачные и промежуточные попытки сохранять и перечислять отдельно: это не один
итоговый verification pass и не часть утверждения о его времени.

Для каждого длинного native scenario N01/N02 дополнительно сохраняется **600 секунд**
от начала самого сценария, включая setup, все delivery/reload и финальную проверку
истории/состояния. При превышении любого применимого предела остановить новые тяжёлые
повторы и сообщить root измерение и состояние. Это не разрешение дальнейших автоматических
увеличений. Test/profile/production limits и полный объём принятой матрицы не меняются.

## Измерения, проверенные root

Все три команды использовали `verification` (unoptimized + debuginfo), Jobs12,
BelowNormal, stable EXE paths. По сохранённым result JSON и логам:

| Проверка | Сценарий/harness, секунд | Runner, секунд | Результат |
| --- | ---: | ---: | --- |
| N01 | 311.149 / 311.17 | 337.5438557 | 1 PASS; 642 sources, 640 Text, 1284 native calls |
| N02 | 423.412 / 423.48 | 455.6676524 | 1 PASS; 642 sources, 640 Text, 1284 native calls |
| Bounded matrix | 39.49 + 3.35 + 0.00 + 24.62 | 86.9962110 | 23 PASS across four harnesses |
| Сумма этих трёх команд | отдельные запуски | **880.2077191** | исходные 600 секунд превышены |
| Selected legacy regressions | отдельные запуски | 190.7713670 | seven commands, exit 0 |
| Текущий итоговый комплект | отдельные запуски | **1070.9790861** | около 17 мин 51 с; ниже новых 1200 секунд |

Последняя строка относится к N01/N02/bounded-final и regression-codec1,
regression-guard1, regression-s1-ledger, regression-s2-cache, regression-s2-cursor,
regression-s2-history, regression-s2-private. Она не подтверждает полноту coverage
или соответствие окончательному исходному коду: это предмет implementation review.
Исполнитель должен заново сверить финальный набор и сумму при оформлении сдачи.

Исходные immutable evidence находятся в
[S3 evidence](long-lived-dialog-a3-protocol-s3-evidence/):

| Артефакт | SHA256 |
| --- | --- |
| n01-final-result.json | `d69363d276ef0451dc23659bbb9006ea218a7f2b3ba1f97eb1ca79e1dd7e2b46` |
| n01-final.log | `c049a64c2b3e297e467998d09b1975690a27582a7cadc484a38575eeef4d43d8` |
| n02-final-result.json | `d04ceedb3f9febbe6a6684b2650b4f19fe6993bfb697b86941d4ed82606a5ebe` |
| n02-final.log | `a97366a3adda3ddaa56a05604a8655a5e55269c0633fb14848a40bb009b61fea` |
| bounded-final-result.json | `714c1653ae722e3b1746cb0c05c319c77fa8f2a456b1a0796e08e02d936f4b8d` |
| bounded-final.log | `36f2398a1014944dc8ace5803971d4e8c0bd87935c381f42dc5fdd7dd21be6d0` |

## Основание и границы интерпретации

Два real-native сценария занимают 793.2115081 секунды runner time — основную часть
набора; overhead сверх их harness time составляет около 58.56 секунды. Следовательно,
превышение нельзя объяснить только запуском runner. В N01 после 512 из 640 Text
с reload прошло 202.034 секунды; в N02 после 15 из 20 cycles с reload — 255.096 секунды.
Это реальные persistence/native/correspondence checks в unoptimized profile.
Раздельных CPU/crypto/backend IO замеров нет, поэтому конкретный внутренний bottleneck
этими логами не установлен и приписывать всё шифрованию или диску нельзя.

Решение расширяет ресурсный бюджет итоговой проверки после измерения и сохраняет
реальные native/persistence, O/A/F/R, first/tail/Seen/checkpoint, exact retry и reopen
assertions. Оно не делает пользовательскую работу быстрее, не доказывает latency
или throughput продукта и не закрывает production performance вопрос. Не переключать
профиль, не уменьшать counts и не повторять уже успешные длинные сценарии только ради
этой поправки. При необходимости повторения вследствие изменения кода применять
те же явные границы и доказать source/EXE correspondence.

В итоговом implementation report показать: исходный aggregate ceiling превышен;
root разрешил именно эту явную поправку; отдельные scenario/harness/runner времена;
точный состав финальной матрицы и её сумма; результаты legacy regressions и static
checks отдельно; сохранённые FAIL/промежуточные попытки и их причины. Acceptance кода
и разрешение публикации остаются следующими отдельными решениями root.
