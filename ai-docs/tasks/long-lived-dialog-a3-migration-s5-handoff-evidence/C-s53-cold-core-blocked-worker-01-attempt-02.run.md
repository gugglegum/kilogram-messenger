# C2-Q attempt02 actual run

**Observed expected external work cutoff; pending root actual review.** One wrapper invocation, native exit0. Initial call yielded session37439; one write_stdin poll of that same invocation returned the final exit. No second launch or in-place repair.

Witness accepted before20s, with exact identity/MTA/pins and historical Core32-owned snapshot. Parent detected work_expired, observed its retained child alive, called Kill, then observed exit-1/reap/both EOF and known parent resource cleanup. forced=true/guardian_forced=false; cleanup_kill_attempted=false. Worker finally/final Core snapshot/individual closes are not claimed; the last Core32 remains historical. No cooperative timeout or hard-fallback firing coverage.

Request-to-final **22.355s**; actual app command20236ms; supervisor20.0945091s. The two raw chunk wall times describe tool waits and exclude the between-call gap; these overlapping durations are not added. Root owns charge/release.

```json
{
  "schema_version": 1,
  "classification": "cold-core-blocked-worker-attempt02-executor-actual-r01",
  "attempt": "C-s53-cold-core-blocked-worker-01-attempt-02",
  "root_thread_id": "01a164ec-589c-7b41-bc34-3ba158d1f22b",
  "executor_thread_id": "01a1653c-0154-7611-8e8c-044fbe1e4d7a",
  "verdict": "observed_expected_external_work_cutoff_pending_root_review",
  "wrapper_invocations": 1,
  "before_utc": "2026-10-31 04:07:33 UTC",
  "request_utc": "2026-10-31T04:07:33.194Z",
  "initial_return_utc": "2026-10-31T04:07:45.321Z",
  "poll_request_utc": "2026-10-31T04:07:53.710Z",
  "final_return_utc": "2026-10-31T04:07:55.549Z",
  "clock_basis": "clock.curr_time before; executor Date.toISOString immediately around native request/returns",
  "exec_command_arguments": {
    "cmd": "& 'C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe' -NoLogo -NoProfile -NonInteractive -File 'E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.watchdog.ps1' -AllocationSha256 '101a9ff396c41152d34797068dccef92166cadfb1f5fe8b04755983e46c2818a'",
    "shell": "C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe",
    "login": false,
    "workdir": "E:\\AI-Projects\\kilogram-p2p-messenger",
    "yield_time_ms": 10000,
    "max_output_tokens": 4000,
    "sandbox_permissions": "require_escalated",
    "justification": "Один разрешённый synthetic attempt-02 до 30 секунд: блокировка только собственного worker и его завершение supervisor’ом по deadline, без пользовательских данных, ключей и сети."
  },
  "poll_arguments": [
    {
      "session_id": 37439,
      "chars": "",
      "yield_time_ms": 10000,
      "max_output_tokens": 4000
    }
  ],
  "session_id": 37439,
  "cell_id_supplied": false,
  "native_chunks": [
    {
      "chunk_id": "c2b1e3",
      "wall_time_seconds": 10.0098977,
      "session_id": 37439,
      "original_token_count": 0,
      "output": ""
    },
    {
      "chunk_id": "a42ef3",
      "wall_time_seconds": 1.8360254999999999,
      "exit_code": 0,
      "original_token_count": 24,
      "output": "ColdA blocked-worker expected external cutoff returned; root actual-return review required.\r\n"
    }
  ],
  "actual_app_item": {
    "type": "commandExecution",
    "id": "exec-40473da0-45bb-4ef7-88b3-fe010b3300e0",
    "command": "\"C:\\\\Users\\\\Paul\\\\.cache\\\\codex-runtimes\\\\codex-primary-runtime\\\\dependencies\\\\native\\\\powershell\\\\pwsh.exe\" -NoProfile -Command \"& 'C:\\\\Users\\\\Paul\\\\.cache\\\\codex-runtimes\\\\codex-primary-runtime\\\\dependencies\\\\native\\\\powershell\\\\pwsh.exe' -NoLogo -NoProfile -NonInteractive -File 'E:\\\\AI-Projects\\\\kilogram-p2p-messenger\\\\ai-docs\\\\tasks\\\\long-lived-dialog-a3-migration-s5-handoff-evidence\\\\C-s53-cold-core-blocked-worker-01-attempt-02.watchdog.ps1' -AllocationSha256 '101a9ff396c41152d34797068dccef92166cadfb1f5fe8b04755983e46c2818a'\"",
    "cwd": "E:\\AI-Projects\\kilogram-p2p-messenger",
    "status": "completed",
    "exitCode": 0,
    "durationMs": 20236,
    "output": {
      "text": "ColdA blocked-worker expected external cutoff returned; root actual-return review required.\r\n",
      "truncated": false
    }
  },
  "request_to_final_seconds": 22.355,
  "actual_app_duration_ms": 20236,
  "durations_note": "Chunk wall times are individual tool waits, not the full process duration. Request-to-final includes overhead; parent/app timers overlap and are not added.",
  "inputs_before": [
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-cold-preservation.ps1",
      "bytes": 80889,
      "sha256": "e4c426dafc68e1f727c51db9164aefd90efec8c607326fda9f8f2d72063f2e8e"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.preflight.ps1",
      "bytes": 69276,
      "sha256": "424e0ef4c03bb6c5a9950e042f1ffe6c3976de719fd520abd86300f6633c5c39"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.watchdog.ps1",
      "bytes": 37619,
      "sha256": "5b1c7dba8d6fe13e92a8196b955ba4b848a1f105c974338f65a896541e7151d2"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.source-review.md",
      "bytes": 4279,
      "sha256": "92143794bf9f109b05cac68b4526319a11236e515657287094736c7ec658d1be"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.root-allocation.json",
      "bytes": 1167,
      "sha256": "101a9ff396c41152d34797068dccef92166cadfb1f5fe8b04755983e46c2818a"
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
  "inputs_after": [
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-cold-preservation.ps1",
      "bytes": 80889,
      "sha256": "e4c426dafc68e1f727c51db9164aefd90efec8c607326fda9f8f2d72063f2e8e"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.preflight.ps1",
      "bytes": 69276,
      "sha256": "424e0ef4c03bb6c5a9950e042f1ffe6c3976de719fd520abd86300f6633c5c39"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.watchdog.ps1",
      "bytes": 37619,
      "sha256": "5b1c7dba8d6fe13e92a8196b955ba4b848a1f105c974338f65a896541e7151d2"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.source-review.md",
      "bytes": 4279,
      "sha256": "92143794bf9f109b05cac68b4526319a11236e515657287094736c7ec658d1be"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.root-allocation.json",
      "bytes": 1167,
      "sha256": "101a9ff396c41152d34797068dccef92166cadfb1f5fe8b04755983e46c2818a"
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
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.started.json",
      "bytes": 1032,
      "sha256": "2f15d89b3fe227678a607145d5afd26828589f56e2c92d2175b9083b3356694e"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.witness.json",
      "bytes": 7263,
      "sha256": "15c78cb1c7dc973a922a7c2bead0287ad66ccaeb07ac688292ecafb40bd6ab6c"
    },
    {
      "path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-blocked-worker-01-attempt-02.watchdog.json",
      "bytes": 4699,
      "sha256": "f560daa2210263b5533a2fef69e76a6394d199299e8a7ce217d10f3151879770"
    }
  ],
  "native_report_total_bytes": 12994,
  "report_caps": {
    "each": 65536,
    "total": 196608,
    "within": true
  },
  "observed": {
    "native_exit": 0,
    "parent_result": "observed_expected_forced_timeout",
    "primary_reason": "work_expired",
    "primary_mechanism": "supervisor_work_cutoff",
    "primary_cause": {
      "type": "System.InvalidOperationException",
      "hresult": -2146233079,
      "native_error": 0,
      "marks": {
        "c2q_code": "work_expired"
      }
    },
    "forced": true,
    "guardian_forced": false,
    "cleanup_kill_attempted": false,
    "child_pid": 48792,
    "child_started_utc": "2026-10-31T04:07:35.9374971Z",
    "retained_process_handle": 2292,
    "child_alive_before_kill": true,
    "kill_returned": true,
    "child_exit": -1,
    "child_reaped": true,
    "stdout_eof": true,
    "stderr_eof": true,
    "stdout": "",
    "stderr": "",
    "supervisor_cleanup_unknown": false,
    "parent_events_before": {
      "slots": 2,
      "returned": 2,
      "close_attempts": 0,
      "closed": 0,
      "remaining": 2,
      "cleanup_unknown": false,
      "records": [
        {
          "role": "ready",
          "returned": true,
          "created_new": true,
          "close_attempted": false,
          "closed": false
        },
        {
          "role": "block",
          "returned": true,
          "created_new": true,
          "close_attempted": false,
          "closed": false
        }
      ]
    },
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
    "public_inputs_before_close": 8,
    "managed_inputs_disposed": true,
    "stdio_disposed": true,
    "registered_child_released": true,
    "observed_owned_child_processes_after": 0,
    "owned_process_absence_basis": "witness job children0; no spawn path after witness; retained worker exit/reap/EOF",
    "reached_witness_valid": true,
    "worker_apartment": "MTA",
    "worker_job_children_before_block": [],
    "witness_fields": 50,
    "parent_fields": 63,
    "last_core_fields": 38,
    "last_core_remaining": 32,
    "last_core_input_handles_released": false,
    "last_core_peak": 33,
    "last_core_path": 7024,
    "last_core_record": 12086928,
    "last_core_inventory_bytes": 3586,
    "worker_final_core_observation": null,
    "worker_finally_completion_observed": false,
    "worker_close_count": null,
    "worker_application_cleanup_state": "not_observed_forced_exit",
    "os_release_basis": "observed process exit and Windows handle-release semantics; not per-handle instrumentation",
    "timeline": {
      "ready_qpc": {
        "qpc": 2697555364099,
        "age_seconds": 2.9211177
      },
      "witness_accepted_qpc": {
        "qpc": 2697557327058,
        "age_seconds": 3.1174136
      },
      "cutoff_qpc": {
        "qpc": 2697726556395,
        "age_seconds": 20.0403473
      },
      "kill_before_qpc": {
        "qpc": 2697726566811,
        "age_seconds": 20.0413889
      },
      "kill_after_qpc": {
        "qpc": 2697726573468,
        "age_seconds": 20.0420546
      },
      "child_exit_observed_qpc": {
        "qpc": 2697726828020,
        "age_seconds": 20.0675098
      },
      "both_eof_qpc": {
        "qpc": 2697726877151,
        "age_seconds": 20.0724229
      }
    },
    "parent_observed_seconds": 20.0945091
  },
  "boundaries": {
    "attempt01_replayed": false,
    "source_modified_after_acceptance": false,
    "repair_during_consumed_attempt": false,
    "retry": false,
    "extra_tmp_or_event_probes": false,
    "private_access": false,
    "network_access": false,
    "Rust_or_product_runtime": false,
    "ledger_modified_by_executor": false,
    "accounting_owner": "root after actual review",
    "cooperative_worker_timeout_coverage": false,
    "hard_guardian_firing_coverage": false,
    "whole_attempt_verdict": "not_established",
    "commit_push": false,
    "s54": false
  },
  "stop": "Await root actual review; no repeat, repair, commit or push."
}
```
