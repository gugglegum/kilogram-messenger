# Security Policy

## Project status

Kilogram is experimental software. M1.0.0 is an evidence-backed Windows
technical proof of concept, not a production or public security release. The
project has not received an independent security audit, does not provide a
security response SLA, and must not be used for sensitive or high-risk
communication.

The presence of end-to-end encryption mechanisms does not by itself establish
that the complete application, its metadata behavior, recovery procedures, or
operational environment are secure.

## Supported versions

| Version | Security handling |
| --- | --- |
| Current `master` | Reports are evaluated and fixes may be made on a best-effort basis. |
| `M1.0.0` | Historical technical proof of concept; no backports are promised. Reports are evaluated against current `master`. |
| Older milestones | Unsupported. |

Protocol and state compatibility may change while the project remains
experimental.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting form:

<https://github.com/gugglegum/kilogram-messenger/security/advisories/new>

Do not open a public issue containing exploit details, private keys, seed
phrases, message contents, personal data, or other secrets. If the private
form is unavailable, open a public issue that contains only the sentence
"Private security contact requested" and no vulnerability details.

A useful report includes:

- the affected commit, tag, executable hash, and operating system;
- the expected and observed security boundary;
- the practical impact and required attacker capabilities;
- minimal reproduction steps or a proof of concept;
- sanitized logs and whether the issue is already public;
- any suggested mitigation, if known.

High-priority areas include Account Root or device-secret exposure, seed and
recovery handling, authentication or membership bypass, message
confidentiality or integrity failures, replay or rollback acceptance, local
vault and IPC authorization, relay or mailbox boundary violations, remote
code execution, and remotely exploitable denial of service.

## Disclosure and response

Please allow a reasonable period for investigation and coordinated disclosure.
Receipt, remediation, release, advisory, and CVE timelines are best effort and
are not guaranteed. The project currently offers no bug bounty or monetary
reward. Testing must not access other people's accounts or data, disrupt
third-party infrastructure, or exceed the authorization granted by the system
owner.
