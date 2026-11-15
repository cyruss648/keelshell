**KeelShell SSH/PTy farewell diagnosis, 2026-11-15**

No verified causal defect was found in the inspected SSH reader, channel ownership, or real-peer fixture lifetime path. Verified production findings: P0=0, P1=0, P2=0, P3=0. Root cause remains unestablished; direct PTY acceptance remains HOLD/PARTIAL. This is a static diagnosis and retained-evidence analysis, not a fresh interoperability run or merge approval.

Source root: `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`. The parent supplied HEAD `b66ec7e564e97c3cc8261b447288377ac1ffccc2`; Git was deliberately not queried. The nine relevant inputs listed below match the retained r3 freeze. The applicable user restrictions supersede historical handoff authorization.

Only this report was written. No source, fixture, Cargo, test executable, worktree, Git, or target operation was performed. Fresh test counts are 0 run, 0 passed, 0 failed; the historical counts below remain historical. No core protocol or upstream lrzsz finishing implementation was reviewed; those investigations belong to the parent.

The retained failures narrow the boundary:

| Retained run | Historical result | Observed failure |
| --- | --- | --- |
| real-pty-r6 | 6 passed, 1 failed; test exit 101 | CRC32 Unicode receive: `FileSaved` for 262156 bytes, controller `Receiving`, final captured data is the 20-byte ZFIN, then `Exited { code: 0, success: true }`. |
| linux-cycle-r2 | 5 passed, 2 failed; test exit 101 | Empty-file and CRC32 receives encounter `Exited { code: 0, success: true }`; this older binary did not capture controller state or tail bytes. Do not retroactively attribute the richer r3 diagnostics to r2. |
| linux-cycle-r3 | 4 passed, 3 failed; test exit 101 | Empty, CRC16 and CRC32 receives each show `FileSaved`, controller `Receiving`, the same final 20-byte ZFIN, then exit 0. |

The rich failures end with `2a2a184230383030303030303030303232640d8a`. The logger retains only the first 96 bytes of each of the last six data events; because this final entry contains only 20 bytes, that last event is fully represented. Earlier large events are truncated. These records do not contain a complete bidirectional transcript or a peer syscall/PTY-master trace. They cannot decide whether OO was never emitted, flushed/discarded by peer-side terminal operations, or lost before SSH channel delivery.

The three decompressed test-log SHA-256 values independently match the values in the retained review:

| Evidence file, relative to the evidence directory below | Decompressed SHA-256 | Relevant decompressed lines |
| --- | --- | --- |
| real-pty-r6/tests.log.gz | c2f0e45f78d0340d895c4a27720d010eb9df886d7749e0bc57c11b2011b0f6b8 | Failure 17; result 23 |
| linux-cycle-r2/025-parent-command.stdout.log.gz | 7092407cf28093a6a9cf559818128fa8c2cfdf5d32a4a358fbe3aa3c0ec3e47e | Failures 14, 17; result 24 |
| linux-cycle-r3/022-parent-command.stdout.log.gz | 197a63bb445f70f78817d38f50708f790884a5b453a2a216fc9f15a85be0179e | Failures 14, 17, 20; result 28 |

Evidence directory: `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/testing/records/2026-11-15-reviewed-pty-zmodem-evidence`.

The existing [interop boundary record](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/testing/records/2026-11-15-reviewed-pty-zmodem-evidence/review-core-session-r2/interop-boundaries.json:63) corroborates r6 and explicitly limits the capture at line 80. Its pipe comparison, lines 82–96, supports investigating TTY behavior but does not establish a cause. That comparison was read from the retained record and was not rerun.

The SSH delivery trace provides these verified facts:

1. [start_shell, ssh.rs:589](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:589) registers a monitor, requests and acknowledges the PTY and shell, then splits the actual channel at line 604. No `shell*.rs` implementation exists in this source tree; `SshShell` is implemented in this same file.
2. [ssh.rs:605](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:605) creates a 64-event output queue. Lines 609–635 forward both `Data` and `ExtendedData` with `data.to_vec()` and await each send. There is no terminal decoding, suffix trimming, special OO filter, or lossy `try_send` in this path.
3. Exit status and exit signal only record evidence at [ssh.rs:614](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:614); they do not break the read loop. EOF falls through the ignored-message arm at line 628, so it does not itself terminate the reader. Explicit channel close breaks at line 624. The synthetic terminal event is queued only afterward, at lines 654–661. Therefore, for data delivered to this reader before close, the same producer queues data before `SessionEvent::Exited`.
4. The lifecycle callback can publish completion before the application's terminal queue is drained: [ssh.rs:243](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:243), [lifecycle.rs:140](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh/lifecycle.rs:140), and [lifecycle.rs:167](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh/lifecycle.rs:167). This separation is documented at [ssh.rs:1051](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:1051). Those callbacks do not abort the reader or remove events from its output queue. Completion alone is unsuitable as a byte-drain barrier, but the failing fixture does not use it that way before protocol completion.
5. Explicit shell close and drop do abort the reader: [ssh.rs:1098](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:1098) and [ssh.rs:1130](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:1130). That can intentionally abandon unread bytes when a caller closes a shell. No such call precedes the observed failure in this real-peer fixture.
6. [PendingChannel::into_stream, channel.rs:46](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh/channel.rs:46) uses a separate duplex relay with cancellation. The fixture's SFTP connection uses it; the shell does not. `start_shell` takes and splits the channel directly. A copy-bidirectional shutdown hypothesis for that SFTP relay is not a demonstrated explanation for the shell's missing OO.

The pinned dependency was followed only where referenced by this path. [Cargo.lock:7097](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:7097) resolves russh 0.65.1. Its cached `.crate` hashes to the lock checksum `94be3d86c71b9ed220a675c405e35c7a02565a009bf8da2e9cc13a700320a5c5`; inspected dependency files were checked against that archive.

In the local pinned russh source:

- [encrypted.rs:590](/Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/russh-0.65.1/src/client/encrypted.rs:590) awaits forwarding channel data at line 611; extended data follows the same pattern at lines 616–648.
- [encrypted.rs:525](/Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/russh-0.65.1/src/client/encrypted.rs:525) awaits forwarding `ChannelMsg::Close` at line 541 before removing the channel sender and invoking the lifecycle callback. Removing that sender is not a source-level discard of data already queued before this close.
- [channels/mod.rs:156](/Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/russh-0.65.1/src/channels/mod.rs:156) implements the raw read half as `receiver.recv().await`. The shell does not use `ChannelRx`'s `AsyncRead` adapter.
- [ssh.rs:1070](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:1070) awaits russh's `data` writer. [channels/mod.rs:352](/Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/russh-0.65.1/src/channels/mod.rs:352) copies all supplied bytes into `ChannelTx`; [io/tx.rs:138](/Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/russh-0.65.1/src/channels/io/tx.rs:138) enqueues writes, and its flush at line 179 is a no-op. Successful `shell.write()` establishes local queue acceptance, not a remote receipt or PTY drain. There is no demonstrated two-byte buffering defect here.

These source facts narrow the ordinary ordered-close path. They do not certify every SSH transport failure or all possible peer behavior.

The fixture lifetime trace rules out the proposed early-cleanup explanation for the retained failures:

- The actual peer runs from [zmodem_openssh.rs:103](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/zmodem_openssh.rs:103): `cd … && stty raw -echo -onlcr && exec …`. The peer inherits the real SSH PTY for stdin/stdout; only stderr goes to `peer.log`. There is no post-peer holder of those descriptors in this command.
- [zmodem_openssh.rs:116](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/zmodem_openssh.rs:116) receives data until protocol termination. Non-data terminal events become errors at lines 137–148. Effect writes are awaited at lines 156–158, and only `Finished` or `Cancelled` sets `ended` at lines 212–215. `FileSaved` does not close the shell or end the loop.
- The completion-monitor wait is reached only after that loop, at [zmodem_openssh.rs:238](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/zmodem_openssh.rs:238). Its explicit close at line 257 cannot account for the earlier `Receiving`/unexpected-Exited failure. Error unwinding subsequently drops the shell, but the failure was already observed.
- The similarly named [support/zmodem_fixture.rs:1](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/support/zmodem_fixture.rs:1) is the encrypted admission/SFTP fixture imported by [tests/zmodem.rs:29](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/zmodem.rs:29). It executes no shell, rejects exec requests, and is not imported by `zmodem_openssh.rs`. Its finish/drop abort path at lines 228–249 is not the lifetime of the real rz/sz peer.
- [openssh_interop.py:435](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/openssh_interop.py:435) waits for the test process. Normal polling captures identities; it does not signal the peer. Process signals and scratch removal begin in `finally` at lines 447–488. r6's complete test run took 5.88 seconds and its outer receipt 7.816 seconds, versus a configured 280-second budget. The retained failure is a terminal event, not an outer timeout.
- [run_linux_file_interop.py:934](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:934) synchronously waits for the supplied test command. [Commands.run:187](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:187) begins command cleanup after polling ends or a deadline/interruption. Both retained parent-command steps report exit 101, `timed_out=false`, `interrupted=false` (r2: 3.078 seconds; r3: 3.498 seconds). Container deletion occurs later in `finally`, lines 964–982. Preserved r2/r3 copies of both Python runners match current bytes.
- The independent reference relay at [run_linux_file_interop.py:709](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:709) reads stdout until EOF even after process exit. Its status/readback success checks at lines 738–755 do not parse or prove the application's strict ZFIN/OO completion.

No verified fixture defect is assigned severity. A completion-before-drain race would be a risk for a different caller that closes on `completion()`, but it is not the observed control flow here. Scope was not expanded to audit application callers.

The remaining hypotheses and discriminating work are:

| Hypothesis | Evidence and limitation | Test that would discriminate |
| --- | --- | --- |
| Peer finishing never emits OO, or a terminal flush/restoration discards it | Failures end at ZFIN; pipe comparison differs. There is no syscall trace establishing a write or flush. | Parent's upstream audit plus a bounded peer trace of writes, drain/flush/termios calls and exit. Capture the application's actual outbound ZFIN too. |
| Last PTY slave lifetime or sshd master draining loses the short tail | The fixture `exec`s the peer and retains no post-exit holder; direct TTY/pipe results differ. No master-side read trace exists. | Compare immediate exit with a wrapper retaining inherited PTY descriptors briefly after the peer exits; keep the peer itself on the same direct TTY. |
| SSH reader loses data under immediate close/backpressure | No such discard was found in the inspected ordered channel queues. Existing tests cover basic bytes/resize and saturated duplex echo, not this farewell/close sequence. | Add a focused real encrypted loopback tail/close test with controlled backpressure and exact byte assertions. |

The first proposed parent-owned regression is `shell_delivers_final_oo_before_exited_with_immediate_close`. Adapt the actual encrypted shell fixture near [ssh_loopback.rs:172](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/ssh_loopback.rs:172), not the no-shell Zmodem admission fixture. Acknowledge PTY/shell, emit fixed binary bytes plus the observed ZFIN and literal `OO`, then exit status 0, EOF and close with no deliberate delay. Drain `SessionEvent` through the terminal event and assert the complete concatenation, exactly one ordered terminal event, and no omitted/duplicated tail.

Use cases for coalesced `ZFIN+OO`, separate ZFIN/OO events, split `O`/`O`, and exit-status-before-final-data. For a second case, send more than 64 small data events and coordinate receiver release with a oneshot and a bounded timeout; exercise backpressure without sleeps or a wait cycle requiring completion before the queue can drain. Observing `completion()` may precede delivery, but must not be used to stop receiving. The existing [saturated echo test](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/ssh_loopback.rs:566) does not close the server immediately after the farewell.

A concrete real-peer experiment can proceed without changing production semantics:

1. Freeze the parent's selected inputs and binary. Add bounded full-byte capture in the diagnostic fixture at the existing `Data` branch and immediately before each awaited effect write. Record direction, monotonic timestamps, exact lengths, and terminal event order; cap capture at 2 MiB per direction and fail capture explicitly on overflow.
2. Compare the current `exec sz` command with a diagnostic wrapper that starts the same sz with stdin/stdout inherited directly from the SSH PTY, waits for its status, writes status only to a separate diagnostic file, and keeps its own inherited PTY descriptors open for 250 ms before exiting with that status. Use no stdout marker, byte relay, pipe replacement, termios restoration, or synthetic OO. Record `isatty(0/1)` for the child path. This changes process/lifetime topology and is diagnostic only.
3. Bound each arm to 10 trials of the retained failing CRC32 receive plus the empty receive. Count actual observed OO and strict `Finished` separately from saved-file digest and exit status. Do not stop recording at monitor completion; drain through the ordered terminal event.
4. Run a separate raw-tail control through `start_shell`: a disposable program writes fixed `ZFIN+OO` bytes directly to fd 1 and exits, compared with writing those same bytes and retaining the PTY for 250 ms. This control tests tail delivery only; it is not a Zmodem acceptance test.

If a held-descriptor arm recovers OO while immediate exit fails, that supports a peer/PTY lifetime interaction; it does not identify tcflush specifically. If both arms fail and a peer-side trace shows successful OO writes, inspect subsequent terminal operations and sshd PTY-master reads. If raw-tail controls and the synthetic SSH tests pass while lrzsz still fails, prioritize the parent's protocol/upstream finishing investigation. A passed synthetic test alone does not close direct PTY acceptance.

Retain strict `Finished`: only actual required ZFIN/OO protocol completion qualifies. Neither `FileSaved`, peer exit 0, the peer's “Transfer complete” text, a delay, nor a pipe/reference transfer may substitute for the missing bytes.

The following current SHA-256 values match `frozen-inputs-r3.json`:

| Input | SHA-256 |
| --- | --- |
| Cargo.lock | b7964caece668b87008bd8ce4fa563977982e075e2487197ce3a4a37b41cafa5 |
| crates/keelshell-session/src/ssh.rs | d86a3cf3f42cbc1ac06f41d5e793b1f042fe558992836730df06184e865fb79b |
| crates/keelshell-session/src/ssh/channel.rs | d1f89dd1c86ba161d03c7b7fa9279c5ced59d04a77d36fc3b54714aad3fa16aa |
| crates/keelshell-session/src/ssh/lifecycle.rs | d95b77381f649066fd663700e41df41f5c59e28f3b1c3797a09656e9a6f9436e |
| crates/keelshell-session/src/ssh/transport.rs | d837922e0c5a836529ca0539759ceae910fc404ff71a5850d92430d19915ff04 |
| crates/keelshell-session/tests/support/zmodem_fixture.rs | 13d6c0eb892fcf8f492c2f3d89dac65bdeecd03591aa24fe6c500e2e34d3468a |
| crates/keelshell-session/tests/zmodem_openssh.rs | 819e4ae15f9779268f26094eaf8645644fb1abf9b8a5abe8c0d84a877498f7f1 |
| scripts/openssh_interop.py | 78a9074e567d7ef128096ffd5dc40c5f458ed9a6db2cb635671d01da47b25a5d |
| scripts/run_linux_file_interop.py | 79e9fdaafd1a1723994ef0ad7bd46e9304f3121189a2099d2df10514cb4fc2c0 |

Limitations: the supplied HEAD was not independently checked through Git; no executable tests, Cargo validation, fixtures, native UI, syscall tracing, or SSH packet capture were run. macOS r6's retained sshd log contains only seven audit-setup warnings; Linux r2/r3 retained final sshd logs are empty. They supply no farewell-byte trace. The recommendation is to keep the acceptance hold, run the focused experiments under the parent's unique target ownership, and obtain the planned new nonauthor review after any implementation.
