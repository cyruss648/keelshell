# MCP session retirement — 2026-10-07

Current follow-up: the frozen candidate received a fresh non-author scoped
review and was precisely imported with Agent epoch3. The root's combined
full gate and new macOS package checks passed; exact-commit CI, native
clients and concurrent-owner transport checks remain open in the
[main integration record](2026-10-07-reviewed-ai-mcp-main-integration.md).
The author checkpoint below retains its original source/evidence scope.

## Source and scope

Independent author worktree starts from exact main `e69575bc19afe03e80708e03d4993768c8f5dfb0` and its 736 frozen inputs. It does not edit the moving main, the application Agent candidate, shared Cargo targets or their evidence. Production changes bind MCP backend authority to the original terminal's typed lifecycle and synchronously revoke old scope on tab close/completed reconnect. The direct registry dependency policy, lockfile, original tool limits and waits remain unchanged.

No commit, push, tag, launch, installation, VM operation, customer environment or vendor model/client run is performed by this author task. This is not the new non-author review.

## Original confirmed failures

An earlier independent review observed real remote-file changes after each of three target retirement boundaries. `retirement-v1` actual wait 101 / 60.706797208 seconds; two assertion failures / 0.51 test seconds, raw 3746 bytes SHA-256 `8c0f822987fdfd06538f0a51db2e25a2170d324c1fdf73083f476e2ca6a180dc`. `reconnect-v2` actual wait 101 / 14.819600875 seconds; one assertion failure / 0.28 test seconds, raw 2208 bytes SHA-256 `4aa7b33f96aa3cba821250ee5ea63d6a0931b82ab86fc35a1c1e1290d19520d0`. Original PID/PGID35386/38526 were reaped and absent, private TMP empty/removed, no signals/survivors; all736 test-epoch and frozen source maps unchanged.

Each case approved a real existing-file proposal, held its canonical prewrite response, retired the original target before releasing the hold, blocked foreground maintenance and read the complete file over a separate authenticated SSH/SFTP connection. Actual readback changed from `受控中文\n` to the test replacement. Close and completed reconnect asserted actual target removal from tabs/remote_sessions; raw-End asserted is_open=false before UI polling. A controlled saved route in the reconnect case supplies the route/trust binding; its old/new handles are separately authenticated to the owned TCP peer.

## Fixed controlled executions

The original three cases were imported byte for byte into this candidate before the first execution. `original-three-fixed-v1` actual0 / 62.255937875 seconds: all3 passed in6.66 seconds, raw2204 SHA-256 `a97945b6422e75e728bde5835df6592707455e74055d7c7fd7492758916912ac`. All737 inputs unchanged; PID/PGID45452 reaped/absent, TMP removed, no signals/survivors. Close/reconnect were already disabled with OutcomeUnknown before backend resume; rawEnd denied backend authority before foreground status caught up. Complete independent readback preserved the original in all3.

Subsequent formatting preserves the original assertions, bytes and waits. Added raw producer closure, held SFTP read, pending command End/producer closure, held running command and held file preparation regressions. Tests directly observe raw authorization rejection and owned backend completion while foreground callbacks remain paused; no command text is executed by the peer.

Two test-only compilation mistakes are preserved rather than recategorized as runtime failures: v2 used Entity.read with TestAppContext outside a window callback (actual101 /3.985786458 seconds, PID/PGID49149); v4 omitted the App type import for the worker-observation helper (actual101 /3.64606675 seconds, PID/PGID52155). Both738 maps unchanged and owned resources reclaimed. Corrected v3 all26 mcp_ tests actual0 /34.2240225 seconds, raw4482 SHA-256 `7a3ea3042291303992df3869b4a83fbac502252af9df09c1947c19cc97201ad2`; v5 all28 mcp_ tests actual0 /39.903610958 seconds (15.51 test seconds), raw4762 SHA-256 `19fb2ac5b8c1e7bb760cbf08c4cf6b0b6d18de9277fc5d7dfe2489e121fb293d`. v3/v5 all738 inputs unchanged, PID/PGID50047/52943 reaped/absent, private TMP empty/removed, no signals/survivors.

## Frozen author engineering gate

The full gate and all subsequent build/package commands consumed the same741 inputs /15,119,949 bytes. Each before/after map was equal. Full-gate input map SHA-256 `50277c1081ffe3bd98c8fd67fb1460b9f08114474d3f75cf66418fe1c215994d` binds the executed freeze; this record is updated after those executions, with its document-only delta recorded separately. No production or test source changed during or after the gate.

| Owned epoch | Actual wait | Elapsed seconds | PID/PGID | Raw bytes | Raw SHA-256 |
| --- | --- | --- | --- | --- | --- |
| full-gate-v1 | 0 | 667.995861958 | 55709 | 414231 | `554343d323624e228abf68b7f1671c5746f3cb755d9baba0a7c23e6a60cb8deb` |
| native-build-v1 | 0 | 14.656820917 | 74439 | 435 | `793e07685d473e38198f5c272f3d43abad2a08bdf475bcf758d8e25b5ac34f7b` |
| packaging-tests-v1 | 0 | 1.411154875 | 74816 | 248 | `66cc664faf8348e0bf47ed27c19bd46494495807881f6815525beb08417e7357` |
| package-stage-v1 | 0 | 0.344428 | 74912 | 207 | `4159af1e39a562fc6f6ef431eec25e4fb7b55cc4455af58724302c79d1e3fb11` |
| package-inspect-v1 | 0 | 0.244301166 | 75061 | 3765 | `3756423bde58a9ddfc6bd40846d108039f98feaac04a773e4d76d39d5ff7fe35` |

All five leaders were actually waited and reaped; their numeric process groups were absent, private TMP directories empty/removed, and no signals or survivors were observed. `scripts/check.py` passed dependency x.y policy,6 Python tests, formatting and strict all-target workspace Clippy. Rust results were1620 ordinary tests plus8 doc tests,0 failed and22 ignored. This includes591 application tests (2 ignored),20 local-agent directory tests and both default/small-stack controller runs with626 stage records each. Ignored native/environment tests are not acceptance. All57 packaging tests passed; their simulated Windows structure message is not a Windows-native run.

The dev-profile macOS build produced both `keelshell-app` and `keelshell-mcp`. Staging into a new owned directory and native structure inspection passed for `aarch64-apple-darwin`: Info.plist, linked-library paths and both binaries' macOS15.0 minimum deployment version. Staging performed no signing, installation, publication or GUI execution. Complete binary and stage hashes are recorded in the ignored artifact receipt.

## Outward MCP direction and remaining acceptance

KeelShell provides an MCP server to external agents. The external agent starts the packaged `keelshell-mcp` stdio adapter; its explicit copied, ephemeral capability connects to the application's authenticated/encrypted IPv4 loopback IPC listener. The desktop-owned server applies current grants and human review before reaching the captured SSH session. This task adds no client for other MCP servers. Without the copied desktop environment, the adapter starts its disabled default service rather than acquiring sessions.

The unchanged eight tools are `keelshell_list_sessions`, `keelshell_read_selection`, `keelshell_sftp_list`, `keelshell_sftp_read`, `keelshell_monitor_snapshot`, `keelshell_propose_command`, `keelshell_propose_file_change` and `keelshell_get_action_status`. Proposals remain human-reviewed; no approve/execute tool is exposed to the external client.

A different non-author must read and test the final frozen candidate; root must then verify main-tree integration and new native/client boundaries. This author result is a candidate, not independent approval. No result here closes actual native UI, external clients/models, Windows/Linux native acceptance, production workloads or the unrelated historical Linux CI failure.

Evidence is preserved in the isolated author's ignored `work/mcp-retirement-author/`, including each epoch's sources, before/after maps, raw log, actual wait receipt, numeric PID/PGID and private temporary-directory evidence. Original independent review remains sealed separately. Full output is kept; known dependency future-compatibility and the test linker's large unwind warning are not hidden.
