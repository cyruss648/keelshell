# Parallel transfer writable CLOSE repair — 2026-10-06

Status: v3 final author gates passed; frozen candidate awaits fresh independent
review and combined root integration. No commit, push, GUI, vendor process, cloud, customer host,
installation or Release. Original v1/v2/v2b packets and failures remain immutable.
[ADR 0055](../../adr/0055-parallel-remote-transfers.md).

## Confirmed independent v2b failure

A new nonauthor review retained an actual TCP counterexample after all 34 WRITE
bytes had replies. The server still held the writable CLOSE STATUS with an exact
barrier, one actual RAII in-flight CLOSE and no fallback expiry. Dropping direct
`write` or cancelling queued upload admitted a same-target `write_atomic`; no
quarantine existed, and the queue falsely returned `Cancelled { bytes: 34 }`.
It establishes early admission, not claimed corruption. The original strict
probe, 104-payload report/inputs/logs and all original 258 v2b payloads remain
unchanged. The independent report manifest is 15,971 bytes, SHA-256
`17f9ee11e0e8a45fefb282ccdba30c5137c2990035550479c090dd28f83f371d`.
The author copied and read back every one of those 104 payloads before repair.
Passing prior full gates does not approve v2b integration.

## Narrow repair and descriptor inventory

| CLOSE entrypoint | Ownership / reply classification |
| --- | --- |
| Direct `write` | Same mutation scope through writable raw CLOSE STATUS |
| Direct/queued regular upload | Transfer context tracks the writable target CLOSE |
| Directory upload child | Same tree reservation through each writable child CLOSE |
| File/directory continuation final and pause-rebind target | OpenFile remembers WRITE mode; writable target CLOSE requires transfer context |
| Atomic temporary in ordinary/reviewed/sync/queue upload | Same exact final/temp owner and pending marker; once operation is polled, cleanup never retries this invalidated descriptor |
| Owned cleanup before ordinary atomic CLOSE was sent | Bounded existing CLOSE then REMOVE ticket; missing CLOSE reply retains final/tree and temp Unknown IDs |
| Download remote source, resume source/scan, listing/read/snapshot | Read-only CLOSE does not create remote mutation quarantine; local write/flush completion remains independently tracked |

Only raw success or an explicit STATUS refusal clears a pending writable CLOSE.
A missing reply, transport loss, cancellation or future drop while it is pending
remains unknown. Acknowledged WRITE bytes do not prove CLOSE, publication or
rollback. Pre-send checkpoint cancellation is separate and can remain known.
The protocol distinction follows the [primary SFTP v3 draft section 6.3](https://datatracker.ietf.org/doc/html/draft-ietf-secsh-filexfer-02#section-6.3),
which permits CLOSE flush failure and invalidates sent handles. No new dependency,
lock/toolchain upgrade, automatic replay/retry/reconnect or durable recovery.

## Focused author evidence

The original strict probe body and 17 copied fixture/lock inputs are unchanged;
only its Cargo path and fixture module absolute locations point to this author's
private tree. The two path-only source deltas are recorded separately. The first
run passes actual direct-drop and queued-cancel states: same-target writer returns
`MutationQuarantined`, read-only inspection finds one record, actual CLOSE remains
pending, and the queue returns `Uncertain { bytes: 34 }`. Its 19 ignored probe
inputs and 542 visible engineering inputs remain independently equal, wrapper
13.180s/test 0.04s. This first run predates the final first-poll cleanup refinement;
final-source repetition is required below.

Ten actual TCP tests, third focused run, passed: wrapper 10.583s/test 2.13s,
542 visible inputs equal. They cover direct-write/queue pending CLOSE and cross-connection
retention; new-directory and directory-continuation child CLOSE; continuation
final/pause-rebind writable CLOSE; atomic temporary drop/cancel plus both IDs and
original final preservation; rejected WRITE then unacknowledged cleanup CLOSE;
valid CLOSE STATUS refusal and a fresh manually submitted write; acknowledged
pause cancellation before CLOSE; read-only download/scan cancellation; actual
normal atomic CLOSE success plus independent-target overlap; and an explicitly
revoked reviewed caller dropping its pending CLOSE, with ordinary approval still
unable to clear unknown IDs. A caller's explicit future drop is tested; it is not
claimed that a synchronous authorization callback alone cancels in-flight I/O.

Each CLOSE fixture hold distinguishes WRITE descriptors from planning/source READ
descriptors, owns its generation, and counts the actual held server handler with
RAII. Tests require exactly one entry, one still-pending CLOSE and no expiry;
late release/handler completion does not clear old Unknown IDs. The ordinary
atomic case proves independent target publication while CLOSE is still pending.
Legacy in-place writes/existing continuations keep their previously documented
whole-side lock; these tests do not claim arbitrary hard-link identity protection.

The first focused run had 5/8 passing and three new expectation failures: those
new tests incorrectly expected an independent writer to bypass an existing
in-place/continuation whole-side Unknown claim. Production isolation was not
narrowed; tests now assert that existing policy. The failed source maps/log/receipt
remain. The second run 8/8 passed before the final first-poll cleanup refinement
and two extra tests. Every run has equal before/after engineering inputs; no
mixed-input gate is used. All owned leaders were waited/reaped, original numeric
process groups observed absent and empty private TMP removed, not an escaped
subprocess census.

## Final gates and boundaries

All following commands completed on the same **543 visible engineering inputs**,
with equal full before/after maps. Every leader was waited/reaped; original numeric
process groups were observed absent and empty absolute private TMP removed. Only
these three result documents changed afterward: this record, ADR 0055 and Files
product guide. Production/tests/manifests/toolchain/scripts retain the exact hashes.

| Final command | Actual result |
| --- | --- |
| `python3 -B scripts/check.py` | exit 0, 345.647s; **1,211 ordinary Rust / 8 rustdoc / 6 script tests**; format, x.y policy, strict workspace/all-targets Clippy and default/explicit 2 MiB controllers |
| Explicit 2 MiB CLOSE TCP selection | 10/10, wrapper 6.354s / test 2.13s; final selection additionally includes direct `upload` future drop to verify the shared scope/context pending marker |
| Original strict independent CLOSE probe, final repetition | 1/1, wrapper 7.213s / test 0.05s; 19 ignored probe inputs separately unchanged; original strict body/fixture unchanged except two absolute location replacements |
| Fresh system OpenSSH | 10/10, test 5.31s / script 9.708s / wrapper 9.780s; existing alias/hard-link/guard/pause/continuation/exec assertions retained |
| Native app + MCP development build | exit 0, 6.121s; arm64 Mach-O, minimum macOS 15.0; neither binary launched |
| Packaging regression | 57/57, wrapper 1.503s / test 1.397s; package structure is not native GUI acceptance |
| Native binary inspection | exit 0, 0.194s; actual file/vtool and binary SHA-256 |

The gate contains 450 app tests, 68 session unit tests and 135 real SSH loopback
tests, in 42 ordinary/four doc harnesses. Focused selections overlap and are not
added again. Twelve opt-in ignored choices remain (two provider CLI, ten OpenSSH);
OpenSSH ran separately. Both local-agent controllers record a 5,040-byte future;
they call no supplier CLI or model. The existing transitive `block 0.1.6` future
compatibility warning remains. Full gate log: 131,323 bytes, SHA-256
`50efb893a588e3ca279baa9b0fa2f47a42bb9dd6da6994937da79a32dab56b11`.
Final strict probe log SHA-256:
`452bc4bec268fcac508d4f72911d63b38dfa5722457f6f101e31dccc6492ad5e`.

The stock OpenSSH receipt records **56** observed kernel birth identities stopped,
no unverified ancestry and private key/root removal; that exact private path was
also observed absent. This count belongs to this run, not the old v2 run. Original
group absence is not an escaped-descendant census; detached processes entirely
between ancestry observations remain outside stock OpenSSH proof.

Native development binaries, never launched:

- App: 184,031,472 bytes, SHA-256
  `a664442596c2b5dd082422b5a2e75fd36ffcceb8f566ddf269ed161a6e49e155`.
- MCP: 16,716,032 bytes, SHA-256
  `b7fda1286acc599fae1b4fcec462fc1a197c28ba13158b8a7e5680fbe2b1fdc2`.

The failed first eight-test source is additionally reconstructible: five changed
preimages were reconstructed after all commands terminated and each matched its
original retained bytes/SHA map exactly. Original failure logs/maps are unchanged.
The first ignored proof-formatting attempt could not resolve a sibling module in
a partial snapshot; that failed script/partial outputs remain. The second uses
isolated child-skipping formatting and verifies all five original hashes. This
proof-preparation failure changed no engineering input and is not a product test.

The v3 package contains the complete 39-file candidate from e247 and an 11-file
CLOSE delta against exact v2b/base preimages. Scratch actual delta application,
complete input/proof manifests and read-only root integration checks are required
before sealing. Original v1/v2/v2b and 104-payload blocked review remain immutable.

Source-level GPUI/TCP, OpenSSH and native builds do not establish desktop
acceptance on any platform, signed releases, installation, all external filesystem
aliases or customer workloads. Shared in-memory isolation is not recovery or proof
that a disconnected remote request stopped. A new nonauthor review and combined
root integration remain necessary.
