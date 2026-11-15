# CORE-R2-01 — P2 — retained farewell tail is lost after peer cancellation or input failure

Status: OPEN. Recommendation: HOLD for the bounded-tail core scope.

Frozen input: `frozen-inputs-r2.json`, SHA-256 `8efdd6ebf0253323d169abd4e87a05bef98011e17c3dfa63beed52dac5eecd1a`; HEAD `b66ec7e564e97c3cc8261b447288377ac1ffccc2`. All 583 frozen files still match when this notice is written.

Location: `crates/keelshell-core/src/zmodem/receiver.rs:253-257` (tail extraction requires the current state to be Finishing). Related transitions: lines 98-101, 122-125, 136-146, 247-251, 343-346.

Trigger A: after a valid initial ZFIN has entered Finishing, successfully feed `"个$ ".as_bytes()`, then feed a valid peer Cancel/Abort/FileError header or five CAN bytes. The peer cancellation branch changes state to Cancelled before the terminal extraction branch. That branch only calls `take_farewell_bytes()` for Done, so the preceding UTF-8 tail is not returned. A following local `cancel()` exits early because the state is already Cancelled; later `feed` returns only new input. The previously retained bytes are permanently inaccessible. The validated cancellation framing should remain protocol data, while the preceding ordinary bytes must be delivered once.

Trigger B: after the same successful ordinary-tail feed, feed `encode_header(Header::new(FrameType::Command, 0, HeaderFormat::Hex))`. `header()` returns RemoteCommand and `feed()` changes state to Failed. The documented error cleanup call `cancel()` now bypasses its Finishing-only tail extraction and returns only `[Send(CANCEL), Cancelled]`. Thus the prior successfully retained `e4b8aa2420` ordinary bytes are lost even though cancellation completes. The same problem follows retry/limit failures after an ordinary tail. This finding does not request replay of the failing input, success reclassification, or weakening remote-command rejection.

Impact: cancellation and failure cleanup can silently delete an already-received shell prompt or diagnostic tail. The two newly added local-cancel tests exercise cancellation while the state remains Finishing and after strict Done; they do not cover these state transitions. Current peer/error tests contain no preceding ordinary tail.

Required correction: keep ordinary farewell-tail ownership available through peer cancellation and Failed cleanup, deliver it once, retain strict failure/cancellation results, and exclude validated cancellation framing. Add parent-owned Rust regressions for both triggers with whole, bytewise and split feeds, repeated cancel and subsequent input, then supply a new freeze and bound stable binary. Preserve this notice and the existing red evidence.

Evidence status: deterministic source/state trace, not a newly compiled or executed failing Rust test. The supplied r2-bound stable binary was independently verified and executed: 94 passed / 0 failed / 0 ignored / 0 filtered out, exit 0. The missing adversarial cases are absent from that inventory. Reviewer did not compile, invoke Cargo/format, access the shared target, or modify implementation/test sources.
