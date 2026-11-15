# P2: Cleanup errors can bypass both durable failure receipts

Observed on 2026-11-15, exact runner SHA-256:
`b23088c77569e0ab9b745597bf30de2a706f2040c0df70e1ce7340d4cf61a299`.
Source baseline: `a092dd5712c0a92ee4af18970fb0500fb230b760`.

Location:
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_local_backend.py:530`
(receipt writes at lines 541-544; inherited fallback at
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:245`).

Trigger: an ordinary primary failure starts final cleanup, stop_owned fails to
observe process identities, and its fallback signal_owned raises an OSError
(for example PermissionError from the kernel identity/signal operation).
The inherited Commands.finish catches the first error, but its fallback may
raise again. execute calls finish outside an exception guard and before saving
cleanup.json / receipt.json or restoring the signal handlers.

An independent injected failure through the real inherited finish implementation
raised `PermissionError: Synthetic cleanup signal denied`. Both receipt.json
and cleanup.json were absent. No actual process or container was created by this
probe; the process record and failing observation/signal calls were synthetic.

Impact: the failure can leave no terminal FAIL record or durable cleanup status,
lose the primary error, and skip signal-handler restoration exactly when cleanup
is unconfirmed. Persist the primary outcome before cleanup and protect finish plus
final persistence/handler restoration; retain cleanup failure details and unknown
ownership without claiming success. Add this execute-level error regression.

Evidence: `probes-r1.command.json`, `probes-r1.stdout.log`,
`probes-r1.stderr.log`, `probe-cleanup-error.json`,
`probe_failure_paths.py` in this review directory.

Boundary: this is a reproducible exception-path defect, not a claim that delivered
native-r3 leaked a process or container. Its existing cleanup receipts remain
separate evidence. Current recommendation for these scripts: HOLD.
