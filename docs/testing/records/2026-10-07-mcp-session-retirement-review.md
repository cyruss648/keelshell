# MCP session retirement: independent review — 2026-10-07

Status: **confirmed P1 on main e695; isolated correction in progress, not
integrated or independently approved**. This is separate from the application
Agent workflow's stop-cancellation finding.

A fresh non-author copied and read all 736 inputs of
`e69575bc19afe03e80708e03d4993768c8f5dfb0`, 15,082,454 bytes. Real controlled
GPUI and pinned owned loopback SSH/SFTP tests created a file proposal,
completed its full baseline and desktop approval, and held the exact
canonical response before writing. After the lifecycle change, the test
released that hold while preventing foreground maintenance, then read
the complete file through another SSH/SFTP connection.

All three cases observed replacement bytes instead of the original
controlled Chinese text:

| Production transition | Evidence before backend release |
| --- | --- |
| `close_tab` | Granted entity removed from both tabs and remote sessions; MCP still enabled and action Running |
| Raw typed Terminal End | Terminal already reports closed, but its foreground poll has not executed |
| `finish_reconnect` | New entity installed and old entity removed; MCP still enabled and action Running |

The background file worker's existing lease alone did not observe these
transitions immediately. Closing the remote shell channel does not close
the shared SFTP connection. The tests concern later write admission after
retirement, not rollback of writes already dispatched.

The first two cases actually exited 101 in 60.707 seconds including
compilation; the reconnect case exited 101 in 14.820 seconds. Their raw
logs are 3,746 and 2,208 bytes, with SHA-256
`8c0f822987fdfd06538f0a51db2e25a2170d324c1fdf73083f476e2ca6a180dc`
and `4aa7b33f96aa3cba821250ee5ea63d6a0931b82ab86fc35a1c1e1290d19520d0`.
Each test snapshot and frozen source remained unchanged. Owned PGIDs
35386 and 38526 were actually reaped and confirmed absent; private TMP
directories were removed without signals or surviving owned processes.

The root fully read and copied the 756-payload review packet, 15,804,919
bytes. Manifest SHA-256:
`583acb36b4540b1c4769f4c8e5f8eb19b4708de35791bdd2564cbe9c20b8f5bf`.
Evidence and failed cases remain in ignored
`work/mcp-main-retirement-root-consumption-20261007-v1/`.

The isolated author correction binds authorization directly to the captured
raw terminal lifecycle and producer liveness, and revokes grants in the
close/reconnect foreground turn. Its same three original cases have passed
controlled author tests, but full gates, new non-author review and main
integration are pending. These tests do not establish native desktop,
external CLI/model, customer-host, Windows/Linux or release acceptance.
