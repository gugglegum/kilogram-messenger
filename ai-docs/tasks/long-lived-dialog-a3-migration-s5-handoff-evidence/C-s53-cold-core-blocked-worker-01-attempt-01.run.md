# C2-Q actual run — failed, preserved

The single authorized wrapper returned **exit 1** before the work cutoff. Supervisor reports `assertion`, ready observed but witness not accepted; cutoff/primary-kill timestamps remain zero. This is **not** blocked-worker deadline coverage.

The root allocation filename mismatch was corrected by root with the same bytes before consumption. It caused no runtime attempt. The later runtime failure is separate: the parent's default JSON parser converts `worker_started_utc` into `System.DateTime`, while the accepted oracle requires a string. Public-only type reproduction confirms that mismatch; no source fix was applied.

The cleanup path forcibly terminated the retained worker (exit -1), observed exit/reap/both EOF and closed parent resources. Historical last Core still has32 owners/inputreleasefalse; no final Core snapshot, worker finally or individual close count is claimed. Sources and native reports remain unchanged. No retry, extra fixture/event probe, ledger edit or commit/push.

Request-to-final **5.536s**; native wall **3.4566043s** / app3456ms. These overlap and are not added. Root owns charge/release.

```json
{
  "schema_version": 1,
  "classification": "cold-core-blocked-worker-executor-failed-actual-r01",
  "attempt": "C-s53-cold-core-blocked-worker-01-attempt-01",
  "executor_thread_id": "01a1653c-0154-7611-8e8c-044fbe1e4d7a",
  "root_thread_id": "01a164ec-589c-7b41-bc34-3ba158d1f22b",
  "verdict": "failed_not_deadline_coverage",
  "wrapper_invocations": 1,
  "attempt_consumed": true,
  "prerun_activation_error": {
    "description": "Root initially wrote .allocation.json while immutable W/S read .root-allocation.json.",
    "detected_before_consumption": true,
    "native_invocations_before_correction": 0,
    "root_created_canonical_same_bytes": true,
    "canonical_bytes": 1163,
    "canonical_sha256": "5eded38b77e9364a3be92780ddbe5cb41369e17c97a212a3c7236af45b118b7a",
    "source_changed": false,
    "initial_public_preflight_chunk": "40c7d0"
  },
  "before_utc": "2026-10-31 03:55:09 UTC",
  "request_utc": "2026-10-31T03:55:09.479Z",
  "initial_return_utc": "2026-10-31T03:55:15.015Z",
  "final_return_utc": "2026-10-31T03:55:15.015Z",
  "clock_basis": "before: clock.curr_time UTC second precision; request/final: executor Date.toISOString immediately around native tool call",
  "tool_arguments": {
    "cmd": "& 'C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe' -NoLogo -NoProfile -NonInteractive -File 'E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.watchdog.ps1' -AllocationSha256 '5eded38b77e9364a3be92780ddbe5cb41369e17c97a212a3c7236af45b118b7a'",
    "shell": "C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe",
    "login": false,
    "workdir": "E:\\AI-Projects\\kilogram-p2p-messenger",
    "yield_time_ms": 10000,
    "max_output_tokens": 4000,
    "sandbox_permissions": "require_escalated",
    "justification": "Один разрешённый локальный synthetic тест до 30 секунд: блокировка только собственного тестового worker и его завершение supervisor’ом, без пользовательских данных, ключей и сети."
  },
  "native_chunks": [
    {
      "chunk_id": "4099f6",
      "wall_time_seconds": 3.4566043,
      "exit_code": 1,
      "original_token_count": 0,
      "output": ""
    }
  ],
  "actual_app_item": {
    "type": "commandExecution",
    "id": "exec-d3121915-4520-4632-8c22-8f7e4abbc1a9",
    "command": "\"C:\\\\Users\\\\Paul\\\\.cache\\\\codex-runtimes\\\\codex-primary-runtime\\\\dependencies\\\\native\\\\powershell\\\\pwsh.exe\" -NoProfile -Command \"& 'C:\\\\Users\\\\Paul\\\\.cache\\\\codex-runtimes\\\\codex-primary-runtime\\\\dependencies\\\\native\\\\powershell\\\\pwsh.exe' -NoLogo -NoProfile -NonInteractive -File 'E:\\\\AI-Projects\\\\kilogram-p2p-messenger\\\\ai-docs\\\\tasks\\\\long-lived-dialog-a3-migration-s5-handoff-evidence\\\\C-s53-cold-core-blocked-worker-01-attempt-01.watchdog.ps1' -AllocationSha256 '5eded38b77e9364a3be92780ddbe5cb41369e17c97a212a3c7236af45b118b7a'\"",
    "cwd": "E:\\AI-Projects\\kilogram-p2p-messenger",
    "status": "failed",
    "exitCode": 1,
    "durationMs": 3456
  },
  "session_id_supplied": false,
  "cell_id_supplied": false,
  "polling_calls": [],
  "request_to_final_return_seconds": 5.536,
  "native_wall_seconds": 3.4566043,
  "app_duration_ms": 3456,
  "input_pins_before": [
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-cold-preservation.ps1",
      "bytes": 80601,
      "sha256": "74f53161ddfb4862efb20249e7a6eeac11d9b1c71665a2755fff1fe6b907703d"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.preflight.ps1",
      "bytes": 69272,
      "sha256": "1ff5ff68b427cfdcb58f0ff6d628681ca60a0551bffbb13daad05385cd5938ca"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.watchdog.ps1",
      "bytes": 37598,
      "sha256": "79ff37841d787945cf35c019ffe34db9a44a932800f0e506c2da0af9ff11a64b"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.source-review.md",
      "bytes": 8750,
      "sha256": "a1788a11d73e928f89da55cf1b146d7f7b3ba0b683441ce6d0725b8bdc729875"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.root-allocation.json",
      "bytes": 1163,
      "sha256": "5eded38b77e9364a3be92780ddbe5cb41369e17c97a212a3c7236af45b118b7a"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-root-copy.ps1",
      "bytes": 25596,
      "sha256": "a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s5-l1-deadline.ps1",
      "bytes": 6038,
      "sha256": "0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45"
    },
    {
      "path": "C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe",
      "bytes": 301368,
      "sha256": "362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139"
    }
  ],
  "input_pins_after": [
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-cold-preservation.ps1",
      "bytes": 80601,
      "sha256": "74f53161ddfb4862efb20249e7a6eeac11d9b1c71665a2755fff1fe6b907703d"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.preflight.ps1",
      "bytes": 69272,
      "sha256": "1ff5ff68b427cfdcb58f0ff6d628681ca60a0551bffbb13daad05385cd5938ca"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.watchdog.ps1",
      "bytes": 37598,
      "sha256": "79ff37841d787945cf35c019ffe34db9a44a932800f0e506c2da0af9ff11a64b"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.source-review.md",
      "bytes": 8750,
      "sha256": "a1788a11d73e928f89da55cf1b146d7f7b3ba0b683441ce6d0725b8bdc729875"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.root-allocation.json",
      "bytes": 1163,
      "sha256": "5eded38b77e9364a3be92780ddbe5cb41369e17c97a212a3c7236af45b118b7a"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-root-copy.ps1",
      "bytes": 25596,
      "sha256": "a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s5-l1-deadline.ps1",
      "bytes": 6038,
      "sha256": "0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45"
    },
    {
      "path": "C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe",
      "bytes": 301368,
      "sha256": "362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139"
    }
  ],
  "pins_unchanged": true,
  "reports": [
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.started.json",
      "present": true,
      "bytes": 1032,
      "sha256": "395c2315cce1c2b3e8f9e06d9774f3a9e3e661e644b669302d4ac9c78db76e20"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.witness.json",
      "present": true,
      "bytes": 7258,
      "sha256": "7a6aab3c8a224beb37a7801c4c62a923b7261f89deeb5137c1c731f8f6444eb2"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-01.watchdog.json",
      "present": true,
      "bytes": 4610,
      "sha256": "c8a778df7fca490207831942e75cedeefba1849be1686683c00bf3c5ca69eab6"
    }
  ],
  "native_report_total_bytes": 12900,
  "report_bounds": {
    "each": 65536,
    "total": 196608,
    "within": true
  },
  "observed": {
    "native_exit": 1,
    "supervisor_result": "failed_or_unknown",
    "primary_reason": "assertion",
    "primary_cause": {
      "type": "System.InvalidOperationException",
      "hresult": -2146233079,
      "native_error": 0,
      "marks": {
        "c2q_code": "assertion"
      }
    },
    "supervisor_seconds": 3.1898723,
    "ready_seen": true,
    "witness_written": true,
    "witness_fields": 50,
    "witness_accepted": false,
    "cutoff_qpc": 0,
    "primary_kill_attempted": false,
    "primary_kill_returned": false,
    "cleanup_kill_attempted": true,
    "forced": true,
    "guardian_forced": false,
    "child_pid": 31768,
    "child_exit": -1,
    "child_reaped": true,
    "stdout_eof": true,
    "stderr_eof": true,
    "stdout": "",
    "stderr": "",
    "supervisor_cleanup_unknown": false,
    "stdio_disposed": true,
    "managed_inputs_disposed": true,
    "parent_events_after": {
      "slots": 2,
      "returned": 2,
      "close_attempts": 2,
      "closed": 2,
      "remaining": 0,
      "cleanup_unknown": false,
      "records": [
        {
          "role": "ready",
          "returned": true,
          "created_new": true,
          "close_attempted": true,
          "closed": true
        },
        {
          "role": "block",
          "returned": true,
          "created_new": true,
          "close_attempted": true,
          "closed": true
        }
      ]
    },
    "observed_owned_child_processes_after": 0,
    "worker_apartment": "MTA",
    "last_core_snapshot_fields": 38,
    "last_core_observation": {
      "property": "bounded_payload_equality_cooperative_owner",
      "whole_attempt_verdict": "not_established",
      "continuous_namespace_absence": "not_established",
      "completed": false,
      "expected_set_captured_before_hash": true,
      "observed_entry_sets_equal": false,
      "input_handles_released": false,
      "cleanup_unknown": false,
      "receipts_completed": false,
      "cleanup_native_error": 0,
      "failure_code": null,
      "failure_phase": null,
      "failure_role": null,
      "cause_kind": null,
      "native_error": 0,
      "error_hresult": 0,
      "expected_files": 5,
      "expected_bytes": 52,
      "locks": 6,
      "hash_passes": 0,
      "hash_bytes": 0,
      "capture_complete_timestamp": 2690117730929,
      "historical_tuple_digest": "54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5",
      "s_before_tuple_digest": null,
      "bkp_before_tuple_digest": null,
      "s_after_tuple_digest": null,
      "bkp_after_tuple_digest": null,
      "peak_owned_handles": 33,
      "remaining_owned_handles": 32,
      "peak_path_bytes": 7024,
      "peak_record_bytes": 12086928,
      "peak_inventory_bytes": 3586,
      "scratch_reserved_bytes": 524288,
      "receipt_bytes_charged": 0,
      "receipt_bytes_known_written": 0,
      "receipt_write_unknown": false,
      "phase_seconds": {
        "allocation": 0.0006591,
        "locks": 0.024519,
        "inventory": 0.0046939,
        "capture_s": 0.0019413,
        "capture_bkp": 0.0120131
      },
      "private_receipt_digests": {}
    },
    "worker_final_core_observation": null,
    "worker_finally_completion_observed": false,
    "worker_close_count": null
  },
  "diagnosis": {
    "public_only_type_reproduction": {
      "field": "worker_started_utc",
      "default_parser_type": "System.DateTime",
      "expected_parameter_type": "System.String",
      "is_string": false,
      "witness_field_count": 50,
      "datekind_parameter_available": true
    },
    "source_path": "Parent ConvertFrom-Json -Depth12 -> Assert-Witness -> Assert-Fields worker_started_utc string check",
    "finding": "Default ConvertFrom-Json yields System.DateTime for worker_started_utc, while Assert-Witness expects the string parameter ChildStartUtc. Worker validated its in-memory string object; the parent rejects the parsed witness. This accounts for assertion before witness acceptance and before work cutoff.",
    "runtime_deadline_not_reached": true,
    "source_fix_not_applied": true
  },
  "boundaries": {
    "source_modified_after_acceptance": false,
    "retry": false,
    "fallback_run": false,
    "extra_fixture_or_event_probes": false,
    "standalone_compile_or_project_invocation": false,
    "private_access": false,
    "network_access": false,
    "product_runtime": false,
    "ledger_modified_by_executor": false,
    "accounting_owner": "root: request-to-final once; native/worker/supervisor durations overlap and are not summed",
    "worker_kernel_handle_release": "Inference from observed process exit, not application finally or per-handle measurement.",
    "whole_attempt_verdict": "not_established",
    "cooperative_timeout_coverage": false,
    "hard_guardian_firing_coverage": false,
    "blocked_worker_deadline_coverage": false,
    "commit_push": false,
    "s54": false
  },
  "stop": "Failed consumed attempt preserved. No repair/retry/source edits. Await root actual review."
}
```
