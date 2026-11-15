P2-01: PAX metadata sizes can be truncated before the 8 KiB safety check

Candidate: production source shared by frozen-inputs-r1.json and frozen-inputs-r2.json.

Locations:
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:699
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:714
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:785
- Locked dependency: /Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/tar-0.4.46/src/header.rs:1536

The dependency numeric_extended_from reads only the last 8 bytes of a 12-byte base-256 size field. A valid-checksum type-x metadata header with size field 80000001000000000000000d declares 18446744073709551629 (2^64 + 13), but header.size() yields 13. With the 13-byte payload `13 path=safe\n`, zero padding, an ordinary empty fallback file and two end blocks, the new PAX branch sees an in-budget complete header. The complete raw size is never checked. This is a newly admitted PAX input that the baseline rejected as an unsupported extension. The same numeric dependency weakness also exists on older ordinary-member handling; this review does not claim that older behavior was introduced by this patch.

Impact: a size declaration exceeding both u64 and the metadata cap is silently shortened, so a structurally truncated archive can pass the claimed strict header-size and integrity policy. No path escape or unbounded actual allocation was established: the wrapped size and stream remain bounded. Severity is P2 for fail-closed and parser-consistency failure, not a demonstrated arbitrary write.

Suggested fix: validate the complete raw 12-byte size field before calling header.size(), rejecting negative, overflowing, malformed or unsupported encodings. Apply the same check to metadata and ordinary headers. Add regressions with nonzero high base-256 bytes and negative base-256 values whose low 64 bits look small; retain actual-writer and portable-fallback tests.

Evidence: base256-overflow-static-probe-r1.json and tmp/pax-metadata-base256-overflow-r1.tar.gz in this directory. Python TarInfo.frombuf independently reads the complete declared size; the new fixture was not dynamically run through Rust because this reviewer may not compile or modify source. Source control-flow analysis supports the Rust acceptance prediction.

Notification: early finding was posted in the review chat. The desktop send_message_to_thread tool refused delivery to this native ancestor; no native collaboration send tool is exposed here. This file preserves the immediate written notice for the parent. Review continues.
