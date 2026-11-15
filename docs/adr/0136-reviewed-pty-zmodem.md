# ADR 0136: Reviewed Zmodem on the existing remote PTY

- Date: 2026-11-15
- Status: candidate implemented; final engineering gate and scoped nonauthor source reviews passed; direct-PTY acceptance open
- Baseline: `721bad2157da8d3a0bc11a1eab5ac59b6616aa9c`
- Requirements: FILE-05, TERM-01, TERM-02 and SEC-02

## Decision

Support actual `rz` / `sz` peers on the existing authenticated remote SSH PTY.
The terminal detects a complete, checksum-valid protocol initiation across
arbitrary transport chunks. Ordinary output, including incomplete or invalid
lookalikes, retains its byte order. Detection never approves a file operation
or executes a shell command.

The protocol codec and bounded sender/receiver state machines belong to
`keelshell-core`. They have no filesystem, network or UI authority.
`keelshell-session` owns source observation, ephemeral reviews, local publication
and cancellation. The application bridge serializes protocol output with PTY
writes; the native terminal displays the current transfer, complete review
and explicit confirmation or cancellation.

## Human review and channel ownership

A detected receiver request asks the user to select local ordinary files.
Preparation reads bounded content and records source observations before a
complete review of the names, sizes, digests and actual SSH target. Confirmation
revalidates the exact sources and review. An unrelated `rz` command controls its
own remote storage policy; the interface must not promise a remote directory,
atomic publication or preservation of existing remote files without evidence.

A detected sender request may negotiate metadata, but every offered filename
and declared size is reviewed before accepting its bytes. The user chooses an
exact new absolute local filename. An offer, destination or session change
invalidates the review. A following file requires its own review; accepting one
does not authorize an unbounded batch.

While a transfer owns the PTY, ordinary terminal, mouse, paste, AI and emulator
response writes cannot interleave with its protocol. Resize and disconnect keep
their own transport controls. Stale queued ordinary input is rejected with
feedback rather than replayed after the transfer. Cancellation sends bounded
protocol cancellation and does not infer that a remote file was removed.

## Files and protocol

Support bounded regular-file batches, the hexadecimal and binary headers,
CRC16/CRC32, escaped control bytes, acknowledged progress and position-based
retransmission needed by real peers. Remote command execution frames are
rejected. No protocol instruction can become a terminal command.

Admit at most 32 files, 64 MiB per file and 128 MiB per transfer. Require a
declared byte length and verify the final length. Filenames are portable UTF-8
basenames; reject traversal, path separators, alternate streams, device names,
links and aliases. Incomplete packets, checksum failures, stalled peers and
retransmission attempts have explicit bounds.

Independent peers may repeat ZRPOS at an already acknowledged offset after a
stop-and-wait frame. Advancing ACKs reset the 64-attempt position recovery
budget; stalled/repeated recovery still fails. Malformed packets and timeouts
retain their cumulative budget. Whole-transfer wire bytes, packet count and
elapsed time stay bounded across progress. Duplicate acknowledged ACKs cannot
advance or skip bytes.

Receiving uses a new private local staging file and no-replace publication.
Existing targets are retained. The final local bytes must match the completely
received contents before completion is reported. Cancellation, publication
uncertainty and verification failure preserve the corresponding uncertainty;
no automatic retry, cleanup write or overwrite is inferred.

On macOS, private access is cleared and inspected through a retained file
descriptor using the small target-specific `calcifer-macos-acl` safe wrapper.
Its registry requirement is `0.1`, with the exact MIT-licensed, zero-dependency
implementation locked and reviewed. Pathname metadata and `ls /dev/fd/N` do not
establish ACL absence. Linux uses descriptor xattr inspection/removal.
Windows remains explicitly unsupported until it has a verified file-identity,
permissions and no-replace backend; protocol detection is not file acceptance.

## Engineering and acceptance

Unobserved publication results keep their ticket until disposition, even after
the blocking worker exits. A shared memory-only mutex synchronizes active-ticket
accounting, sticky unknown state and publication of the close hold. Backend
reservation disposition stays outside that mutex. Poison or accounting errors
retain the hold; a later successful transfer cannot clear an older unknown.

Native review retains the latest 32 complete verified-save receipts independently
of current progress, later offers or terminal outcomes. Every terminal UUID
requests its own bounded, backpressured input handoff, including termination
before the first file offer. Late refusals cannot reactivate retired cards.
An SSH completion observation ends the current write attempt but preserves the
owner which drains ordered tail output. It never acknowledges an unwritten
protocol trailer.

Direct-TTY observations with both macOS and Debian peers include clean peer
exits after valid ZFIN and verified local saves but without the final OO.
These runs remain failed/partial, with their original diagnostics retained.
Pipe-relay observations do not close that acceptance gap or establish its cause.
A distinct future clean-channel-closure result was considered during review but
is not implemented or approved by this candidate. It cannot silently substitute
for the current strict Finished event.

Develop the pure protocol, session integration and owned Linux fixture in
disjoint write sets. The parent owns the current checkout's sole effective
Cargo target and integrates before the full engineering gate. New nonauthor
agents review the final candidate and actual executed evidence.

The final r3 engineering gate exited 0, with all 570 engineering inputs and 12
gate scripts unchanged after execution. The two scoped nonauthor approvals have
zero open P0–P3 findings. Their original reports retain the pending gate state
at signing time; the parent's later verification satisfies that engineering
condition without changing the original reports. Counts, retained failures and
the separate interoperability gap are recorded in the
[batch record](../testing/records/2026-11-15-reviewed-pty-zmodem.md).

Codec roundtrips and same-implementation peers are insufficient to establish
interoperability. Exercise actual `rz` and `sz`, including PTY framing, binary
content, empty files, Unicode names, cancellation and preservation of existing
local targets. Record Linux syscalls separately from emulated branches and
Python 3.8 grammar separately from its runtime.

All three native platform, IME, accessibility, packaging and release boundaries
remain independent. This decision does not mark FILE-05 or the product complete.
