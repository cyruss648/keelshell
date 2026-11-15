# CORE-R1-01 — P2 — repeated ZFIN consumes preceding ordinary UTF-8

Status: OPEN. Candidate recommendation: HOLD.

Location: `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:590-592` (candidate start: lines 570-579; parity-tolerant pad scan: lines 316-329).

After an initial valid ZFIN has entered Finishing, feed `"个".as_bytes()` immediately followed by `encode_header(Header::new(FrameType::Finish, 0, HeaderFormat::Hex))` and `b"$ "`, then call `finish_without_farewell()`.

Expected terminal bytes: `e4b8aa2420` (`个$ `). Source predicts `e4b82420`: the final `aa` continuation byte is removed and the valid CJK text becomes incomplete UTF-8. This affects terminal-tail fidelity, including the actual OO path, without changing saved-file contents.

The new trimming subtracts the whole header candidate, including an ordinary prefix byte: `aa & 7f == 2a`, so the existing decoder starts the candidate at that byte and the existing parser accepts it plus the real FIN's two pads as a three-pad header. The actual FIN CRC is valid; it does not authenticate the extra preceding pad as protocol-owned.

Proof is a deterministic source trace, not a newly compiled/executed Rust regression. The parent's stable suite does not include this adjacent CJK/pad case. Please add the exact regression centrally and verify a narrow boundary fix, including fragmented input and actual OO. No reviewer Cargo/target/source/fixture writes were performed.

Frozen r1 SHA-256: `cf4cb0e53b80b09c44da99e3fe358cd7091622314625f881fd2769df783beba4`. Original finding and evidence must be retained even after a fix.
