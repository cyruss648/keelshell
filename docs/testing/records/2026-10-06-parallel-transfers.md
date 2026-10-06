# Parallel remote transfer author candidate — 2026-10-06

Status: final author validation passed; frozen candidate awaiting fresh independent
review and root integration. Not committed, pushed, released or accepted as a
native desktop product. Base: `e247f2f`, branch `feature/parallel-transfers`.

Only Files/transport and related tests/documentation changed. The author used a
separate APFS-cloned Cargo target and unique private scratch per owned command.
Registry requirements, Cargo.lock and the exact toolchain pin are unchanged.
See [ADR 0055](../../adr/0055-parallel-remote-transfers.md).

## Final evidence

| Final command | Actual result | Input/ownership boundary |
| --- | --- | --- |
| Focused real TCP/SFTP convergence suite (before final busy-state guard) | 11 passed; 26.320s including compile | 536 visible Git engineering inputs unchanged |
| Files convergence selection (before final busy-state guard) | 53 passed; 32.085s including compile | Same 536 inputs unchanged; real GPUI controls and TCP/SFTP peers |
| `python3 -B scripts/check.py` | exit 0; 176.936s; 1,190 ordinary Rust / 8 rustdoc / 6 script tests; 42 ordinary + 4 doc harnesses | 536 inputs unchanged; format, strict workspace/all-targets Clippy, x.y policy, default and explicit 2 MiB controllers passed |
| System OpenSSH interoperability | 10 passed; script 9.280s / wrapper 9.335s | Same inputs unchanged; disposable localhost sshd, verified key, owned process/key cleanup |
| Packaging regressions | 57 passed; script 1.366s / wrapper 1.474s | Same inputs unchanged; package structure checks are separate from GUI acceptance |
| Native development build | app + MCP build exit 0; 4.489s | Same inputs unchanged; arm64 Mach-O, minimum macOS 15.0; neither executable launched |

The complete final gate contains 448 application tests, 68 session unit tests and
116 SSH loopback tests. The focused selections overlap those totals and must not
be added again. Twelve explicitly ignored choices remain: two vendor CLI opt-ins
and ten OpenSSH tests; the latter were executed separately. Default and 2 MiB
local-agent controller futures were both 5,040 bytes; their scenario groups are
separate from ordinary harness totals. The pre-existing transitive `block 0.1.6`
future-compatibility warning remains recorded.

The final fifth full-gate log is 128,785 bytes, SHA-256
`dcc8c776e3a751a8466b42f9a2ee973084e8222a60bcc2fedf506539188e599c`.
Every final wrapper waited/reaped its owned leader, found its original numeric
process group absent and deleted empty scratch. That observation is not an
escaped-descendant census. The OpenSSH script additionally verified 50 observed
kernel birth identities stopped, no unverified ancestry, and private key/temp
root removal; its receipt explicitly preserves the between-observation boundary.

Development binary hashes:

- App, 183,556,448 bytes:
  `ffabe8d20f0da48f7fac7699cc83974b607ebdaf9bcf3190b2b2b8c15c8caf10`.
- MCP, 16,716,032 bytes:
  `b7fda1286acc599fae1b4fcec462fc1a197c28ba13158b8a7e5680fbe2b1fdc2`.

Result documentation was updated after the checks; production/test Rust,
manifests, lockfile, toolchain and gate scripts still match the final gate's exact
input hashes. The frozen packet retains original logs, receipts, before/after
maps, candidate copies and patch separately from subsequent root integration.

## Meaningful acceptance coverage

- Two independent upload handlers must enter held second-WRITE responses before
  either is released. Existing final targets remain unchanged; distinct final
  bytes and reduced-concurrency behavior are checked. The same production path
  runs in a current-thread Tokio runtime on an explicit 2 MiB stack.
- Two download handlers enter independent held READs before release. Real partial
  file sizes and exact final content are checked; a pending READ cancellation
  has no pending destination mutation and retains an honest partial local file.
- Same-target and parent/child tree exclusion, queued pre-I/O cancellation,
  paused slot retention, independent progress and 32-entry admission are tested.
  Local paths are reserved across separate SSH connections, even different peers.
- Delayed, unacknowledged atomic WRITE cancellation reports unknown acknowledged
  bytes, preserves an existing final target and blocks conflicting queues and
  new connections. Independent targets still finish while the old WRITE is held.
- Explicit inspection/consent lists complete targets and process-unique IDs.
  A different connection, revoked consent, changed target observation, changed
  quarantine set, replayed consent and cancellation during held LSTAT reject
  release. Only exact reviewed records are removed; old jobs remain unknown.
- Real GPUI tests require separate approval for every upload, retain exact
  session/generation identity, verify Chinese/English fixed review buttons and
  actual nested platform wheel scrolling, and revoke late inspection/confirmation
  on suspension/cancellation. Losing pause control waits for the real terminal
  result instead of fabricating acknowledged cancellation.
- Real SFTP STATUS rejection of WRITE and missing resume CREATE remains failed,
  preserves output appropriately, creates no quarantine and permits a freshly
  reviewed follow-up. The transport keeps raw STATUS/local completion at those
  mutation boundaries rather than losing that proof in a mapped generic error.
- System OpenSSH verifies distinct hard-link pathname replacement and relative
  plus parent-symlink canonical aliases. Alias checks have spare worker slots,
  so an occupied concurrency limit cannot make them pass accidentally.

## Preserved failures and intermediate scope

All named logs/receipts remain in the private author packet; none was overwritten.

| Earlier run | Actual finding and disposition |
| --- | --- |
| `check-first` | Compile failed: missing scroll trait import; corrected before the next workspace check |
| `check-second` | Workspace/all-targets check passed on the earlier queue; does not cover later resource/consent changes |
| `tcp-first`, `tcp-all-first` | Five targeted / 111 full SSH tests passed before app-scope isolation; the earlier fresh-connection reset assumption was subsequently removed |
| `ui-first` | 19 passed / 3 failed: controlled upload gates still selected the old in-place destination; card visibility changed |
| `ui-second`, `ui-third` | 23/1 followed by 24/0 after real nested-scroll/staged-output checks; earlier connection-scoped implementation only |
| `check-global` | Workspace/all-targets compile passed before final consent/revocation changes |
| `tcp-global-first`, `tcp-global-second` | 7/1 then 8/0: new download assertion assumed a 32 KiB chunk; actual transfer chunk is 64 KiB, now asserted exactly |
| `ui-global-first` | Compile failed: new tests needed direct TransferSpec/TransferEvent imports; inputs also changed during this intermediate run |
| `ui-global-second` | 27 passed, but inputs changed while running; not final proof |
| `final-gate-first` | Strict Clippy rejected unused fixture metadata hold in standalone SSH tests; a meaningful held-LSTAT revocation test now uses it |
| `final-gate-second` | Strict Clippy rejected missing AtomicBool qualification in a new test; corrected after the run |
| `final-gate-third` | exit 1; 446 app passed / 2 failed: old worker-loss expectation and footer-scroll helper. Four session inputs changed during this run; it is mixed-input evidence and cannot prove any final gate |
| `focused-ui-final` | Compile failed: borrowed test ID required an owned SharedString; fixed before the passing fresh selection |

The third mixed run's complete log/receipt and both 536-input maps identify the
four changed files. The author waited/reaped it before further fixes, then
converged the targeted 11 SFTP and 53 Files tests before freezing and running the
fourth full gate. That fourth gate and its OpenSSH/build/packaging checks passed
with unchanged inputs. During the subsequent packet audit, the author found a
foreground worker-start failure branch that cleared the aggregate busy flag even
when parallel jobs remained. After all fourth-run commands had terminated, that
branch was changed to preserve `has_active_transfers()`, keeping unrelated
mutations blocked. The final fifth full gate and fresh OpenSSH/build/packaging
commands then passed with all 536 inputs unchanged throughout each command. No
Rust, manifest, lockfile, toolchain or gate script was edited after those runs.
The OS worker-start failure itself was not fault-injected; the invariant fix and
full regression evidence do not claim that additional acceptance.

The fourth full-gate log and receipt remain retained: 282.330s, 129,080 bytes,
SHA-256 `ce6e8ca3656e911c42446af02cbe925f8ea37f3b6239c25df33dcb55c95300c1`.
The earlier native binary hashes and 53 observed OpenSSH process identities are
preserved under their original filenames; final values above come from fifth
receipts and `native-binaries-fifth.json`.

## Explicit boundaries

No customer host, vendor CLI, model API, telemetry upload, GUI startup or
installation was used. GPUI/TCP, controlled localhost OpenSSH, development builds
and package structure checks do not establish macOS GUI, Windows/Linux desktop,
production SSH, signed release, installation or update acceptance.

Resource exclusion applies to these transfer queues. Different host aliases,
external programs, other non-queue mutators and physical sharing between local
and remote filesystem namespaces are not fully identified. Existing in-place
writes retain conservative whole-side exclusion; ordinary atomic uploads and new
local downloads preserve useful distinct-target concurrency. Read-only risk
inspection does not prove late writes stopped. Quarantine is process memory,
not a durable journal; reconnect/restart cannot be represented as rollback.
Automatic transfer recovery, replay and reconnect remain outside this slice.
