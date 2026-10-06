# Parallel transfer application mutation repair — 2026-10-06

Status: v2 final author gates passed; frozen repair awaits independent re-review
and combined root integration. No commit, push, GUI, release or installation.
Base remains `e247f2f`, isolated `feature/parallel-transfers` worktree. Dependencies,
Cargo.lock and toolchain pin are unchanged. [ADR 0055](../../adr/0055-parallel-remote-transfers.md).

## Confirmed defect and actual repair

The new nonauthor v1 review reproduced an application writer bypass over owned
TCP/SFTP: `write_regular_reviewed` published to an actively held queued atomic
upload target, then published again while its cancelled transfer remained
`Uncertain { bytes: 32768 }` and retained the same quarantine ID. It did not claim
the entire MCP UI was exercised. v1 is blocked; its sealed original 24-file packet,
all failures/mixed-input evidence and old test record remain unchanged. Passing
v1 gates did not establish shared admission for all application writers.

v2 guards the actual transport boundary with a bounded shared action owner.
Ordinary Files workers, other panels and separately approved MCP actions cannot
replace an active or quarantined target. Read-only inspection remains available
and independent targets remain useful. Proposal approval is separate from explicit
unknown-risk consent; neither consent nor reconnect proves late I/O has stopped.

| Existing write entrypoint | Protected scope / actual completion |
| --- | --- |
| `write`, `upload` | Local source read where applicable; remote in-place side; raw CREATE/WRITE replies |
| `write_atomic`, `upload_atomic` | Exact final namespace plus exclusive owned temporary; valid atomic STATUS publication |
| `write_regular_reviewed` and authorized variant | Same atomic scope; reviewed snapshot; MCP lease rechecks before each mutation |
| `download` | Remote source read and create-new local target; local create/write/flush completion |
| `mkdir`, `remove`, `rmdir` | Canonical namespace/tree; raw mutation STATUS, including ancestor overlap |
| `rename` | Both canonical source and destination namespaces under one action |
| `set_permissions_reviewed` | Conservative remote inode side; existing parent/type/mode review plus SETSTAT reply/readback |
| Queue file/atomic/directory/continuation jobs | Existing session-bound reservations, independent handles and pending-I/O tracking |
| Files ordinary Save/create/rename/delete/permissions | Above guarded APIs on dedicated workers; panel busy is not exclusion authority |
| Files directory-sync local/remote writes | Both canonical trees held through fresh replan and all children; synchronous local completion and guarded remote child methods |
| MCP approved file replacement | Authorized reviewed atomic writer; active/unknown refusal marks failed before writes; normal approval releases no unknown records |
| Owned temporary cleanup | Same action final/tree plus exact temporary ID until actual acknowledgement; timeout/abort remains unknown |

One concrete SSH connection has at most four active action owners; global state
has at most 32 active/unknown groups and at most two write paths per group.
Sync children reuse that owner and remove acknowledged temporary claims. A
scope rejects concurrent children and newly quarantined ownership. Direct
conflicts return typed busy/quarantine errors immediately; no delayed replay.

## Final frozen-source evidence

| Final command | Actual result | Exact input / ownership scope |
| --- | --- | --- |
| `python3 -B scripts/check.py`, second gate | exit 0, 273.724s; 1,201 ordinary Rust / 8 rustdoc / 6 script tests | 541 inputs unchanged; format, x.y policy, strict workspace/all-targets Clippy, default and explicit 2 MiB controllers passed |
| Fresh system OpenSSH | 10/10, test 4.88s, script 17.925s / wrapper 17.979s including compile | Same 541 inputs unchanged; paused active ordinary/reviewed/rename conflicts denied with spare capacity, distinct writer succeeds; hard-link/alias original assertions preserved |
| Native app + MCP development build | exit 0, 8.808s | Same 541 inputs unchanged; arm64 Mach-O, minimum macOS 15.0; neither binary launched |
| Packaging regression, second run | 57/57, wrapper 1.403s | Same 541 inputs unchanged; structure checks do not establish native GUI acceptance |
| Native binary inspection | exit 0, 0.151s | Same 541 inputs unchanged; file/vtool and full binary hashes recorded |

The final full gate includes 450 application tests, 68 session unit tests and
125 actual SSH loopback tests; focused selections overlap these counts. Forty-
two ordinary and four doc harnesses passed. Twelve opt-in ignored choices remain
(two vendor CLI and ten OpenSSH); the latter ran separately. Both local-agent
controllers report a 5,040-byte future and all scenario assertions passed; their
scenario totals are not added to ordinary Rust counts. The existing transitive
`block 0.1.6` future-compatibility warning remains recorded.

Final gate log: 130,337 bytes, SHA-256
`2a7d60c10f84f6de45ce5ef68965421943b9bbfda88be3b0fb4df53647f79fd6`.
All five final commands have equal complete before/after maps and the same
541-input snapshot. Each leader was waited/reaped, its original numeric process
group observed absent, and empty private TMP removed. This is not a census of
escaped descendants. The OpenSSH receipt separately records 57 observed kernel
birth identities stopped, no unverified ancestry and private key/temp removal;
the author additionally observed that exact private root absent. Detached
descendants between ancestry observations remain outside that receipt's proof.

Native development binaries, not launched:

- App: 183,977,296 bytes, SHA-256
  `2ddf4b8f07bafd34ed39fb39e77568409084f23689fa078318fda585317e554b`.
- MCP: 16,716,032 bytes, SHA-256
  `b7fda1286acc599fae1b4fcec462fc1a197c28ba13158b8a7e5680fbe2b1fdc2`.

Only result documentation was updated after all final commands terminated.
Production/test Rust, manifests, lockfile, toolchain and gate scripts retain the
exact final command hashes. The new v2 packet includes the full 37-file candidate
and a 28-file repair delta against immutable v1, with all original review/failure
proof preserved separately; no v1 seal or mixed-input record was rewritten.

## Focused evidence before final freeze

Owned wrappers preserve every log and complete input map. Each listed run waited/
reaped its leader, observed its original numeric process group absent, and removed
empty private TMP. This is not an escaped-descendant census. All v2 runs
have unchanged before/after engineering inputs; none is mixed-input proof.

- Nine new actual TCP/SFTP mutation tests passed in `ssh-mutations-sixth`: wrapper
  17.380s, test 2.51s, 540 visible inputs unchanged. They cover all eleven existing
  public mutators plus the authorized wrapper against held second-WRITE active/
  unknown states, independent targets, preflight and pending revocation, cross-
  connection isolation, explicit STATUS rejection, sync double-root/local
  publication, 40 children without budget leaks, child overlap refusal, closing
  owners before I/O versus actual pending writes, cleanup without REMOVE reply,
  and a server that publishes but returns a non-STATUS rename packet.
- `app-isolation-second`: 4 passed, wrapper 23.716s, test 2.40s, 540 inputs unchanged.
  New real GPUI/TCP cases execute ordinary Save in three Files panels and actual
  MCP grant/proposal/native approval. Same targets are refused active and unknown,
  independent saves/proposals succeed, full compound paths/IDs appear in Chinese/
  English risk review, consent changes no bytes, and a new separately approved
  save/proposal succeeds afterward. Old unknown history remains unchanged.
- `ssh-all-fourth`: 123/123 actual TCP/SFTP, wrapper 64.301s including compile,
  test 26.64s; unchanged inputs. Later root-scope, owner-close and unexpected-
  packet cases require the final gate, rather than being attributed to this run.
- `app-sync-fourth`: 5/5 actual GPUI sync cases, wrapper 29.616s/test 1.22s,
  unchanged inputs; the exact root-directory upload/save behavior is restored.

## Preserved repair failures and intermediate scope

| Run | Actual finding / subsequent correction |
| --- | --- |
| `compile-first` | Workspace/all-targets compile passed before cleanup/lifecycle refinements; private visibility warning later removed |
| `tcp-mutations-first` | 5/6 passed; pre-I/O revoked authority was wrapped as SFTP instead of typed Closed; corrected before subsequent 6/6 |
| `ssh-all-first` | 118/122 passed; three old resume tests used an application writer to simulate external mutation, now assert refusal then use an explicitly outside-app fixture; zero-window preflight fixture could not answer canonical metadata |
| `app-isolation-first` | 3/4 passed; new MCP test constructed a queue without entering Tokio; fixed the test's runtime boundary |
| `ssh-all-second` | 121/122 passed; new sequential acknowledged low-level chunks did not saturate the old zero-window condition |
| `zero-window-third` | Original close/drop invariant passed with an intermediate large raw packet; superseded by bounded two-request batches before final verification |
| `ssh-all-third` | 122/122 passed with bounded two-request batches and actual WRITE framing/window evidence, before later cleanup test/root correction |
| `app-files-mcp-third` | 449/450 passed; canonical `/` sync root was rejected by atomic-file basename validation, leaving original bytes; fixed namespace root resolution without changing its old assertion |
| `ssh-mutations-fifth` | 8/8 passed before the final unexpected-rename-packet proof; historical scope retained |
| `final-gate-first` | Strict Clippy rejected unwrap and map_err used only for observation in the new window fixture; corrected after both the gate and packaging command were reaped. 541 inputs stayed unchanged during each run |
| First v2 sealed metadata / `freeze-verification` | Complete 37-file patch apply-check passed, but isolated reconstruction found no-index repair-delta headers retained stripped absolute prefixes. The original seal/archive and failed unchanged-input receipt remain immutable. v2b corrects only packet path headers and verifies actual 28-file delta application against all 37 candidate bytes before sealing; no production/test source changed |

The window fixture now uses ordinary canonical preflight and an independent owned
raw SFTP channel. It observes actual framed WRITEs, a consumed 64 KiB SSH window,
a pending bounded pipeline and remote child-channel closure on explicit close
and drop. Production requests remain 32 KiB with at most two per batch; every
reply is observed and missing replies take priority over a peer STATUS rejection.
It does not replace backpressure with a delay or a giant production packet.

## Unverified boundaries

No customer host, vendor process, cloud/model API, native window or installation
was started. GPUI/TCP/OpenSSH and native development builds do not establish
Windows/Linux/macOS desktop accessibility, production SSH, signed Release or
update acceptance. OS foreground worker-start failure is still not fault-injected;
existing busy preservation and full regression cover its invariant only.
Different endpoint aliases, external programs, source hard-link identity and
physical local/remote namespace sharing remain bounded. Quarantine is in memory,
not durable recovery; no automatic retry/replay/reconnect/restore is implemented.
Final author gates prove the scope above; fresh independent re-review and
combined root integration remain required.
