# P2: Required tests may all be ignored while the runner reports PASS

Observed on 2026-11-15, exact runner SHA-256:
`b23088c77569e0ab9b745597bf30de2a706f2040c0df70e1ce7340d4cf61a299`.
Source baseline: `a092dd5712c0a92ee4af18970fb0500fb230b760`.

Location:
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_local_backend.py:503`
(also named result accounting at lines 249-265 and final PASS at line 519).

Trigger: an inventory still contains the required names, but those tests acquire
`#[ignore]` or platform conditional ignore attributes. The Rust harness exits zero
and emits a named `ignored` result for each selected test. `named_results` accepts
these names; execute only rejects a nonzero exit, timeout or `failed > 0`.

Independent synthetic end-to-end execution of execute (all external commands
replaced; no Cargo, rustc or container) returned **exit 0, status PASS, passed 0,
failed 0, ignored 8** with every required inventory name present. The reviewer
regression expecting failure is red.

Impact: a later platform run can claim successful backend acceptance even though
none of its required behavior executed. Gate at least the required names on an
actual `ok` result, or reject ignored results in these selected acceptance suites.
Add an execute-level regression rather than only testing the result parser.

Evidence: `probes-r1.command.json`, `probes-r1.stdout.log`,
`probes-r1.stderr.log`, `probe-all-ignored-receipt.json`,
`probe_failure_paths.py` in this review directory.

Boundary: the delivered native-r3 raw logs report 251 passed / 0 ignored. This
finding concerns the runner's acceptance rule; it does not reclassify that
historical native result. Current recommendation for these scripts: HOLD.
