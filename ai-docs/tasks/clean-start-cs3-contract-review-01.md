# CS3 contract review 01

2026-11-10. Reviewed contract SHA256 00a062281e3b961aeb76724801b22676ce4152c5d350098e1c35682a80e56480 (32926 bytes), baseline f7fd176d33c93eebe79d4d6fa0df385eca111290. Root033747 verified document,26 links,HEAD/index; inspected the desktop bootstrap/helper boundaries, Runtime2 IPC dispatch, CS1 roles/authority, actual RouteConfig2 limits and RFC0083.

## Decisions

**CS3.1 invite-first bootstrap/awaiting scope is accepted for implementation.** The account is usable as the foundation of the first-dialog pilot, not a completed chat/MVP. Inviter and joiner share the exact ordered contexts before CS2 creation; canonical consent role is derived later from real device IDs. Do not create one account per additional contact or silently change slots. The stage must remain outside legacy profile/readers/start actions.

The three-slice sequence is accepted as a roadmap, not blanket implementation approval for CS3.2/3. Exact artifact/IPC schemas and tests for those slices will be reviewed at their boundary. In particular, the shared 512KiB envelope cap must be reconciled with hex expansion and all simultaneous fields when proposal/final kinds are introduced. CS3.1 implements only invite/reply; unknown future kinds fail closed.

The bounded helper adapter is necessary. Secret stdout and stderr must not reach generic diagnostics. Ensure cancellation/app-close and late worker responses are ownership-safe, with finite waits and no orphan child. An acknowledgement UI flag is not a cryptographic guarantee that the user backed up the phrase. Its durable state must never be set by helper success, a ready receipt or an export click alone.

Write a small nonsecret pending UI record **outside the destination that CS2 requires absent**, before launching bootstrap, so a lost response/restart can be presented honestly. Reopen must distinguish incomplete creation, ready without phrase acknowledgement, and acknowledged awaiting state. Validate receipt/public bindings and expected locally chosen path/contexts; a manifest alone is not authority to launch a store. No automatic bootstrap retry at an occupied destination, reset or phrase reconstruction. Persist peer/contexts/artifact selection consistently; asynchronous stale results cannot change a newer selection.

External peer comparison before signing is required. In CS3.1 invite/reply parsing and Root signature verification validate data, but do not by themselves establish that the peer is the intended human. Freeze confirmed peer identity and exact invitation/context bindings; future imports cannot silently replace them. No Root signing or final consent install in CS3.1.

## Deferred decisions and explicit gaps

CS3.3 needs durable request identity for ambiguous-send recovery; purely RAM retry is insufficient. The proposed metadata-only file is a reasonable direction, **not yet an approved exact schema**. A raw hash of a short message is dictionary-testable and must not be described as hiding its content. Before implementation choose a protection strategy consistent with existing local storage, bound retention/cleanup and specify tamper/replay and crash behavior. Do not implement this in CS3.1 or invent a new cryptographic subsystem now.

Remote connectivity, account-first arbitrary contacts and default-on bounded volunteer storage remain product/acceptance gaps, not removed requirements. The route restriction is real: current direct binds are loopback-only; a two-device relay candidate requires actual verification. Do not use the old volunteer checkbox as proof the new runtime provides that role, and do not silently downgrade its intended default. These gaps do not block the explicitly labelled first-dialog UI pilot, and the pilot does not complete the MVP.

No legacy create repair, migration acceptance, historical private fixture access or broad endurance rerun is required for CS3.1. Its concrete implementation assignment is clean-start-cs3-1-onboarding-task.md. Final implementation/evidence review precedes exact-scope commit/push.
