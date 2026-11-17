# T1.2b. Приглашение и ответ из терминала

База `01e631444d9bc7096b6f72bf9546880f1f5c91af`. Alice и Bob готовят по одному локальному Setup для двух контекстов с одним peer. Alice создаёт аккаунт по T1.2a и экспортирует приглашение. Bob сначала проверяет и принимает приглашение в пустом каталоге, затем создаёт свой аккаунт, сохраняет слова и экспортирует ответ. Alice явно принимает ответ. Этот срез заканчивается сохранёнными привязками peer. Взаимное согласие, LAN и переход в `chat` относятся к следующим частям. [T1.2a](terminal-t1-2a-contract.md) · [Границы T1](terminal-t1-contract.md)

## Формат и проверка личности

Использовать действующие `Artifact`, `Setup::export`, `Setup::check_reply` и `Setup::select_reply`. `Artifact::decode` проверяет структуру и тип, версию, канонические ID, подписи credential и authority и их соответствие заявленным account/device. Тип invite требует пустого recipient. Reply требует другого отправителя и конкретного получателя. Формат V1 содержит подписанные учётные данные, а сам конверт приглашения и его context IDs отдельной подписью не защищены. [Тип и проверка](../../crates/kilogram-first-dialog/src/artifact.rs#L48) · [Проверка reply](../../crates/kilogram-first-dialog/src/model.rs#L518)

После inspect пользователь сравнивает полные `account_id` и `device_id` отправителя с теми, которые назвал контакт по независимому известному каналу. Для приглашения он также сверяет `invitation_id` и оба `contexts` в указанном порядке. Для reply терминал дополнительно проверяет эти поля и recipient по локальному Setup Alice. Проверка подписи устанавливает целостность учётных данных. Привязку владельца к знакомому человеку подтверждает пользователь. При расхождении или отсутствии независимой сверки пользователь очищает preview. Выбираемые файлы содержат публичные данные. Setup, ключи и слова остаются у локального владельца.

## Команды `kilogram-cli setup`

| TTY | JSONL op и args | Успех |
| --- | --- | --- |
| `/invite export` | `invite_export {}` | `E` для `exchange/invite.kilogram`. Требует открытый inviting Setup с подтверждёнными словами. |
| `/invite inspect <путь>` | `invite_inspect {"path":"<путь>"}` | `P` для приглашения. Допускается при `unopened` и не создаёт каталог. |
| `/invite join <preview_id>` | `invite_join {"preview_id":"<id>","confirmed":true}` | Снимок `S` с `prepared` и `peer_selection` роли `joiner`. Создаёт запись Setup до helper. |
| `/reply export` | `reply_export {}` | `E` для `exchange/reply.kilogram`. Требует открытый joined Setup с подтверждёнными словами. |
| `/reply inspect <путь>` | `reply_inspect {"path":"<путь>"}` | `P` для ответа, проверенного против локального открытого inviting Setup с подтверждёнными словами. |
| `/reply accept <preview_id>` | `reply_accept {"preview_id":"<id>","confirmed":true}` | Снимок `S` с `recovery_saved` и `peer_selection` роли `inviter`. |
| `/preview clear` | `preview_clear {}` | `{"cleared":true}`, включая повтор. |

`P` содержит `preview_id`, `kind` (`invite` или `reply`), `invitation_id`, упорядоченный массив `contexts`, `sender` с полями `account_id` и `device_id` и `recipient` той же формы для reply (`null` для invite). Поле `verification` имеет форму `{"credential_signatures_verified":true,"envelope_signed":false,"independent_verification_required":true,"connection_authenticated":false}`. `E` содержит поля типа и привязок, дополняет их `path` и опускает `preview_id` и `verification`.

TTY показывает тип, полные ID, путь при экспорте и ID preview при просмотре. При inspect терминал прямо сообщает, что подписи credential и authority проверены, а конверт и контексты не подписаны. Личность контакта требует проверки пользователя. Аутентифицированное соединение ещё не установлено. TTY требует сверить account, device, invitation и упорядоченные контексты по независимому каналу перед join/accept. Команды join/accept в TTY и буквальное `confirmed:true` в JSONL означают заявление пользователя об этой сверке.

После сохранения peer `S` в результатах `status/open` и других обычных переходах Setup включает `peer_selection={"role":"joiner|inviter","invitation_id":"<id>","peer":{"account_id":"<id>","device_id":"<id>"}}`. Упорядоченные контексты уже входят в `S` T1.2a. Пока peer не сохранён, поле отсутствует. TTY при открытии и status показывает эти же привязки, включая роль. Это состояние выбора peer пригодно для проверки после потери ответа или перезапуска Setup.

Framing, correlation, размер строки и последовательность публикации ответа следуют T1.2a. В TTY весь хвост после `inspect ` — путь, включая внутренние пробелы. Путь в JSONL передаётся строкой. Относительные пути разрешаются от рабочей директории процесса. NUL и пустой путь отклоняются. Путь не интерпретируется оболочкой. При выводе путь экранируется по правилам T1. [Setup service и снимки](../../apps/kilogram-cli/src/terminal_setup/service.rs#L36) · [Пути с пробелами в T1](../../apps/kilogram-cli/src/terminal_chat.rs#L235)

## Preview, привязка и сохранение

- **Inspect.** Одно ограниченное чтение обычного публичного файла до `Artifact::FILE_CAP`, затем `Artifact::decode` и проверка ожидаемого kind. Для reply вызвать общую проверку `Setup::check_reply` под тем же lock. Worker очищает прежний preview, как только начинает новый inspect, до проверки состояния и чтения. Ошибка IO, decode или kind оставляет пустой preview. Невалидный запрос либо `busy`, возвращённый до передачи worker, текущий preview сохраняет. Worker хранит один проверенный typed `Artifact` и выдаёт новый `preview_id` от `artifact::id()` на сеанс. Показанные pins берутся из этого же снимка. Preview живёт в памяти. [Предел чтения](../../crates/kilogram-first-dialog/src/model.rs) · [Artifact](../../crates/kilogram-first-dialog/src/artifact.rs#L98)
- **Подтверждение.** Дождаться retirement worker и своевременного writer ACK предыдущего результата по правилам T1.2a. При пропущенной или неуспешной записи результата сессия закрывается, ожидающее подтверждение отменяется. По точному ID и kind взять этот снимок из памяти и погасить token перед попыткой записи. Исходный путь не открывается повторно. Если файл после inspect заменили или удалили, применяются ранее показанные данные. Для нового файла пользователь делает новый inspect и сверяет его ID. Чужой, погашенный либо отсутствующий token даёт `preview_required`. При неверном token текущий снимок сохраняется. `preview clear`, допущенные к worker `begin` и `open` и выход очищают preview. `cancel` сохраняет назначение T1.2a для создания аккаунта.
- **Join.** Передать подтверждённый invite в общий защищённый начальный путь `Setup`. Он повторно проверяет тип и привязки и применяет те же проверки отсутствующего/пустого каталога, обычных файлов и `.onboarding.lock`, что в T1.2a. Запись с `joined=true`, `peer=sender`, invitation и обоими контекстами публикуется до отдельной `create`. При потерянном ответе `open` показывает тот же `prepared` Setup, сохранивший pins. Новый inspect в другом сеансе не обходит запрет join в существующий Setup. Незавершённый create и recovery дальше проходят по T1.2a. [Общая модель begin](../../crates/kilogram-first-dialog/src/model.rs#L179) · [Защищённый вход](../../crates/kilogram-first-dialog/src/model/terminal.rs#L92)
- **Reply.** После нового подтверждения сверить текущий локальный Setup под lock и тот же снимок через общую проверку reply и выбор peer. Если уже сохранён тот же `Identity` целиком, результат тот же без замены peer. Другая identity отклоняется, даже при совпадающих видимых ID. После потерянного ответа `open` показывает сохранённый peer. Для повторного accept нужны новый inspect и новое подтверждение. [Проверка и запись peer](../../crates/kilogram-first-dialog/src/model.rs#L518)
- **Export.** Путь фиксирован под `exchange` локального Setup. Перед экспортом проверить, что путь остаётся в каталоге Setup и его компоненты относятся к обычным файлам/каталогам без reparse. Общий экспорт повторно проверяет ready/recovery и выдаёт тот же файл при совпадении существующих байтов. Конфликтующий файл сохраняется с ошибкой `export_conflict`. Ошибка пути, включая reparse, возвращает `export_failed`. JSONL экранирует управляющие и bidi-символы в строке ответа, сохраняя исходный путь после JSON decode. [No-clobber export](../../crates/kilogram-first-dialog/src/model.rs) · [Проверка путей артефактов](../../crates/kilogram-first-dialog/src/connection.rs#L147)

Ошибки inspect различают `artifact_unreadable`, `artifact_invalid` и `wrong_artifact_kind`. Для валидного reply сначала проверить совпадение invitation, контекстов и recipient и вернуть `reply_mismatch` при расхождении. При совпадении этих привязок проверить сохранённую identity и вернуть `peer_conflict` для другой. Эти исходы следуют из общей проверки, которую используют inspect и accept. Для неподходящей стадии Setup применяются `not_open`, `invalid_state` и `recovery_required`, а guard join сохраняет `setup_exists`, `setup_locked`, `directory_not_empty` и `setup_invalid`. Проверка self у reply опирается на разные account/device отправителя и recipient. На пустом Setup Bob собственной identity ещё нет. Его приглашение выбирается независимой сверкой, а после создания общий `Setup::ready` проверяет, что локальный account и device отличаются от peer. [Проверка ready](../../crates/kilogram-first-dialog/src/model.rs#L334)

## Сеанс

```text
Alice> kilogram-cli setup --dir 'C:\Kilogram\Alice'
> /begin
> /create
> /recovery show
> /recovery saved
> /invite export
[Alice передаёт Bob только exchange/invite.kilogram]

Bob> kilogram-cli setup --dir 'C:\Kilogram\Bob'
> /invite inspect C:\Public Files\invite.kilogram
[Bob сверяет с Alice account, device, invitation и два контекста]
> /invite join <показанный preview_id>
> /create
> /recovery show
> /recovery saved
> /reply export
[Bob передаёт Alice только exchange/reply.kilogram]

Alice> /reply inspect C:\Public Files\reply.kilogram
[Alice сверяет с Bob account и device и проверяет показанные привязки]
Alice> /reply accept <показанный preview_id>
Alice> /status
```

## Приёмка

| Проверка | Наблюдение |
| --- | --- |
| Два свежих каталога | Реальные локальные CLI/helper без fixture, один сеанс TTY и один JSONL. Export → inspect → явное подтверждение, стабильные pins/reopen на обеих сторонах, оба готовых ID контекстов взяты из приглашения. После join до create существует только запись Setup и lock. Проверить неизменные методы T1.2a для слов. |
| Отказ проверки | Повреждённый или слишком большой файл, отсутствующий/некорректный подписанный credential или authority, другой kind, self reply, чужой recipient, другой invitation или контексты reply. У приглашения от неизвестного лица пользователь может очистить preview без изменений. Ошибка нового inspect погашает прежний preview. |
| Связность preview | После валидного inspect заменить входной файл на другой валидный файл. Join/accept применяет только снимок, чьи pins были показаны. Отдельно проверить неверный ID, отказ при `confirmed:false`, clear, повтор погашенного ID и новый процесс. |
| Сохранность | Отклонить join в чужом/непустом каталоге, второй Setup lock, export/accept до recovery. Два одинаковых export дают тот же файл, чужой existing output сохраняется. Повторно проверенный тот же ответ сохраняет одну identity, дальнейший другой подтверждённый ответ отклоняется. После reopen проверяются те же pins. |

Матрица локальная на новых одноразовых каталогах. Достаточны целевые Windows harness под стабильными именами и короткий пользовательский прогон. Слова в квитанции не попадают.
