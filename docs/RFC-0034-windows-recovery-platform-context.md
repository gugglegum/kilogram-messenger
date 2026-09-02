# RFC-0034: Windows recovery platform context (M0.9.7)

Статус: реализовано в M0.9.7

Дата: 2026-09-02

## 1. Задача

M0.9.5 подписал network/power execution policy, а M0.9.6 сохранил retry state
между процессами. Однако runtime context передавал caller, поэтому coordinator
не мог отличить реальное Ethernet/Wi-Fi/mobile/metered/roaming состояние от
произвольных CLI-аргументов.

M0.9.7 добавляет первую доверенную platform boundary и Windows-native snapshot:

- класс физического сетевого интерфейса;
- Windows connection cost, metered и roaming;
- data-limit/background restriction diagnostics;
- источник питания, состояние батареи и Energy Saver;
- консервативное поведение при VPN, нескольких маршрутах и ошибках API.

Срез не регистрирует Windows service/Task Scheduler, не подписывает platform
snapshot и не добавляет network-change/power-change subscriptions. Wakeup всё
ещё выполняет внешний caller.

## 2. Platform-neutral boundary

`recovery_platform` определяет `RecoveryPlatformContextProvider` и value types,
которые не содержат Win32/WinRT handles или типов. Signed recovery plan и
protocol/core crates по-прежнему знают только существующие
`RecoveryNetworkClass` и `RecoveryPowerSource`.

Windows implementation компилируется как target-specific dependency приложения
и использует WinRT `Windows.Networking.Connectivity` и
`Windows.System.Power`. На других ОС system provider возвращает explicit
`unsupported-platform` с `unknown`; будущие adapters смогут реализовать ту же
boundary без изменения recovery plan или wire protocol.

## 3. Network probe и VPN

Probe сначала читает Internet Connection Profile. Класс интерфейса считается
точным только для:

- WinRT WLAN/WWAN markers;
- IANA type 6 — Ethernet;
- IANA type 71 — Wi-Fi;
- IANA types 243/244 — mobile WWAN.

Любой другой тип, включая tunnel/VPN, сам по себе даёт `unknown`. Если основной
Internet profile является tunnel, adapter перечисляет active profiles и может
выбрать только один exact physical profile. Один Internet profile имеет
приоритет над одним local/constrained profile. Ноль либо несколько подходящих
physical profiles остаются `unknown`; случайного выбора по порядку API нет.

Для выбранного профиля probe требует известные connection cost и roaming.
`Fixed`, `Variable` либо roaming переводят effective class в существующий
policy bucket `mobile`: signed `allow_mobile` тем самым одновременно означает
явное согласие на mobile/metered/roaming transfer. Неизвестные cost или roaming
делают effective class `unknown`, даже если media type похож на Ethernet.

Локально печатаются connectivity, exact cost, metered, roaming, over/approaching
data limit и background restriction. Имена профилей, SSID, adapter GUID, IP и
другие сетевые идентификаторы не выводятся и не сохраняются scheduler state.

## 4. Power probe

Windows `PowerManager` даёт power supply, battery, Energy Saver и remaining
charge. Policy получает:

- `external`, только если supply `Adequate`;
- `battery`, если батарея `Discharging` или `Idle` без adequate supply;
- `unknown` при inadequate/contradictory/unavailable данных.

Процент заряда не выводится для `BatteryStatus::NotPresent`. Energy Saver и
battery status пока diagnostics: signed plan v1 разрешает только требование
external power и не меняется в этом срезе.

## 5. CLI и fail-closed semantics

Новая команда:

```powershell
kilogram-cli platform-context
```

делает один read-only native snapshot. `history-recovery-plan-run` теперь по
умолчанию использует тот же provider, если оба прежних аргумента отсутствуют:

```powershell
kilogram-cli history-recovery-plan-run `
  --state-dir .\recipient `
  --plan-file .\approved-recovery.plan `
  --conversation example
```

Для deterministic development tests сохранён explicit override: старые
`--network-class` и `--power-source` должны передаваться только вместе. Один
аргумент отклоняется до чтения plan/discovery. Output явно различает
`windows-native`, `caller-supplied` и `unsupported-platform`.

Default signed plan разрешает Ethernet/Wi-Fi и запрещает mobile/unknown. Поэтому
unavailable/ambiguous native network snapshot блокирует runner до UDP и Iroh.
Пользователь может разрешить unknown только при создании нового signed plan;
adapter не ослабляет уже выданное consent.

## 6. Совместимость и проверки

Recovery plan v1, scheduler state v1, URI v1, ticket v9, ALPN
`kilogram/m0/sync/7`, wire messages и checkpoints не изменились. Добавлен только
Windows-target dependency `windows 0.62.2`; crate уже присутствовал в lockfile
транзитивно, но теперь features platform adapter объявлены явно.

- unit tests покрывают exact interface mapping, VPN/ambiguous fail-closed,
  metered/roaming policy bucket, power mapping и all-or-none manual override;
- M0.9.6 process regression `.tmp/m096-smoke-20260902-140653` снова прошёл
  restart/deferred/resume/cancel и identical history;
- Windows process smoke `.tmp/m097-smoke-20260902-142158` получил native
  Ethernet/unrestricted/non-roaming/external snapshot даже при активном VPN;
- completed signed plan автоматически использовал `windows-native`, прошёл
  policy gate и не открыл UDP/connection; partial manual override был отклонён;
- все 118 workspace tests, rustfmt, strict Clippy и release build проходят.

## 7. Что остаётся дальше

- настоящий Windows background service/task с bounded wakeup;
- подписка на network/cost/roaming/power change вместо разового snapshot;
- повторный policy check непосредственно перед discovery и connect при смене
  среды во время живого процесса;
- macOS/Linux/Android/iOS providers той же boundary;
- monotonic OS clock/witness для scheduler rollback;
- privacy-preserving wide-area discovery/mailbox вместо LAN-only multicast.
