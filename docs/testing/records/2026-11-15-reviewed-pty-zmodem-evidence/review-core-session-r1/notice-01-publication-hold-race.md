# P2: publication hold can regress to false across a late unknown result

Source baseline: 721bad2157da8d3a0bc11a1eab5ac59b6616aa9c; original frozen-inputs-r1.json.

Location: `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem.rs:1149` (known), also 1131, 1158-1163 and 424.

A timed-out/dropped publication retains its PublicationTicket in the detached task result. RunningOnDrop clears `running` before that result/ticket is disposed. `feed` admits another transfer once `running` is false. A new successful publication and the old detached result can therefore dispose their tickets on different threads with shared `pending` and `unresolved` atomics.

Allowed interleaving:

1. New success enters `known()`, finishes its reservation and reads `unresolved == false` for line 1150; suspend before its pending store.
2. Old abandoned result drops its mutated ticket. Lines 1158-1163 set `unresolved = true`, quarantine the old reservation, and store `pending = true`.
3. New success resumes its previously evaluated `pending.store(false, Release)`.
4. Both workers have stopped; `unresolved == true`, but the handle returned by `mutation_pending()` reads false indefinitely.

Acquire/Release does not make the load and store across two atomics a transaction. The public contract at lines 377-381 promises that a later success never erases an earlier unresolved outcome; this interleaving violates that contract. Backend quarantine remains intact. The current app also has sticky uncertainty epochs (`terminal_zmodem.rs:43-56`), so this notice does NOT claim a demonstrated native close-protection bypass.

Recommendation: preserve unknown holds monotonically across concurrent late ticket disposition, or derive the exposed hold from synchronized publication/unknown state; cover a late old unknown racing a new known completion. Apply the same reasoning to the load/store in PublicationTicket::drop (1162-1163).

Evidence type: source-level reachable interleaving; no independent rebuild, no production regression binary for this new scenario, no source edits. This finding is separate from Debian peer ZFIN/OO failure diagnosis.
