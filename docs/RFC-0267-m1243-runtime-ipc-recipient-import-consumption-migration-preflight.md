# RFC-0267: Runtime IPC recipient-import-consumption migration preflight (M1.2.43)

Status: accepted as an executable, inactive migration preflight.

## 1. Why a separate migration is mandatory

`RuntimeIpcCommand` and `RuntimeIpcResponse` are Postcard enums. Adding a
variant changes numeric discriminants and is therefore an incompatible byte
contract even when the new variant is appended. The consumption ceremony may
not be inserted under the existing IPC version 33 or the signed descriptor
domain `kilogram:runtime-ipc-descriptor:v14\0`.

The next active generation is allocated as IPC version 34 with signed JSON
descriptor domain `kilogram:runtime-ipc-descriptor:v15\0`. The command is
exactly `RecipientImportConsumption { request: Vec<u8> }`; the matching result
is exactly `RecipientImportConsumption { signed_response: Vec<u8> }`. Both
byte vectors carry the already canonical, independently validated and signed
M1.2.37/M1.2.41 nested contract. The outer Postcard layer must not reinterpret
that nested payload.

## 2. Compatibility behavior

This local-control protocol connects co-installed Kilogram binaries, so the
migration is an exact-version hard cut rather than a dual reader. Version 33
descriptors are rejected by a version 34 client before connect. An authenticated
version 33 request is rejected by a version 34 server before actor dispatch.
Mixed binaries require updating both processes and restarting the runtime.

There is no format sniffing, fallback or silent downgrade. Version and signed
descriptor domain advance together; every hybrid `33/v15` or `34/v14` state is
invalid. No peer, relay, mailbox, sync or history-transfer network wire changes.

## 3. Atomic implementation set

The activation commit must update as one reviewed unit:

1. both Postcard enum inventories and the exact version/domain constants;
2. canonical runtime IPC schema and completion inventory;
3. the protocol compatibility registry;
4. every live-binding gate that currently pins version 33 or domain v14; and
5. the runtime dispatch match, initially with an explicit fail-closed service
   availability boundary if the M1.2.42 cache is not yet owned by the process.

The preflight found 58 boundary files that bind at least one current constant.
Historical manifests continue to describe the versions shipped at their own
milestones. Their gates may recognize the explicit successor contract, but
their historical claims must not be rewritten to pretend they originally used
version 34.

## 4. Activation and authority boundary

M1.2.43 changes no executable bytes. Version 33/domain v14 remain active, and
the new command and response are absent. The process-local M1.2.42 cache remains
dormant and no handler, CLI, desktop control, timer or network path is added.

The later handler must own the cache only for the runtime process lifetime.
Restart, expiry, capacity exhaustion, missing Plan stage, descriptor mismatch
and Device mismatch all fail closed. Registration alone must not grant Commit
authority or expose unsigned outcomes.

The public default remains `legacy-only`. This preflight requires no release
build, archive, tag, second host, product network or automatic GitHub build.
