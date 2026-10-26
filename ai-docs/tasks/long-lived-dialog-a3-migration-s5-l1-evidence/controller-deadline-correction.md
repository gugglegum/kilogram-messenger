# Partial continuation controller correction

F-partials-01 remains FAIL (39.6781966s whole contributor,29.08s test).
Its first victim reached genuine phase3 and committed the original two genesis
rows. The frozen A log recorded its fixed15s admission timeout on a stale copied
Locator. The new controller had used18s after queueing; the accepted `wait_old`
helper in `migration_s5_process.rs` uses25s for actual frozen process state.

The controller now uses that same25s bound. Product connect/RPC/admission timeouts,
constructor/state assertions and contributing ceilings are unchanged. Actual
elapsed, including failed attempts and cleanup, remains fully charged. This
correction accommodates the frozen administrative/reconnect sequence and is
independent of the unknown onset of reported CPU contention. A clean focused run
is still required; no historical failure is reclassified or attributed solely to
Ollama.
