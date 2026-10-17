# RFC-0200: M1.1.73 canonical sync v2 coverage-guided fuzz harness

Status: accepted locally; harness and exact seed corpus verified, instrumented
campaign not run, transport inactive.

## Purpose

M1.1.72 established a bounded deterministic mutation regression corpus. That
corpus is intentionally finite and has no compiler instrumentation. M1.1.73
adds the separate standard `cargo-fuzz`/libFuzzer harness needed to search the
same history-compatible canonical sync v2 decoder surface with coverage
feedback, sanitizers and a persistent evolving corpus.

The harness covers all 13 strict decoder entry points: event key, verified
event carrier, signed inventory, verified event batch, signed diff, four
terminal/control objects, both wire directions, and both signed session
checkpoint forms. Production builds do not compile the dispatcher unless the
explicit `decoder-fuzzing` Cargo feature is enabled.

## Raw corpus contract

`CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json` defines 20 exact golden seeds. The seed
preparer resolves only four allowlisted vector documents and writes raw binary
files. The first byte is a harness selector, not a protocol byte; the fuzz
target strips it before invoking the selected strict decoder. The remaining
bytes are the exact canonical payload assembled from the named JSON pointers.

The Rust integration test independently reconstructs every seed, verifies its
declared byte count and requires the selected decoder to accept the golden
payload. The PowerShell materializer additionally records a SHA-256 for every
raw input. These checks prevent a convenient but invalid text or hex-string
corpus from being mistaken for the bytes actually consumed by libFuzzer.

## Campaign and resource contract

The standard runner has three modes: preflight, a bounded smoke campaign and an
explicitly acknowledged sustained campaign. It fixes libFuzzer at one job and
one worker, sets Cargo build jobs to one, lowers the parent process priority,
limits inputs to 8,388,609 bytes and applies a ten-second per-input timeout.
The sustained contract requires at least 28,800 seconds, at least 1,000,000
executions, zero crash artifacts and a generated coverage report. A
machine-readable evidence record captures revision, elapsed time, execution
count, worker count, coverage result and the exact success decision.
Sustained mode additionally requires a clean committed worktree before and
after the run, an unchanged revision and explicit coverage generation; failure
of any requirement leaves a machine record but returns an error.

The implementation follows the Rust Fuzz Book setup: `cargo-fuzz` uses
libFuzzer and requires a nightly Rust toolchain; Windows runs additionally need
the MSVC AddressSanitizer component. Coverage reporting requires the prepared
LLVM tooling. The scripts deliberately do not install toolchains or packages
and do not hide those prerequisites behind an automatic network action.

## Current evidence and honest boundary

On the current Windows development host the stable dispatcher, selector set and
all 20 golden seeds compile and pass, while preflight reports that nightly,
`cargo-fuzz` and the MSVC AddressSanitizer runtime are absent. Therefore the
libFuzzer target has not been compiled here, no smoke or sustained campaign was
run, and no coverage report was generated.

This stage verifies a coverage-guided fuzz harness, not coverage-guided fuzzing
results. It does not close the sustained coverage-guided fuzzing gate. That
gate remains pending until a prepared host completes the eight-hour and
one-million-execution contract with coverage evidence and no crash artifact.
Fresh hosted and physical-device evidence remains pending independently.

## Deliberate non-activation

The stage changes opt-in test and evidence surfaces only. It does not register
or dial `kilogram/m1/sync/cbor/2`, authorize canonical transport, alter active
Postcard wire behavior, execute network traffic, produce a release build or
archive, or replace a stable executable.
