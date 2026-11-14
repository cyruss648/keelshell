# Independent strict read-only review — named tunnel runtime recovery

Date: 2026-11-14 (Asia/Shanghai)

## Frozen input

- Product baseline: `fd4cc894aa876f23cde49698b693779e81027489`
- Reviewed implementation SHA: `7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`
- Exact SHA check: pass; `fd4cc894...` is an ancestor of the reviewed SHA.
- Review worktree: `/Users/hdh/.codex/worktrees/6711/keelshell`
- Independent Cargo target: `/Users/hdh/.codex/worktrees/6711/keelshell/work/review-final-7e6ecb3/target`
- Toolchain: `rustc 1.100.0 (0ef1bc386 2026-11-09)`, `cargo 1.100.0 (3acc79c0c 2026-11-05)`

The reviewed source commit was unchanged during review. The worktree inherited uncommitted documentation files from the author worktree; no source, test, Cargo, or documentation file was edited by this review. Review artifacts are confined to `work/review-final-7e6ecb3/`.

## Finding counts and recommendation

| Severity | Count |
| --- | ---: |
| P0 | 0 |
| P1 | 0 |
| P2 | 0 |
| P3 | 0 |

Recommendation: **APPROVE** the reviewed implementation SHA for merge after the author binds the final documentation commit and preserves this review receipt. The review does not approve native desktop acceptance, real customer SSH, release artifacts, or production/business acceptance.

## Reviewed behavior

- `crates/keelshell-core/src/tunnel_runtime.rs:50-175` keeps ordered route/profile identity, auth category, and opaque authentication-target fingerprints without persisting passwords, private-key contents, private-key paths, vault IDs, proxy passwords, sessions, sockets, workers, commands, or traffic.
- `crates/keelshell-core/src/tunnel_runtime.rs:245-340` validates bounded snapshots, preserves unknown nonzero snapshot versions for storage, and rejects unsupported versions during recovery assessment.
- `crates/keelshell-core/src/tunnel_runtime.rs:422-538` compares the current rule, connection, route, jump graph, auth category, auth target fingerprint, state validity, cleanup status, revision, and review token before admitting a fresh start. The confirmation result has no transport execution capability.
- `crates/keelshell-app/src/tunnels.rs:32-95` represents runtime persistence as explicit panel events. `crates/keelshell-app/src/workspace.rs:3205-3292` rechecks ownership, state revision, conflicts, and storage admission before dispatching `StateStore` work through the existing persistence path.
- `crates/keelshell-app/src/tunnels.rs:850-1020` records a candidate only after the worker reports its actual listener, marks unresolved cleanup as `CleanupUnconfirmed`, and removes a candidate only after confirmed cleanup persistence succeeds.
- `crates/keelshell-app/src/tunnels/rules.rs:42-272` keeps recovery candidates read-only until Inspect, Review start, state-bound token confirmation, and explicit Confirm and start fresh. Discard is an explicit persistence action.
- `crates/keelshell-app/src/tunnels/rules.rs:1207-1385` renders the bilingual warning card with Inspect, Review start, and Discard, using the existing palette/token path without theme-ID branching and without online indicators.

## Test and gate evidence

All commands below used the independent target above, `--locked`, and `--offline` where applicable.

- `cargo test -p keelshell-core --test tunnel_runtime --locked --offline -- --nocapture`: exit `0`; **8 passed, 0 failed, 0 ignored**.
- `cargo test -p keelshell-app --bin keelshell-app --tests --locked --offline tunnel_rule_tests -- --nocapture --test-threads=1`: exit `0`; **19 passed, 0 failed, 1127 filtered**.
- `cargo test -p keelshell-app --bin keelshell-app --tests --locked --offline tunnels -- --nocapture --test-threads=1`: exit `0`; **22 passed, 0 failed, 1124 filtered**.
- `cargo test -p keelshell-core --test tunnel_runtime --locked --offline -- --list`: exit `0`; **8 tests listed**.
- `cargo test -p keelshell-app --bin keelshell-app --tests --locked --offline -- --list`: exit `0`; **1146 tests listed**.
- `cargo fmt --all -- --check`: exit `0`.
- `cargo clippy -p keelshell-core --all-targets --locked --offline -- -D warnings`: exit `0`.
- `cargo clippy -p keelshell-app --bin keelshell-app --tests --all-features --locked --offline -- -D warnings`: exit `0`.
- `git diff --check fd4cc894aa876f23cde49698b693779e81027489..7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`: exit `0`.

The app test link step emitted the known macOS linker warning `__eh_frame section too large`; it did not change any command exit code or test result.

## Unverified boundaries

- No native macOS window-level visual/scroll/theme or accessibility acceptance was performed.
- Windows/Linux native windows, IME, and accessibility were not exercised.
- No real customer SSH, jump-host, credential, host-key, or cross-process listener detection was performed.
- No Podman integration, power-loss durability, CI runner, signing, notarization, installation, upgrade, or release artifact validation was performed.
- The implementation intentionally cannot discover an old listener in another process; it retains a possible-running observation and requires human re-review.
- The author full `scripts/check.py` result was not independently rerun in this review; the focused and static gates above are the independent evidence.
- The review did not independently validate the author's uncommitted documentation files; those must be updated after the final local implementation commit and must retain the exact SHA/target/log bindings.
