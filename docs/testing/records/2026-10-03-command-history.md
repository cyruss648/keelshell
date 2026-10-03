# Remote command history slice — 2026-10-03

## Scope

The command tool panel now records commands after the remote transport accepts
them into its bounded send queue. History is isolated by SSH terminal tab,
shown newest first, can be placed back into the review field, and can be
cleared for the active session. It is memory-only by design: command text can
contain tokens, passwords or other sensitive arguments and must not enter the
persisted connection state.

The history model keeps at most 200 entries per tab, ignores blank input,
removes only trailing transport line terminators, and collapses consecutive
duplicates. Insertion still binds the command to the currently selected tab;
the command is never executed automatically by selecting a history row.

## Verification

Commands executed from the repository root:

```sh
cargo fmt --all -- --check
cargo test -p keelshell-app --all-targets --locked
cargo clippy -p keelshell-app --all-targets --locked -- -D warnings
```

Results:

- Formatting passed.
- Application tests passed: **56 passed, 0 failed, 0 ignored**.
- Clippy passed with `-D warnings` for all application targets.
- Unit tests cover blank input, consecutive duplicate collapse, multiline
  commands, newest-first ordering, the 200-entry bound and clear semantics.
- The GPUI workspace test queues a reviewed command on the right-hand remote
  pane, verifies it is recorded for that tab, opens the Commands panel, uses
  the history insertion control and verifies the review target remains bound
  to the same pane.

## Boundaries

This slice does not persist history across application restarts, synchronize
history between devices, provide shell-aware completion, or execute commands
without review. It also does not infer whether a command contains a secret;
memory-only retention is the safety boundary for this iteration.
