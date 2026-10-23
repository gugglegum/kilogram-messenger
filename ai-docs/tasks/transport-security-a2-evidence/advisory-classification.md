# Advisory classification, 2026-10-24

RustSec snapshot `84d943a6deeb8cc6c6f632531aaf4f1c62275239`, cargo-audit 0.22.2.
Baseline: 11 vulnerability records + 8 warnings. Final: **0 vulnerability records +
7 warnings; exit1**. Эти числа не включают upstream-only GHSA, для которого есть
обязательный самостоятельный source/version/checksum gate.

## Совместимые patches

| Findings | Reachability / решение |
|---|---|
| [ctutils 2026-0385](https://rustsec.org/advisories/RUSTSEC-2026-0385.html) | Active Windows graph: Iroh, digest, universal-hash. 0.4.2→0.4.4; timing exploit не воспроизводился. Crypto/identity/ratchet и реальные native paths проверены |
| quinn-proto [0338](https://rustsec.org/advisories/RUSTSEC-2026-0338.html), [0340](https://rustsec.org/advisories/RUSTSEC-2026-0340.html), [0343](https://rustsec.org/advisories/RUSTSEC-2026-0343.html) | Присутствует в lockfile, отсутствует в default Windows normal/build graph. 0.11.17→0.11.18 — точечное исправление locked optional stack, без включения HTTP3/DoQ features |
| rustls [0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html), [0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html), [0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html), [0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html) | Active TLS stack. 0.23.43→0.23.46 закрывает все четыре записи. Debug/negotiation preconditions отдельно не доказывались; Brotli feature отсутствует в просмотренном default graph. Без verifier bypass |
| [rustls-webpki 0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) | Active certificate verification dependency. 0.103.15→0.103.16 |
| roots [0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html), root-certs [0383](https://rustsec.org/advisories/RUSTSEC-2026-0383.html) | roots используется relay TLS; root-certs отсутствует в default Windows normal/build graph. Оба locked bundles 1.0.9→1.0.10, без изменения trust policy приложения |
| [tokio 0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html), warning | Active Tokio, affected Unix AsyncFd API недоступен на Windows. 1.53.1→1.53.3 — совместимый portable patch. Unix runtime test не заявляется |

## Предупреждения остаются видимыми и блокируют строгий gate

| Warning | Проверенная граница |
|---|---|
| [lru 2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html), 0.16.4 unsound | Default Windows graph: rqrr 0.10.1 → CLI/GUI QR. В reviewed `rqrr/src/prepare.rs`: `LruCache<u8, ColoredRegion>`, методы get/put/pop_lru. Panicking key Drop/вызов affected pop и catch_unwind trigger не обнаружены. Это source-based ограничение, не доказательство безопасности всех consumers. lru≥0.18.2 потребует отдельного изменения QR dependency; без forced semver override |
| [atomic-polyfill 2023-0089](https://rustsec.org/advisories/RUSTSEC-2023-0089.html), 1.0.3 unmaintained | В lockfile; default Windows normal/build reverse tree пуст. Иные feature/target builds не проверены |
| [paste 2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html), 1.0.15 unmaintained | В lockfile; default Windows normal/build reverse tree пуст. Замена macro dependency не входит в transport patch |
| [ttf-parser 2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html), 0.25.1 unmaintained | Active GUI font stack: owned_ttf_parser → ab_glyph → eframe. Maintenance notice, не доказанная удалённая уязвимость. GUI dependency migration вне A2 |
| [zbus_names 2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html), 4.3.4 unmaintained | Default Windows normal/build reverse tree пуст; platform accessibility dependencies остаются в lockfile |
| [zvariant 2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html), 5.15.0 unmaintained | То же; переход на иной zbus dependency layout требует отдельного scope |
| [zvariant_derive 2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html), 5.15.0 unmaintained | То же. Никаких ignore/blanket exceptions не добавлено |

Reachability определялась `cargo tree --locked --offline --workspace --target
x86_64-pc-windows-msvc --edges normal,build --invert <package>`; пустой reverse tree
не равен отсутствию в lockfile или безопасности иных targets/features. Сохранён
[итоговый reachability evidence](advisory-reachability.txt). Исправления применены по
объявленным upstream ranges; новые exploit проверки не проводились.
