# Cold-B B0a root review and implementation scope

2026-10-31. Baseline HEAD 32c4361c58a90a4ce4ad46edb742f4234eae74f4.

Root accepts the amended Cold-B direction: B0 and B1 are separate code reviews but a single typed session; existing-file adoption must not create or recover state before verification. Contract SHA256 1a1923b96903e134da044aae28b599dc208e92161bb840caf8b27b9b8911325e. This is not acceptance of an implementation or private activation.

The next implementation is deliberately smaller than the contract's proposed B0: only the additive owned-directory/lock adoption API and focused synthetic Windows interop and refusal tests. No C# preservation lifecycle, full channel, B0/B1 reader or access to S/O/BKP/R/E. Default acquire behavior stays unchanged. Respect workspace unsafe policy and use existing safe Windows wrappers where possible. Check actual handle identity/path/kind, no-reparse, empty single-link lock, journal refusal without writes, guard lifetime and duplicate/range-lock interoperability. Do not replace real ownership with caller assertions.

Executor may implement and perform relevant synthetic tests after self-review without another source-only stop. Build locked/offline with --no-run, use stable Windows runner, CargoJobs2/BelowNormal, finite timeouts and actual logs. Up to600 seconds C total, at most180 seconds per command, actual wall charge to be reported; this fits historical inactive C670.0705091. No F allocation. No new activation/manifest framework for synthetic checks. Preserve failed results and diagnose before retry. Submit diff and actual evidence to root before commit/push.

The existing standing test-data authorization remains effective. Per-run timeouts are execution controls, not renewed human-permission requirements. Remaining Cold-B lifecycle/continuation logic requires code review; the complete B0+B1 private run must wait for both readers and ownership handling. G1-G4 and S5.3 acceptance remain open.

Primary Windows documentation reviewed: DuplicateHandle refers to the same object and DUPLICATE_SAME_ACCESS retains access rights; LockFileEx permits GENERIC_READ or GENERIC_WRITE and must be tested for the exact duplicate lifetime used here. These documented properties are premises, not measured interop results.

- https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex

## Early review notes and next bridge decision

Root requested an actual duplicate lifetime test: dropping one File is not documented to release locks while duplicates remain; post-lock constructor refusal also needs a defined cleanup path. See https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock . Default acquire must not change.

The later cross-process bridge has a possible simplification, not yet accepted or implemented: let Rust open/adopt the two S directory/lock pairs using safe std APIs and retain the originals; the already-owned C# child duplicates those parent handles into its registry with its existing native interop. Thus Rust never needs unsafe FromRawHandle or cross-process DuplicateHandle. C# still captures O/BKP and all payloads, with full equality after both actual Rust leases and all six locks exist. No typed open precedes that check. This reverses only the S handle origin; exact parent identity, duplicate rights, lifetime and retained ancestry still need focused interop verification. Review this direction before adding a new Rust FFI dependency or rejecting the architecture as impossible. B0a remains narrowly scoped as above.
