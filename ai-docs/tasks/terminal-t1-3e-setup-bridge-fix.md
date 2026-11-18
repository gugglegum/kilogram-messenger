# T1.3e: Auto LAN setup bridge compatibility

The protected setup bridge previously rejected `route_lan_auto` and omitted public `connection.lan_discovery` status. The bridge now accepts exactly two canonical IPv4 arguments and validates/projects the manual or Auto discovery shape. Existing status frames without discovery remain supported. Private fields and child stderr remain filtered.

Root reviewed the source delta and independently passed 21 Auto LAN process scenarios and 10 existing protected setup scenarios under Windows PowerShell 5.1. Executor additionally checked the pinned CLI in a fresh unopened setup: the route request returned `not_open`, then quit completed. That smoke does not prove a configured live Auto route.

The t13e06 two-host field run failed earlier, during B account creation, before any phase A-D. Both administrative readiness gates passed and final cleanup restored original images and firewall state. A separate synthetic CurrentUser DPAPI probe failed on B over pinned SSH with Win32 5; A passed. The original helper failure did not preserve its Win32 code, so the probe is supporting evidence, not proof of that original error. DPAPI/session diagnosis and the full two-host field acceptance remain open.

This bridge fix does not change application cryptography, key storage, deployment or firewall policy. Historical deployed bridge and failed-run evidence are preserved.
