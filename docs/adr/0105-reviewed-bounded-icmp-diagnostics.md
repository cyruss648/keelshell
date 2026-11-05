# ADR 0105: Reviewed and bounded remote ICMP diagnostics

Date: 2026-11-05. Status: candidate source; targeted gates and integration
boundaries are recorded separately. No commit, publication or native acceptance.

## Context

MON/NET-02 needs explicit ICMP echo beside reviewed DNS/TLS/HTTP. A TCP handshake
cannot supply ICMP results. Arbitrary remote command templates, help-derived
option guesses and automatic tool installation would expand the authority and
process surface beyond the reviewed host-only request.

## Decision

Add an immutable `Icmp` request in core, typed counters/RTT/fixed reasons and
strict wire admission in session, and an explicit preview/confirm mode in the
existing bilingual themed protocol panel. Add no dependency. Never dispatch on
edit, preview, language/theme change or automatic monitor refresh.

Admit Linux iputils 20240117's exact version/config output and complete quiet
summary grammar. Fixed C locale and fixed argv include numeric family, no PTR,
a tool count of four, a one-second interval, 56-byte payload and one-second response
wait. Use the first remotely resolved numeric address only. Unknown tools and
versions fail closed; there is no second-address, TCP or local fallback.

The supervisor directly owns three sequential children: version inspection, the
existing Python DNS worker, and ping. Each child has its own process group.
Read bounded pipe chunks using selectors, sharing a 32 KiB raw-output budget
across all stages. Stop runtime at 7.5 seconds and reserve up to 0.4 seconds for
actual kill/wait within the existing eight-second budget. Retain the ten-second
SSH envelope and its separate 32 KiB combined output cap.

Signals set an interruption flag. They never throw across Popen or reap.
Deadline, cancellation and output overflow terminate the owned group and
actually wait. A denied kill with a still-live child yields CleanupUnknown.
The known admitted tools do not spawn descendants; fixture process groups
exercise containment. Stronger proof after supervisor destruction or host loss
is not available. An ICMP channel without a verifiable exit disables diagnostic
authority rather than admitting another uncertain process.

Use counters and validated output together with exit status. The transmitted
counter means utility-counted send attempts, including `hard_local_error`;
it does not prove successful transmission. Quiet local-error paths can return
before incrementing `nerrors`, so a zero error counter does not exclude local
send failures. A difference between counted attempts and unique replies means
unanswered attempts, without identifying local failure or network packet loss.
Keep the existing `IcmpPartial`/`PacketLoss` wire category for compatibility,
document its limited meaning, and do not infer a hidden permission error.

A supervisor deadline has no invented counters. Counts/RTT, status/reason and
all unrelated protocol fields must agree before adoption. RTT describes observed
echo replies. Summary loss-percentage validation checks arithmetic, not an
independent network-loss measurement. Error text is excluded. Offline fake
processes replay both all-local-error and mixed-local-error histories alongside
modeled successful-probe histories with the same quiet summary: the adapter must retain
identical counters/reasons instead of inventing a distinguishable cause.

Observe raw input values, not only normalized requests. Any edit clears review,
changes the adoption token and cancels an in-flight operation. Reverting input
does not restore consent. Before dispatch/adoption, recheck exact input plus the
existing captured-session/route/trust authority. A cleanup-unknown result must
retire authority even when its normal result token has already been invalidated.

The source candidate's permitted write set excluded Workspace, so switching
away and back remained an integration OPEN at its delivery. Parent integration
now provides that minimal selection UUID and synchronous invalidation under
[ADR 0106](0106-diagnostic-selection-consent.md); combined checks and native
acceptance retain their own scope. The original requirement was a minimal
read-only generation contract; existing connection liveness does not prove it.

## Candidate-r2 review correction

The 2026-11-06 scoped review identified ICMP-R1-02: candidate-r1's public
`Actual sends` rustdoc and product loss wording overstated what quiet output
proves. Correct the public counter/status rustdoc, Python comments, offline
counterexample tests and this contract. The adapter's executable behavior,
wire schema, argv, version admission and budgets stay at the reviewed contract.
Candidate-r1 artifacts and gate evidence retain their original hashes.

ICMP-R1-01, app wording, all-Err authority retirement and SSH terminal-frame
regressions belong to the parent's separate app/Workspace increment. This
semantic correction does not sign those changes or the parent's selection
generation implementation, combined gate or native acceptance.

## Alternatives and consequences

Do not rely on experimental JSON or generic regexes matching arbitrary utility
text. Do not set iputils `-w`: the admitted `main_loop` changes its count-stop
condition when a deadline is present. The external supervisor enforces time
without weakening the fixed send count.

Launching ping from the DNS worker would add a grandchild that the supervisor
could kill but could not directly reap. Direct sequential ownership avoids that
topology. Installing raw-socket libraries or new Rust crates is unnecessary.

The narrow version whitelist rejects many otherwise capable hosts. That is an
intentional capability state until another precise upstream contract and
isolated real-tool acceptance exist. Offline fake binaries and controlled GPUI
prove parsing/lifecycle/state behavior; they do not prove actual ICMP, native
desktop behavior or complete product acceptance.

See [product/source contract](../product/ICMP_DIAGNOSTICS.md) and
[targeted validation record](../testing/records/2026-11-05-icmp-diagnostics.md).
