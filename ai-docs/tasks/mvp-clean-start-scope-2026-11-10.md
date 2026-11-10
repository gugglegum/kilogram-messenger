# MVP scope: current version from clean profiles

User clarification 2026-11-10: nobody uses the old development version; the project can start directly on the new version. Backward compatibility with old development versions and lossless migration of their profiles are therefore removed from MVP acceptance requirements. This supersedes the requirement to finish migration-only S5.4/F/WholeP before progressing toward a usable MVP.

This is a scope change, not a claim that the failed migration acceptance passed. S5.3 results remain historical; F01-F13 failures/results remain intact. Existing code, old profiles and evidence are not deleted, reset or silently converted by this decision. Unknown/old formats must not be overwritten on startup.

Required MVP proof now concerns new-to-new operation: safe fresh account/device/session creation; two users connecting and exchanging messages; authenticated E2EE; durable lossless history; offline queues and duplicate/replay handling; process restart/crash recovery; key/device transitions within the supported current version; context isolation. Direct/relay and user-facing onboarding must be assessed against actual product requirements, not inferred from migration test success.

The current implementation may route new-protocol activation through compatibility machinery. Inspect actual production CLI/runtime/desktop entrypoints before removing dependencies or claiming a simple flag is sufficient. If necessary, implement direct current-format initialization with the same authentication, authority and durability guarantees. Do not mistake clean-start permission for dropping current-version recovery or key-rotation correctness.

Executor instructed to reach a safe boundary in BG, preserve unfinished work, stop new migration-only builds/tests, and provide a short read-only map of the clean-start path, existing functionality, minimal missing changes and reusable BG findings. No further F14/WholeP assignment. Do not finish obsolete migration gates merely because prior stages consumed effort.

Next root action: review the concrete clean-start map, define the smallest end-to-end current-version task, and let the executor implement/debug/test autonomously under verification-workflow.md. Independent final review and accepted-scope commit/push remain.
