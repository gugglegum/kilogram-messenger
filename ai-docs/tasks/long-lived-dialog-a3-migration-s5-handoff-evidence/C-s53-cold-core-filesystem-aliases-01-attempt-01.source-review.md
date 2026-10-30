# C2-G filesystem aliases: source review

2026-10-31. Source/static submission only under accepted [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md), [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) and [verification workflow](../../verification-workflow.md). No compile, native call, fixture access or runtime verdict.

Baseline: published C2-F attempt-02 supporting package `f9a50db03aabf2330c9101c9eeacfe120ed55731`, with the accepted outer-array cardinality correction and its 92-row pure-data regression. Existing failed attempt-01 stays failed. Neither evidence set establishes whole S5.3.

| Input | Bytes | SHA-256 |
|---|---:|---|
| [Core](../../../scripts/migration-s53-cold-preservation.ps1) | 74989 | `afe18a0f4ef306f0c59e941445cdc43da0c7eb28071230a42ffbb6001b05866e` |
| [Worker](C-s53-cold-core-filesystem-aliases-01-attempt-01.preflight.ps1) | 104190 | `fe18646d87fbfd91374ae5cdfa5678ccff1f5cd8e26e328211181348299c8679` |
| [Watchdog](C-s53-cold-core-filesystem-aliases-01-attempt-01.watchdog.ps1) | 34805 | `3f1280c8d1a2b5fc7132e9f1f435af7f6e54a3ede5df4f08a17517a13cadfc34` |

Core diff is exactly three substitutions: append two selectors to C# and PowerShell allowlists; bind their fixed case roots/negative-attempt labels. Inverting them restores the accepted 74,407-byte Core, SHA-256 `9f52540e35baf01fa69caab1d195069e796e95dbbd074d1120a4e196c8bb19fa`. Production paths, native/security checks and caps are unchanged.

Fresh closed family, currently unqueried/uncreated: `E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-filesystem-aliases-01-attempt-01`. Each case starts with the same healthy 28-object layout. Existing ACL/no-follow/default-stream/content checks establish the baseline. Setup then adds one filesystem entry; prepared verification checks 29 objects.

| Case / selector | Deliberate premise and independent setup evidence | Expected Core refusal |
|---|---|---|
| `payload-hardlink` / `aliases-payload-hardlink` | Six-byte `state/one/state/payload.bin` plus same-case `alias-link`: two no-follow handles, same volume/file ID, link count 2, exact bytes and sole default stream; repeated metadata after reads. | `file_alias / capture_s / s` |
| `directory-reparse` / `aliases-directory-reparse` | Move required `state/one/state` to same-case `alias-target`, create protected replacement and set junction tag 0xA0000003. Check exact GET buffer/target and stable junction/moved-directory IDs twice. BFS never descends the junction; relocated files are checked at physical paths. | `reparse / locks / s_one_lock` |

Both expect native error 0, `bounded_refusal`, the semantic three-exception chain, zero hash passes/bytes and capture timestamp 0. Setup/helper/native failures cannot qualify as Core refusal. The alias helper and all case directory guards must close before the factory; passive hooks perform observations only. Helper snapshot/oracle has 35 scalar fields, scoped to its own owners/results; this is not a native false-close proof.

Source-derived expectations (not measurements):

| Case | Pre owned/peak; record/path/inventory bytes | Final remaining/peak; record/path/inventory bytes |
|---|---|---|
| hard link | 26/27; 12084624/7368/3618 | 0/28; 12085392/7368/3618 |
| junction | 0/0; 12073840/0/0 | 0/14; 12079216/3128/0 |

Record base 12073840, each acquisition 384, each parsed row 160. Interned paths charge aligned `32 + 2*UTF16Length`; the path model reproduces accepted C2-F values first. Hard-link inventory is 3618 bytes/`c0ab7a5a81251abc9612a48be20f557679f01bbb72de2d8aa1f5b72835862ad7`; junction inventory 3622/`2201cf6fa12d08b624c830fc84986bf3f9baebceb2009056cc46e7a45589eeee`. Geometry: 32 directory names (including junction), 27 file names, 114 stream checks, 7454 logical default-stream bytes, zero named-stream bytes within the observed/created stream scope; 36 directory guard returns/closures.

Stream coverage correction: baseline 28 for each case; prepared hard link 29, junction 28; case totals 57/56, aggregate 114 including family 1. The junction remains a separately verified deliberate target (1), but its own stream set was not observed. Zero `named_stream_bytes` covers verified objects and streams created by this setup; it does not establish absence of named streams on the junction itself.

Static checks: PowerShell AST errors 0 in all three sources; eight shared oracle bodies identical; 38 Core field assertions and corrected outer-array expression retained; C# lexical delimiters balanced (not compiled); 35 helper fields match the exact oracle. Native guardian/job, deadline, pipe draining/reap/EOF lifecycle and work20/whole30 profile are reused. Allocation/report labels are closed C2-G `r01` bindings. Thirteen frozen public inputs/evidence hashes match; HEAD remains `f9a50db`, index empty.

Remaining review/runtime questions: C# compilation, actual filesystem support/sharing behavior, measured resource counters and native owner cleanup have not been exercised. No allocation/reservation created, no family stat/read, no private keys/DB/network/Rust/product changes, no commit/push or S5.4. STOP for architectural source review and separate root runtime allocation.
