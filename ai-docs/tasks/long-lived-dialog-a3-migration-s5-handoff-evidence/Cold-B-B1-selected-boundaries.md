# B1 selected implementation boundaries

2026-10-31. Executor implementation under the final root contract, parent
300d0c6afa59f17e7c6f8729f42c4c614d0609fa. No private activation.

- Add the nondefault CLI feature `verification-s53-cold`, forwarding the existing
  state adoption feature, and one dedicated `migration_s53_cold` integration
  harness. Keep readers, history pagination, fixed expectations, ownership worker
  and invocation wiring in separately readable support modules.
- Readers borrow already-open L1Repository objects. Reconstruct original signed
  scope without live controls; use protocol root-verified credentials and
  ParsedReceipt1::verify_pre_q, stored projection/auth/proof bytes, and local-only
  request checks. Repeat both B0 and B1 digests without reopening.
- A small verification-gated state child module returns the actual lease via
  core.into_owner_lease after database close. Its module declaration is an exact
  new hunk in the already-dirty live1.rs; preserve its previous changes and record
  the before-image. Ordinary acquisition/loader/cryptography remain unchanged.
- Extend held preservation through separate fixed private factory/worker wiring,
  one absolute deadline and pinned materialized composed source/assembly. Keep
  synthetic fixture constraints separate. Full changed stamps stay in bounded
  protected evidence; pipe responses stay at most4KiB.
- Focused test premises: a genuinely verified small original signed scope and
  source/receipt is the positive control before receipt/auth mutation; valid
  signed authority/account/floor is the control before account/revision mismatch;
  peer source does not require a local request; future request presence must
  trigger the exact absence invariant. No new511-message fixture.

Compile-only locked/offline, stable runner, Jobs2/BelowNormal, bounded command180s
and aggregate C480. No original S/O/BKP/R/E reads, G1, commit or push before root
review. Submit current build-input pins, exact introduced hunks and concrete
combined private invocation with cleanup/failure classification.
