# P2-003 — r3 carries forward the uncorrected bridge cleanup issue

Status: **OPEN / HOLD for session/bridge/UI final signoff**.

The original independent finding is preserved in `../review-session-ui-r2/notice-P2-003.md`, with exact source excerpts in its companion evidence JSON. This is a status receipt, not a modification of that notice.

The reviewer independently verified r3 freeze `e83c251520832cbb3a15f0c52c9ae23365729e0c4f192cf0edf2c537c0ba6434`, all 571 + 12 inputs, and the four-file r2-to-r3 delta. Both `crates/keelshell-app/src/ssh_bridge.rs` and `ssh_bridge_zmodem.rs` have identical r2/r3 hashes. `run_with_protocol` still calls `protocol.fail()` after `pump` returns and then drops the protocol without delivering cleanup-produced terminal bytes from `protocol.output`. The core peer-cancel/Failed-state tail fix and new session controller tests do not exercise or correct this production bridge exit path.

Await a parent correction and a newly bound freeze/binary containing a meaningful production cleanup regression. If r3 remains unchanged, report HOLD with this P2 explicitly open; do not reinterpret old 4/4 or 26/26 passing tests as coverage of this path. No new source/test edits or compilation have been performed by the reviewer.
