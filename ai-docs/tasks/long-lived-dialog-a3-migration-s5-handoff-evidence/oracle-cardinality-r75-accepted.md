# Accepted isolated oracle regression

2026-10-31. Root independently accepted the public-data regression under the simplified [verification workflow](../../verification-workflow.md). This accepts the isolated checker correction only. Whole S5.3 and the native named-stream scenario remain open.

One actual invocation passed all 92 cases: 24 array-shape, 16 baseline and 52 negative cases, covering both frozen checker copies and old/candidate variants. The two OLD directory-pre calls reproduced PropertyNotFoundStrict for Count at worker1041 and supervisor207, column36. The exact three-byte outer-array candidate passed all required cases, with the original 38-field assertions retained.

Runtime was 2.6546224 seconds at the tool, 2.0472004 seconds inside the parent and 5.474 seconds request-through-return; these intervals overlap. Child exit0/reaped, closed streams/process, no timeout/forced termination/unknown cleanup. Root checked the actual app item exec-a72ab63e-8f77-4a11-b777-3b148b920214 and complete matrix (source gate7d19f0; result gatebfdf46). This is one successful functional run, not a speed comparison or fault-injected timeout test.

| Role | Artifact | SHA256 |
| --- | --- | --- |
| Isolated worker | [check](C-s53-pure-oracle-cardinality-01-attempt-01.check.ps1) | 74a0d3dcde7c218ee3d3b538a13ed5159b3431567f6cf4fc997b866de1db958a |
| Ordinary timeout parent | [watchdog](C-s53-pure-oracle-cardinality-01-attempt-01.watchdog.ps1) | d7c6c4b6e372d70eceb97ac82f0e71da4e8536d5ec3bf8635f4018cb3639291e |
| Complete matrix | [cases](C-s53-pure-oracle-cardinality-01-attempt-01.cases.json) | 8f9ca565309031af5fcde6619176b901973458b7ad50d4df8106bf170d900d69 |
| Actual invocation and return | [run](C-s53-pure-oracle-cardinality-01-attempt-01.run.json) | a375f0b12649585983147ac08161aa10ff9293c02f0c6e9e0003bda9c2b49ec6 |
| Frozen source input W | [old worker](C-s53-cold-core-named-streams-01-attempt-01.preflight.ps1) | 2c375e0092ba96784ae72ccaa847e30fe851f3c8c8f26f26108812d7384aac00 |
| Frozen source input S | [old supervisor](C-s53-cold-core-named-streams-01-attempt-01.watchdog.ps1) | 2f4f3fe968d20ef3bd332c853b8c88d8f3b2e673b94f0ba601ce4b3b8a5b34f4 |
| Public data input J | [old cases](C-s53-cold-core-named-streams-01-attempt-01.cases.json) | b7d1be3b9a653d88cff607b76b80c4c3d3cb743e0d7d18c6d9243e3e5ea6dbb4 |

The three old files are frozen text/data inputs. The new check extracts only seven hash-verified pure functions from each source; it does not execute the old controllers or their native/Core helpers. Their failed r73 result is unchanged. The expected directory-final snapshot is synthetic; no new native observation or access to original/private state is claimed.

This small archival package contains the four isolated-test files, three frozen inputs and this note (eight files total). The run record additionally identifies a local preparation note and the installed PowerShell binary; those are provenance references, not worker inputs or extra test dependencies. Keep actual source/report bytes immutable. The recorded attempt refuses output collisions; this is retained evidence, not an instruction to rerun it over existing results. No product code is changed by this publication.
