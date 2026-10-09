# ADR 0067: Explicit remote protocol diagnostics

Date: 2026-10-06. Status: implemented candidate; independent integration review pending.

## Context

The existing listening-socket view and `nc -z` TCP probe cannot prove DNS resolution, verified TLS negotiation or an HTTP response. Protocol operations must originate at the authenticated SSH target, remain explicit and bounded, and fit the current bilingual themed monitor without installing remote tools or sending diagnostics to AI providers.

## Decision

Keep immutable input admission in `keelshell-core`, fixed SSH execution and strict typed reports in `keelshell-session`, and review/state/rendering in `keelshell-app`. Use existing workspace serde dependencies; no additional registry or Git dependency is required. Run Python 3.8+ standard-library code through the captured SSH session in a POSIX environment. Missing tools are actionable capability states, not local fallback results.

Use the remote OS resolver, a verified default TLS context and exactly one credential-free HTTP HEAD. No insecure certificate retry, redirect, body, proxy, cookie or authentication is introduced. Return only bounded address, certificate, status and monotonic timing fields. Request validation prohibits shell syntax, control characters and credential-bearing or query/fragment URLs. Fixed code receives serialized base64 data. Strict wire validation rejects malformed phase identities and reflected unsafe text before display.

The eight-second wall-clock budget must survive a blocking C resolver. [Python's signal documentation](https://docs.python.org/3/library/signal.html#execution-of-python-signal-handlers) explains that callbacks can be delayed by C work. An actual controlled SSH/Linux/DNS stall reproduced this: the first implementation reached the ten-second SSH envelope instead of returning a remote timeout. Preserve that original failure. Replace signal-only enforcement with a remote supervisor and a separate owned process group. `communicate(timeout)` alone does not kill the child, as documented by [Python](https://docs.python.org/3/library/subprocess.html#subprocess.Popen.communicate); terminate the exact group and actually wait. Only confirmed exit permits a timeout/interrupted result. Unknown cleanup is a distinct fail-closed state. An owned kill-denial counterexample proved that signalling failure must still reach bounded actual wait and preserve unknown exit, rather than become unsupported capability. Shutdown handlers record intent without throwing across child creation or reap; repeated signals cannot abandon the owned child between creation and assignment.

The transport checks cancellation after channel open and before exec, bounds the SSH envelope to ten seconds, limits combined stdout/stderr and uses existing owned-channel cleanup. Cancellation discards results and closes that channel, without claiming immediate remote process death. The supervisor independently bounds its worker. Loss of the remote host or forcible termination of the supervisor cannot be turned into cleanup proof.

The monitor offers DNS/TLS/HTTP mode selection, endpoint entry, an exact-target preview, explicit confirmation, cancellation and result cards. Saved route/trust and captured connection identity are rechecked before dispatch and adoption; profile sync/save and reconnect polling retire stale diagnostics. Both initial connection and the actual successful reconnect installation bind that authority; a production-path stale-route preview counterexample verifies the latter. Retirement compares the child’s captured connection to the current transport, rather than comparing a map entry to itself. Language/theme changes only rerender. History is memory-only and no model/telemetry path is connected.

## Alternatives and consequences

Local DNS/TLS/HTTP libraries would observe the desktop network and are unsuitable for remote diagnosis. Renaming the TCP probe would claim a protocol it never spoke. A curl/openssl/dig adapter could serve some hosts but would need separate version/output/timeout contracts; it is not an automatic fallback or installation path. A worker thread cannot reliably interrupt `getaddrinfo`, while signal-only enforcement was disproved by the actual counterexample.

POSIX Python is the initial explicit capability boundary. Remote Windows, HTTP/2/3, custom CA/client certificates, authenticated/custom requests and UDP remain open. Test services use isolated keys, local listeners and an offline owned Linux container; GPUI scenes remain presentation and interaction evidence, separate from native desktop acceptance. See the [product guide](../product/REMOTE_PROTOCOL_DIAGNOSTICS.md) and [test record](../testing/records/2026-10-06-remote-protocol-diagnostics.md).
