# RFC-0120: Configured relay evidence (M0.9.97)

Status: fresh two-host reduced field run, candidate refresh and local readiness
verified; hosted confirmation pending.

## 1. Problem found by the fresh field run

The two-host reduced run `20261014-135100` used revision
`78164eca9ec3f0ccc6b3cb12e72613b7cd6052ac`. Its application lifecycle
completed: two distinct volunteer stores accepted the encrypted item, Alice
stopped before Bob received it, Bob committed one message, deleted both
replicas and observed no redelivery after restart.

The evidence verifier correctly rejected the run. Provider 1 was configured
with the canonical APS1 relay but did not register a relay home within 30
seconds and continued over a direct path under `route_policy=auto`. Its closed
stdout therefore contained no `relay_home_url`, while stderr recorded the
bounded relay timeout. The old verifier incorrectly used observed relay-home
registration as the only evidence of configured relay pinning.

## 2. Diagnostic contract

When a runtime is launched with an explicit `--relay-url`, it now emits
`relay_config_url=<canonical URL>` after endpoint binding and before waiting
for online relay registration. This line reports immutable launch
configuration, not connectivity. Existing diagnostics retain their meanings:

- `relay_config_url` is the explicitly configured relay allowlist/pin;
- `relay_home_url` is emitted only for an actually observed online home relay;
- `relay_status=timeout ...` remains a warning for `auto` and a hard failure
  for `relay-only`;
- `route_policy=auto` may legitimately continue on a direct path.

No routing, endpoint, timeout, retry, mailbox or cryptographic behavior is
changed.

## 3. Evidence verifier

Every endpoint-bearing closed runtime log (providers, capability convergence,
send, receive and restart) must now prove the exact configured APS1 relay
through one `relay_config_url`. Any observed `relay_home_url` or
`target_relay_url` must still be that same canonical URL; a divergent value
fails closed. Actual delivery, distinct store/transport identities, exact
2-of-2 receipts, sender-offline ordering, commit-before-delete and restart
non-redelivery remain mandatory.

This distinction permits an honest two-host reduced run to use a direct path
for one provider while preserving proof of its configured relay boundary. It
does not turn such a run into independent-provider acceptance and does not
claim relay availability.

## 4. Local proof and remaining acceptance

The verifier self-test accepts the canonical configuration and rejects a
divergent provider relay configuration. The existing independent-provider
self-test also passes. The exact isolated runtime lifecycle test still passes
`1/1` in 60.64 seconds after the diagnostic change.

Because production CLI output and the protected field surface changed, the
rejected run was not accepted retroactively. A new debug/two-job kit from
technical revision `be80d28ec12b41987bdbfa800016fb86b54b4723` completed
physical run `20261014-141058`: two distinct volunteer stores returned exact
2-of-2 receipts, Alice stopped before receive, Bob committed the message once,
deleted both replicas and observed no redelivery after restart. The verifier
reports `result=verified-reduced-two-host` and
`independent_provider_field_acceptance=false`.

The fresh run exposed two control-plane issues without invalidating the product
lifecycle. The verifier initially still required observed relay registration
for send/receive/restart, and the final wrapper could observe the synchronized
provider-stop marker before both closed provider logs arrived. The verifier now
applies the configured/observed distinction consistently, and the wrapper waits
for both closed stdout/stderr pairs before creating final boundary evidence.
Self-tests cover configured-only success plus divergent configured/observed
relay rejection.

Local candidate composition and complete M1 readiness now pass. Remaining
acceptance requires a fresh green hosted Windows CI run. No step authorizes an
M1 tag or public security-release claim. The reduced two-host run does not
replace the pending three-/four-host independent-provider field test.
