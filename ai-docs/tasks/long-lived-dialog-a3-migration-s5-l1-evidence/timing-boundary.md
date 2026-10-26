# Timing boundary of contributing records

Existing immutable `elapsed_seconds` values measure the runner from its stopwatch
start through setup, inventory, compile/stage if nested, fixture/runtime work,
waits/recovery/owned-child cleanup, source/image verification and printing its
retained log tail. The stopwatch stops before the terminal case/aggregate JSON
publication. Those final writes are not already included in that measurement.
No old attempt value, failure, charge or classification is rewritten here.

For bounded attempts the independent Windows-job whole deadline remains armed
through those final writes and is disarmed only after child cleanup and successful
publication. Its enforced boundary therefore differs from the recorded stopwatch
measurement. A started marker alone is not a completed result and cannot be reused.

For `F-obligations-resume-01`, the execution-tool outer observation was9.9028161s,
versus runner9.6329237s, a difference0.2698924s. This mixed difference includes
invocation/bootstrap/publication/teardown; it is not an isolated measurement of
JSON writing. Both original measurements are retained without conflating them.
Subsequent outer observations are supplemental evidence with the same limitation.

The aggregate retains its historical charged-record arithmetic. Any additional
allocation or adjustment remains an explicit root decision; an outer interval is
not silently substituted for or added twice to an already charged inner interval.
