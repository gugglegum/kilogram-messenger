## Summary

Describe the problem, the chosen boundary, and the user-visible result.

## Security, privacy, and compatibility

Describe effects on cryptography, identity, authorization, metadata, local
state, transport, recovery, abuse resistance, compatibility, and migration.
Write `None` only after considering each applicable boundary.

## Verification

List exact checks performed, relevant results, and checks deliberately omitted.

## Evidence and documentation

State whether an RFC, project memory, accepted field evidence, candidate
record, or reproducibility record changed or became stale.

## Checklist

- [ ] The change is focused and does not include unrelated formatting or artifacts.
- [ ] No real secret, IPC bearer, private key, seed phrase, personal message, or field-test state is included.
- [ ] Security-sensitive details are not disclosed through a public pull request.
- [ ] Formatting and relevant tests pass, or omitted checks are explained above.
- [ ] Documentation and RFCs are updated where the contract changed.
- [ ] Existing evidence is not silently reused after its protected surface changed.
- [ ] I agree to provide this contribution under `AGPL-3.0-or-later`.
