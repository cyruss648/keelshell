# ADR 0112: Reviewed and bounded remote Linux traceroute

Date: 2026-11-06. Status: frozen r3 version-stream correction independently
APPROVE_FOR_SOURCE_ONLY at 2026-11-06T04:04:14Z, with zero open findings.
Parent final combined r3 engineering gate, normal application build and macOS
package generation/inspection passed with exit 0 and actual reap receipts.
Native UI, Windows/Linux platform acceptance, Release and installation remain
pending. R2's historical source-only approval did not clear the subsequently
observed real-tool defect.

## Context

The existing reviewed network diagnostics need a small traceroute operation with
explicit destination, protocol, IP family and limits. Arbitrary command templates,
automatic installation and broad tool/version discovery would expand the reviewed
authority and parser surface. Existing ICMP, TCP, DNS, TLS and HTTP behavior must
remain separate.

## Decision

Add validated immutable traceroute options to core, typed request-bound results
and the fixed remote adapter to session, and preview/confirm/cancel controls to
the existing app protocol panel. Preserve the UI/core/session/AI layers, bilingual
semantic theme tokens and no autonomous AI execution. Add no dependency.

Admit only Dmitry Butskoy Linux traceroute 2.1.5's exact version/output contract:
`--version` must exit zero, stdout must be empty and stderr must exactly equal
the fixed two-line version bytes. Do not concatenate streams or accept substrings.
Inspect the fixed paths `/usr/bin/traceroute`, then `/bin/traceroute` only when the
first path is absent. Other implementations, versions, options and denied
permissions remain explicit capability results. Do not install, escalate, change
remote configuration or fall back to another address/tool.

Require explicit UDP/ICMP and IPv4/IPv6, 1–16 hops, 1–3 probes per hop and
100–1000 ms per probe. Their cumulative requested wait is at most 6000 ms.
Use one first remotely resolved numeric address in the selected family.
The preview explains that this address is obtained after confirmation.

The SSH command carries a fixed supervisor script and an encoded request.
Validate again remotely. Start the actual tool with `shell=False` and a fixed
argument array: numeric family, `-n`, `-M default|icmp`, `-f 1`, bounded `-m/-q`,
`-N 1`, bounded `-w`, `-z 0`, `--`, numeric address and fixed total size 60.
Traditional UDP starts at port 33434 and increments per probe; no scan range is
exposed.

Keep the existing eight-second remote and ten-second SSH envelopes. Version,
resolver and traceroute stages share `started + 7.5s` and 32768 bytes of combined
stdout/stderr. Reuse direct child/process-group ownership and up to 0.4 seconds of
actual bounded kill/wait. Signals record cancellation intent without interrupting
the spawn/ownership handoff. Closed output pipes alone do not establish process
exit. CleanupUnknown preserves uncertainty and retires diagnostic authority.

Parse only numeric addresses, measured RTT, admitted ICMP markers and complete
sequential hop rows. Keep stars unknown and discard an incomplete trailing row.
Rust separately verifies request limits, family, timing, destination and
status/reason consistency. Destination response, hop limit, reported
unreachability and supervisor timeout are distinct. Do not infer packet loss,
zero RTT or host-offline status from missing responses.

Bind preview, dispatch and adoption to the real captured connection/profile,
saved route/trust, active pane/session selection token and exact draft revision.
Any edit or authorization change revokes old consent, including switching away
and back. Cancelled or stale completion cannot restore authority. Missing
verifiable SSH termination also disables this diagnostic authority.

## Frozen-r2 review correction

Archimedes's original r1 review retained one P1 and one P2. R2 changes only four
Rust paths: explicitly import `gpui_kit::prelude::FluentBuilder` for the new
`when` call, and replace 20 denied `expect/expect_err` test sites using existing
checked/diagnostic extraction. Preserve assertions, workspace denies and all
17 new declarations; introduce no lint suppression.

The independent r2 necessary-delta report is `APPROVE_FOR_SOURCE_ONLY`.
TR-R1-P1-01 and TR-R1-P2-01 are closed in frozen r2, with no open findings.
The r1 archive, failed attempts and original review remain historical evidence.
Source approval permits parent combination validation; it does not sign the
combined source, full gate, package or native behavior.

## Frozen-r3 real-tool correction

The parent's owned Linux image exposed a real r2 defect: upstream 2.1.5 sends its
110-byte two-line version to stderr, with empty stdout and exit zero. R2 instead
required the bytes on stdout and rejected any stderr. The original fake tool
repeated that mistaken stream contract, so r2's offline success did not detect it.
R2 is not a release candidate.

R3 changes only the version predicate, faithful fake version output and focused
positive/negative cases. A new exact-stderr positive test fails on the old r2
production predicate and passes after the correction. A separate negative test
rejects stdout-only, both streams, a split version, extra/missing bytes, nonzero
exit and other versions/implementations before resolver/probe dispatch.
The full offline suite has 14 passing tests, including those two additions.
Parser, request admission, command arguments, budgets and authority are unchanged.

R3 also includes exactly the parent-provided `#![cfg(test)]` plus a blank line
at the start of the Rust traceroute test file. No Rust test semantics change.
The parent's separate fullgate-r1 remote-only guard failure and Gauss review
remain parent-owned evidence. The author did not run Cargo for r3 and does not
carry forward the r2 Rust/Clippy receipts as freshly executed r3 validation.

The parent raw-tool probes independently observe one-hop IPv4/IPv6 UDP loopback
and the existing exact ICMP permission errno in its unprivileged image.
That evidence requires no parser expansion and does not sign the SSH adapter,
review UI, complete cancellation or native behavior. The later r3 source-only
approval and the parent's final combined full gate/build remain separate.

A later parent red/green run executes the exact production Python script
(`icmp + trace + probe`) with the actual worker in the owned Linux image,
network none, UID 1001, read-only root and no capabilities. The old predicate
returns UnsupportedEnvironment; r3 UDP IPv4/IPv6 each return one-hop success
and ICMP returns PermissionDenied. Saved process waits and container removals
all exit 0/reaped. This extends evidence to the production Python adapter and
worker; Rust response parsing, SSH, review UI and native acceptance remain open.
The targeted record retains both the red and green originals.

## Consequences and boundaries

The narrow whitelist intentionally rejects otherwise capable hosts until another
precise contract and isolated real-tool evidence exist. Version text is a
capability assertion from the authenticated remote, not executable
cryptographic attestation. IPv6/ICMP source branches do not prove kernel or
privilege support.

Author r2 validation actually passed 17 new Rust tests and 37 existing
regressions, with four existing environment tests ignored, plus strict workspace
all-targets Clippy. These include controlled SSH, offline owned processes and
GPUI controllers. The macOS production supervisor test honestly returns
UnsupportedPlatform; it does not execute Linux traceroute.

The parent's earlier combination of editor r3 and traceroute r2 was followed by
a separate 543-input, 21-source-path combined r3 freeze that imports the two
corrected Python files; the test prefix already matched. It retains the parent's
core/lib.rs merge. Its final source review, gate and package results remain
parent-owned. The author read back the parent's final gate originals:
2355 ordinary passed / 23 ignored, 14 rustdoc passed, and Python 131 run /
130 passed / 1 skipped. The app subset is 1047 passed / 2 ignored.
All 42 combined new names, including this slice's 17, and 204 selected baseline
names passed; these counts are subsets of the gate, not additions to its totals.
The 14-test traceroute Python suite retains its separate scope and receipt.

The parent's normal debug application build, macOS aarch64 package and static
inspection each exited 0 and were actually reaped. The package manifest records
application SHA-256
`9e83d83af253ffe3c3aba29997450ea75f4c70a6dbe9311e61469228bd0158b9`.
These engineering and artifact results are separate from Archimedes's isolated
r3 source-only approval and from native/runtime acceptance. Native macOS UI,
Windows/Linux platform acceptance, Release and installation remain pending here;
the parent owns subsequent native records. The author's earlier
536-input preservation fact is scoped to its execution window and release time;
later authorized parent assembly is not historical drift.

See the [product contract](../product/TRACEROUTE_DIAGNOSTICS.md) and
[targeted validation record](../testing/records/2026-11-06-traceroute-diagnostics.md).
