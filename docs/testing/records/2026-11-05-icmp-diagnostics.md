# ICMP diagnostics candidate — 2026-11-05 / 2026-11-06

## Input and scope

Started on 2026-11-05; targeted execution crossed midnight into 2026-11-06
(Asia/Shanghai). Existing managed worktree, branch
`feature/reviewed-icmp-diagnostics-20261105`, exact base
`dcf7438a13fff737997ee5e8061044c94d4ba53a`; initial status clean and
`origin/main...HEAD = 0 0`. No new worktree, commit, push, parent-tree write,
installation, Podman, native GUI or real ICMP endpoint operation.

Write scope is core/session diagnostic admission, the remote probe/adapter and
their tests, the app protocol panel and its GPUI protocol tests, and the three
new ICMP documents. HANDOFF/ROADMAP/README/Hallmark and workspace/terminal/files
source are outside this slice. Existing work/evidence are retained.

Read AGENTS/HANDOFF/RTK and both requested Rust skills. Upstream provenance and
the narrow admitted Linux iputils 20240117 contract are in the
[product guide](../../product/ICMP_DIAGNOSTICS.md) and
[ADR 0105](../../adr/0105-reviewed-bounded-icmp-diagnostics.md).

## Verification receipts

Each bounded execution has an unchanged engineering-input before/after
manifest, complete output log, numerical exit code, child PID, actual
`Popen.wait` reap, elapsed time and timeout flag. Manifests cover 569 non-doc
versioned/unignored input files; they exclude ignored work/build artifacts.
Each command's budget includes compilation. No budget was extended.

- Standalone offline Python r1: **19 tests passed**, exit **0**, actually reaped,
  **11.950 s**, fixed **45 s** budget, 569 inputs unchanged.
  All ping executables are local fakes; DNS stages print constant fixture data.
  Includes real subprocess pipe limits, missing executable, denied permission,
  exact/unknown version, full/partial/zero replies, contradictory exit,
  unknown/stderr/overflow output, direct-child kill and actual reap, closed pipes,
  denied kill/cleanup unknown, full-supervisor repeated shutdown and 7.5-second
  runtime budget. This is not real ICMP acceptance.
- Core r1: **4 passed**, exit **0**, actually reaped, **22.543 s**,
  fixed **240 s** budget, 569 inputs unchanged.
- Session r1: exit **101**, actually reaped, **49.137 s**, fixed **480 s** budget,
  569 inputs unchanged. **13 passed / 1 failed / 4 ignored**. Retain this failure.
  The old repeated-shutdown fixture passed a raw PID-file path as the request
  argument; structured request admission prevented its worker from starting.
  The fixture was changed to carry the same owned PID path inside base64 JSON;
  production validation and its three-second startup budget were preserved.
- Session r2 after that fixture correction: **14 passed / 4 ignored**, exit
  **0**, actually reaped, **12.856 s**, fixed **480 s** budget, 569 inputs
  unchanged. Includes authenticated 127.0.0.1 request/reply/cancel fixtures,
  real Python resolver/HEAD loopback, the legacy supervisor actual-reap
  regressions, strict ICMP wire validation and the 19-test fake process suite.
- App r1: command-selection failure, exit **101**, actually reaped, **0.439 s**,
  fixed **480 s** budget, 569 inputs unchanged. `--lib` was incorrect for this
  binary-only package; no compilation or tests ran. Preserve its log.
- App r2 using `--bin keelshell-app`: **8 passed**, exit **0**, actually reaped,
  **63.098 s**, fixed **480 s** budget, 569 inputs unchanged. Production GPUI
  controls cover exact review and normalized-equivalent input edits/reverts,
  held replies after input/cancel/session revocation, current route/pin/closed
  tab checks for both DNS and ICMP, structured ICMP partial-loss rows, and
  bilingual controls across the three registered themes and light/dark
  appearances without dispatch. The existing six language/appearance layout
  assertions also ran. These are controlled GPUI tests, not native acceptance.
  One linker warning was observed: `__eh_frame section too large (max 16MB)` for
  compact-unwind encoding; the complete message remains in the log.
- Strict `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`:
  exit **0**, actually reaped, **21.688 s**, fixed **480 s** budget,
  569 inputs unchanged.
- `cargo fmt --all --check`: exit **0**, actually reaped, **1.613 s**,
  fixed **60 s** budget, 569 inputs unchanged.
- `python3 -B scripts/check.py --policy-only`: x.y policy passed, exit **0**,
  actually reaped, **0.067 s**, fixed **30 s** budget, 569 inputs unchanged.

Targeted Cargo uses `CARGO_BUILD_JOBS=2`, the parent-assigned shared target,
`--locked --offline`, and `--test-threads=4`. No full `scripts/check.py` run.
Dependency validation is only its explicit `--policy-only` mode.
Session r2, app r2, Clippy, fmt and policy bind the same final 569 engineering
inputs. `gate-bindings.json` separately binds the earlier core result to all
88 core files and the standalone Python result to the final three Python inputs.
No final-combined full gate or additional ignored real-service suite was run.

## Acceptance boundaries

- **OPEN**: parent integration of a read-only active-selection generation.
  Existing captured connection/route/trust checks do not observe a pure
  switch-away/switch-back generation. Workspace writes are prohibited here.
- **OPEN**: isolated real Linux iputils 20240117, actual ICMP IPv4/IPv6 and
  real permission/capability behavior. Fake processes prove adapter/lifecycle
  logic, not network protocol or genuine utility acceptance.
- **OPEN**: native macOS/Windows/Linux, real AX/keyboard/VoiceOver, complete
  theme/language/size matrix and final combined full gate/non-author review.
  Controlled GPUI assertions do not establish native acceptance.
- **UNSUPPORTED by design**: other ping implementations/versions, automatic
  install, sudo, TCP/local fallback, alternate addresses and experimental JSON.
- Cancellation/transport failure does not prove immediate peer-process death.
  The independent remote deadline remains; uncertain ICMP channel termination
  retires the diagnostic authority. CleanupUnknown is preserved when actual
  kill/wait cannot confirm exit.

## Frozen delivery and release

The candidate is **13 paths**: 10 diagnostic source/test files and these three
new documents. `worker.py`, dependencies/lockfile, HANDOFF/ROADMAP/README and
workspace/terminal/files/Hallmark source remain unchanged. No commit or push.

Local-only retained evidence root:
`work/reviewed-icmp-diagnostics-20261105/candidate-r1/`.
It contains complete logs, both failed attempts, per-command before/after
manifests and receipts, downloaded upstream tag source bytes, actual preimage
files, `preimage-manifest.json`, `source-sha256.json`, `gate-bindings.json`,
`candidate.patch`, final `freeze.json`/path hashes and `cargo-slot-release.json`.
The patch includes new files and does not depend on Git staging.

SHA-256 of `source-sha256.json`:
`96deb9c840b8a66ecbb754cdd0136f31e60f98b523a30435c3d4d18ac3dd5968`.
SHA-256 of `preimage-manifest.json`:
`be94d532c3301f86e6051604b09e429feb57f42a85e78c818154e05f9bfab3c7`.
Final patch and all 13 content hashes are in `freeze.json`; the document does
not embed its own containing patch hash.

The shared Cargo slot was **released at 2026-11-06 00:22:23 Asia/Shanghai**
(`2026-11-05T16:22:23.657826+00:00`). All recorded command subprocesses were
actually waited/reaped; no Cargo commands remain running for this slice.
Only evidence/document finalization followed release. Parent combination must
run its one full gate and independent review, and close the selection-generation
integration gap before claiming MON/NET-02 complete.

## Candidate-r2 increment — ICMP-R1-02

On 2026-11-06 (Asia/Shanghai), Goodall's scoped review identified that quiet
iputils summary counters do not prove successful sends or network packet loss.
The retained 20240117 upstream `ping_common.c` hard-local-error path advances
`ntransmitted` after failure. The IPv4/IPv6 local-error quiet branches can return
before `nerrors++`. Four modeled `send_probe=-1/EPERM` results, an empty error
queue and no replies can therefore share the accepted quiet summary and exit 1
with four successful probe calls that receive no replies. The former history
has zero successful probe calls. Mixed local failures and partial replies have
the same ambiguity. This is a source-backed counterexample, not an observed
endpoint failure or syscall-injection result.

Correct the public session count/status rustdoc and product/ADR contract to
say **utility-counted send attempts** and **unanswered attempts**. A zero error
counter does not rule out local send failures; the quiet summary cannot recover
a hidden EPERM or justify a PermissionDenied classification. Retain the existing
`IcmpPartial`/`PacketLoss` wire category without attributing a network-loss cause.
RTT describes observed echo replies; percentage validation checks summary
arithmetic. Adapter executable behavior, wire schema, fixed argv, version
admission and budgets are unchanged.

The six-path increment contains session rustdoc, `icmp.py` comments,
`tests/test_icmp.py`, and these three ICMP documents. Both app files, core and
the other original diagnostic paths remain byte-identical to candidate-r1.
The parent owns app wording, ICMP-R1-01 all-Err authority retirement, SSH
terminal-frame regressions and Workspace selection-generation integration.
Those changes are outside this increment's validation.

Fresh r2 checks:

- Offline Python: **20 tests passed**, exit **0**, fixed `Popen.wait` budget
  **45 s**, wall time **12.263 s** (suite time **12.190 s**), no timeout.
  PID **98543** was actually waited/reaped; a subsequent `waitpid` confirmed
  there was no remaining child to reap. All 13 candidate-path input hashes were
  unchanged during execution. This testing record was finalized afterward.
  Complete command, log, numerical exit/reap and before/after input manifests
  are in `candidate-r2/python-offline/`.
- The new fake-process test runs four modeled histories: all hard-local-error
  versus all-successful-probe calls with zero replies, and mixed hard-local-error
  versus all-successful-probe calls with two replies. Each pair produces the
  same counters/reason despite different successful-probe counts. No genuine
  iputils binary, network resolver or packet operation is used.
- Static comparison: removing only Rust `///` lines leaves identical bytes;
  the adapter Python AST and all non-comment tokens are identical. Test Python
  parses successfully. All six increment preimages match candidate-r1 hashes,
  and its seven other candidate paths remain unchanged.
- All **61** original files under `candidate-r1/`, including the original
  patch, freeze, preimages, upstream bytes and gate evidence, retain their
  original hashes. r2 evidence is separate under
  `work/reviewed-icmp-diagnostics-20261105/candidate-r2/`.

The delivery is `candidate-r2/increment.patch`, strictly from r1 preimages to
the six r2 files, with per-path before/after SHA-256 in
`candidate-r2/paths-before-after.json` and a separate freeze receipt.
**No Cargo, Rust compilation/rustdoc compilation, Clippy, fmt, GPUI, GUI,
Podman or real ICMP checks were run for r2.** The shared Cargo slot remains
released; no target access or new reservation occurred.

Parent integration, the combined gate, the scoped independent re-review and
real Linux iputils/native acceptance remain **OPEN** for this delivery.
The original r1 receipts above remain historical evidence at their recorded
inputs; they are not newly executed r2 gates.
