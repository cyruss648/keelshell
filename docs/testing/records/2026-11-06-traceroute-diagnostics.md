# Reviewed traceroute — r3 correction and final combined gate, 2026-11-06

## Source, authority and scope

The author worked only in the designated ignored, no-`.git` implementation copy
under `work/reviewed-traceroute-20261106/implementation-source`. Its input was
850b77b plus the parent's verified 536-input/18-path overlay. The planned and
actual source delta is 12 existing diagnostic-related core/session/app and test
paths, including five new files. Monitor sampling, knowledge, files, dependencies
and parent source/Git/package writes were outside the author scope.

On 2026-11-06 the parent delegated a three-file formal-document write scope
to the author within the user's broader authorization. Each document was
confirmed absent before creation. Indexes, README,
HANDOFF, ROADMAP, combination records and later gate/native updates belong to
the parent. This documentation phase runs no Cargo or network operation.

The single-tool and review contract is in
[TRACEROUTE_DIAGNOSTICS](../../product/TRACEROUTE_DIAGNOSTICS.md) and
[ADR 0112](../../adr/0112-reviewed-bounded-traceroute.md).

## Frozen source and independent review

| Artifact | SHA-256 |
| --- | --- |
| r2 aggregate input tree | `658f9e47f0e2cedba41bf28ad9bc64edbc7085dd1a3f80612b30dbb502b6bfec` |
| r2 source tar.gz | `7792d033033c582d479a8e10322e458799e1ee0cfb950a98a4885935ad53ff58` |
| r2 source-manifest.json | `8e460457049da1d9d1f6cdbdddba5e01b147393ab1bd7d7c2e171d6e3ea9c347` |
| baseline-to-r2 patch | `b4fc634bb6d07b1838316b7d4de02b90fb06f290258a921b598699707483f86f` |
| r1-to-r2 patch | `11834cd5741ae3b7a8770ae07df6b4488e23ef3cb7b3e8ccf8e053bb3d1d1195` |
| independent source-r2.json | `94bee87f963d8bad5b056421cc8daefb372f0dc8238e96336651b4b9c155ebc0` |

The archive contains 1032 ordinary files; this is not an engineering-input
count. Per-file hashes match before and after author validation. R1-to-r2 changes
exactly four Rust paths and adds no new test declaration. Both baseline and
necessary-delta patch application/readback passed.

Archimedes reviewed r2 at 2026-11-06T03:24:18.259688Z:
**APPROVE_FOR_SOURCE_ONLY**. TR-R1-P1-01 and TR-R1-P2-01 are
CLOSED_IN_FROZEN_R2, with zero open P0/P1/P2/P3 findings.
The reviewer independently checked saved author originals, hashes, named
inventories and exit/wait receipts; the reviewer did not rerun Cargo or sign a
complete gate.

The aggregate is reproducible from full archives: SHA-256 of UTF-8 concatenated
`lowercase-file-sha256 + two spaces + relative POSIX path + LF`, ordered by full
Path sorting. The r1 aggregate-method UNKNOWN is resolved by this later evidence,
without rewriting the original r1 report.

## Author Cargo validation

Hegel's explicit release was read before ownership transfer. The author used
the existing shared target, build jobs 2, test threads 4 and `--locked --offline`.
Only keelshell-app/core/session/ai/mcp artifacts were invalidated; third-party
cache was retained. Fresh raw Compiling lines identify the implementation copy.

| Actual inventory | Tests listed | Frozen new names present |
| --- | ---: | ---: |
| core library | 189 | 3 |
| session library | 134 | 7 |
| app binary tests | 1035 | 7 |

Every one of the 17 new names was checked individually. The inventory also
records actual test-binary paths, sizes and SHA-256. These are the author's
standalone r2 inventories, not the parent's later combination inventory.

| Command / scope | Actual result | Numeric exit | Actual wait/reap seconds | Original budget seconds |
| --- | --- | ---: | ---: | ---: |
| Five-package clean | Project packages only | 0 | 3.493642 | 120 |
| core inventory | 189 listed | 0 | 4.047734 | 300 |
| session inventory | 134 listed | 0 | 8.510225 | 420 |
| app inventory | 1035 listed | 0 | 36.965018 | 600 |
| core traceroute | 3 passed | 0 | 0.115995 | 120 |
| session traceroute | 7 passed | 0 | 11.039065 | 180 |
| app traceroute controllers | 7 passed | 0 | 5.478533 | 240 |
| core diagnostic regression | 4 passed | 0 | 0.121487 | 120 |
| session diagnostic regression | 14 passed / 4 ignored | 0 | 12.541710 | 240 |
| app diagnostic regression | 19 passed | 0 | 11.976060 | 240 |
| strict workspace all-targets Clippy | `-D warnings` | 0 | 19.170597 | 600 |

Totals: **17 new + 37 existing = 54 distinct Rust tests passed, 0 failed,
4 ignored**. All 11 commands and outer executions actually waited/reaped;
original deadlines were retained without extension. All owned process groups
were absent at release. App test linking retained the `__eh_frame` compact-unwind
capacity warning; no warning suppression or build-setting change was introduced.

The four ignored tests are the existing actual_services TLS/HTTP/DNS environment
cases. They were not enabled. The new Unix process-contract test runs the
12-test traceroute Python suite; the ICMP regression runs its 20-test suite.
These nested counts are not added again to the 54 Rust tests.

Earlier standalone Python traceroute r3 passed 12 tests, exit 0, actual wait
10.427177291 seconds; ICMP regression r1 passed 20, exit 0, actual wait
12.878502417 seconds. Own nine-path rustfmt check r5 exited 0 after
0.081327 seconds. Rust 1.99.1/aarch64-apple-darwin was used; Python 3.8 syntax
checks are not Python 3.8 runtime acceptance.

## Preserved failures and corrections

- Python traceroute r1 ran 12 tests with one failure, exit 1, actual wait
  9.867314667 seconds. A broad os.stat fixture replacement made the PID file
  appear present before the owned process started. Restricting the mock to
  the two tool-discovery paths corrected the fixture; later runs passed.
- Rustfmt r1 exited 1 after 0.050967250 seconds for a missing parenthesis in the
  new UI iterator. The correction and subsequent formatting/check results are
  retained separately.
- Independent source r1 retained P1 TR-R1-P1-01: the new `when` call lacked the
  FluentBuilder trait import. R2 adds the explicit import and app compilation
  passes. The review's standalone rustc negative control is not a failed r1
  project build; no r1 Cargo build was claimed.
- Independent source r1 retained P2 TR-R1-P2-01: 20 new test expect/expect_err
  sites violate workspace deny. R2 replaces them without weakening assertions,
  lint policy or the 17 declarations; strict all-targets Clippy passes.
- A hash-verifier attempt mistakenly used the reviewer's 17-file excerpt as a
  full source copy, exit 1. Its original output remains; subsequent verification
  from the complete r1 tar matches all 1032 files and reproduces the aggregate.
- The first release audit wrongly required zero Cargo processes system-wide,
  exit 1. Read-only cwd and rustc out-dir evidence identified a separate project
  and target. No unrelated process was signalled. Corrected scope checked only
  this slice's actual waits and owned process groups before release.

These failures remain in ignored work evidence and are not overwritten or
relabelled as successful runs. Original r1 source, patches and review remain
unchanged.

## Release, parent combination and open boundaries

The author explicitly released the Cargo slot at
2026-11-06T03:11:25.889148Z, or 11:11:25 Asia/Shanghai.
The final Cargo outer session 99251 was actually waited, exit 0/reaped.
No author Cargo has run after release; the parent owns the current slot.

The parent's 536-input preservation check applies to the author's actual
execution/release window. The delivered 20-document freeze was separately
verified against fa7c05914e0eefc0f24ab32f8af690bdc50a63ed blobs.
Later authorized editor/traceroute assembly, branch metadata and formal
documentation progress are not drift during that past window.

At the earlier r1 handoff, the parent declared a
543-engineering-input/21-source-path combined freeze
at `work/next-batch-20261106/final-source-r1/freeze.json`, whose readback SHA-256
is `6ac76fd88bbf245e49663961d8f801e4e7b8ec88478e02e1313adc208ffc68de`.
It includes the parent's core/lib.rs merge. At that earlier handoff, combined
inventory/full gate/package remained pending; later parent records own their
results. Source-only approval and standalone inventories do not sign that merge
or combination.

At that earlier handoff, unverified here: real Linux traceroute 2.1.5,
actual IPv6/kernel/privilege
behavior, complete real-SSH cancellation/remote-reap behavior, native GUI and
macOS/Windows/Linux desktop acceptance, complete workspace/full scripts gate,
combined package, CI, release and installation. The macOS supervisor test
honestly returns UnsupportedPlatform; GPUI controller tests are not native UI.
The product guide's minimal loopback case is an expectation for future parent
execution, not an executed receipt.

Local evidence root: `work/reviewed-traceroute-20261106/`, including
`freeze-r1/`, `freeze-r2/`, `independent-review/source-r1.*`,
`independent-review/source-r2.*`, `validation-r2/`, `freeze-r2-validation.json`,
`cargo-slot-release-r2.json` and retained Python/rustfmt failures. Ignored local
paths are not a claim that raw evidence is published with the repository.

## R3 increment: real version-stream mismatch

The later parent owned image
`sha256:0223b4051699eaa0b626fa0762a8bd0f7c7e12ffc7299f0ef4d0bec4e7d5c4ca`
reports `/usr/bin/traceroute --version` as exit 0/reaped, empty stdout and
exactly the 110-byte existing TRACE_VERSION on stderr. Raw stderr SHA-256 is
`b51b0c2e7d6c7488135626e2c3bf121cd4d36acb9f2a10a67adef87d79c0c3e0`.
Originals are in the parent's
`work/next-batch-20261106/image-preparation/tool-versions.json`,
`traceroute-stderr.bin` and `traceroute-stdout.bin`.

R2's production condition required TRACE_VERSION on stdout and rejected stderr.
Its fake tool used the same wrong stream, leaving a real capability blocker
despite historical source-only approval and passing offline checks.
R2 is not a release candidate; those receipts remain historical, not erased.

The r3 production correction requires all three conditions: exit zero,
stdout empty, stderr exactly TRACE_VERSION. It never joins streams or accepts
substrings, dual-stream output, extra bytes or an unknown version.
Only three source paths change from r2:

- `network_diagnostic/traceroute.py`: the exact stream predicate and explanatory
  comment; parser, limits and actual probe argv remain unchanged.
- `network_diagnostic/tests/test_traceroute.py`: faithful stderr fake and two
  focused version-stream tests.
- `network_diagnostic/tests/traceroute.rs`: exactly
  `#![cfg(test)]` followed by one blank line, as supplied by the parent.
  The separate parent remote-only guard failure/review is not author-validated.

The new positive case
`AdapterTests.test_exact_upstream_stderr_version_runs_resolver_and_probe`
actually failed against the old r2 production predicate: exit 1, actual
wait/reap 0.372565167 seconds, original budget 30 seconds, expected success
received UnsupportedEnvironment. This counterexample failure remains in
`validation-r3/r2-stderr-positive-counterexample/`.

After the predicate correction, the complete offline traceroute Python suite
passed **14/14**, exit **0**, actual wait/reap **10.891797375 seconds**, original
budget **45 seconds**, outer session **20066** actually waited/reaped.
No timeout or deadline extension occurred. The other new case,
`AdapterTests.test_version_admission_rejects_stdout_both_split_extra_nonzero_and_other_versions`,
has ten negative subcases: stdout-only, both streams, split version,
extra stdout byte, leading stderr byte, trailing stderr byte, missing final byte,
nonzero exit, another version and another implementation. Every rejection
occurs after exactly one version process, before resolver or probe.
The full suite also exercises cancel/actual child reap and the existing deadline.

There are still 17 Rust test declarations; none were added in r3 or rerun for r3
by the author. The later parent final-gate execution is recorded below.
No r3 Cargo, Clippy, real tool, SSH, Podman or native GUI was run by the author.
At the correction handoff, the parent retained exclusive Cargo ownership and
its in-progress r2 fullgate; the author did not modify that combined source.
Parent raw-tool receipt
`work/next-batch-20261106/raw-tool-probes-r1/receipt.json` reports UDP IPv4
127.0.0.1 RTT 0.079 ms and IPv6 ::1 RTT 0.042 ms, each one hop/exit 0/empty
stderr, and ICMP exit 1 with the existing exact two-line permission errno.
Those are raw image-local loopback observations, not production SSH/native
acceptance. No parser expansion was made.

R3 source tree SHA-256 is
`522e26e923dffd7678185ab7d5a412b4521a5b8ff17f7c2e0d1ea8d8e12dda37`;
the minimal r2-to-r3 patch SHA-256 is
`c100d01294a4504c0d552a8a7d98c31283f30d1fe7a7a5cb6e85bfcee56abd09`.
New artifacts live in `work/reviewed-traceroute-20261106/freeze-r3/` and
`r3-correction/`. Old r1/r2 archives, approvals, failures and validation remain
unchanged. At that correction handoff, r3 incremental source approval and the
parent's final combination full gate/build were pending; later native results
require their own receipt.

Archimedes subsequently approved the isolated frozen r3 necessary delta at
2026-11-06T04:04:14.888326Z: **APPROVE_FOR_SOURCE_ONLY**, with zero open
P0/P1/P2/P3 findings. `independent-review/source-r3.json` binds the same r3
source-tree and minimal-patch hashes above and closes CR2-P1-01 in frozen r3
source only. The independent review does not sign the parent's combined
candidate, final Rust inventory, gate, package or native behavior.

## Parent production-script red/green follow-up

The parent imported the two corrected Python files into its separate
543-engineering-input/21-source-path r3 candidate; the Rust test prefix already
matched exactly. Readback of
`work/next-batch-20261106/final-source-r3/freeze.json` gives SHA-256
`1b5c7cf42d8a4d9ede5de6fcca29caf11da5daeaef59fc8e580ea4a1172590bd`.
This is the combined freeze-file hash, distinct from the author's r3 aggregate
source-tree hash above. The isolated r3 necessary delta has the independent
source-only approval above; combined source/gate closure remains parent-owned.

The parent executed the exact production Python `SCRIPT = icmp + trace + probe`
with the actual worker, on the owned image identified above. Saved container
inspection confirms network `none`, UID/GID `1001:1001`, a read-only root,
no effective/bounding capabilities and no privileged mode. Requests used
3 maximum hops, 1 probe per hop and 100 ms wait, with loopback destinations.
The author read back the saved component hashes, actual script/worker arguments,
requests, framed responses, inspections and wait/removal receipts; the author
did not rerun these probes.

| Parent run | Actual framed result | Observed hops / RTT | Total probe-batch wait seconds | Original budget seconds |
| --- | --- | --- | ---: | ---: |
| Old predicate, UDP IPv4 | UnsupportedEnvironment / UnsupportedImplementation | No resolver/probe result | 0.414948583 | 45 |
| R3 UDP IPv4 | Success / DestinationResponded | TTL 1, `127.0.0.1`, 44 us | 1.205782500 for all three green cases | 45 |
| R3 UDP IPv6 | Success / DestinationResponded | TTL 1, `::1`, 51 us | Included in green batch | 45 |
| R3 ICMP IPv4 | PermissionDenied / PermissionDenied | No hop; no privilege escalation | Included in green batch | 45 |

The red batch ended at 2026-11-06T03:58:09.115886Z and the green batch at
2026-11-06T04:00:57.592376Z. Every production-worker invocation, container
inspection and removal exited 0, was reaped and stayed within the original
deadline; containers were stopped and removed. The ICMP permission result is
inside the structured response even though its production-worker/container
exit is 0; the earlier raw-tool ICMP exit 1 is a different process boundary.
Outer readback records red `functions.exec_command` completion and green
session 28793 `functions.write_stdin`, each actual exit 0/reaped.

Parent originals remain under
`work/next-batch-20261106/production-probe-r1-red/` and
`work/next-batch-20261106/production-probe-r2-green/`, including `receipt.json`
and `exec-readback-r1.json`. Their receipt SHA-256 values are respectively
`45cd60fc22e83683069c2c7ad7d4adb73257b5828f1beed8cfbf56652a616785`
and `8d8bb42b8b9980837286e7409c792d82e4e9fbcfe7e38c38da758096e82f58bf`.
The red failure is preserved as evidence of the real version-stream defect.

These observations establish owned Linux loopback execution through the exact
production Python adapter and worker. Rust response parsing, SSH transport,
review/confirm UI, complete cancellation/remote reap and native application
acceptance still require their own evidence. At that follow-up the parent
reported final fullgate-r3 running in foreground session 5546 and continued to
own Cargo. Its later terminal gate/build/package receipts are recorded below;
no commit or publication result is claimed here.

## Parent final r3 engineering gate and macOS artifacts

The parent subsequently completed `python3 -B scripts/check.py` against the
543-input final combined r3 freeze identified above. The author read back the
saved originals without running Cargo or repeating tests. The gate result
records exit 0/reaped, unchanged engineering inputs and no deadline extension.
The corrected external receipt records session **5546**, an actual
`functions.write_stdin` wait, exit **0** and external reap. It supersedes the
earlier external readback; the old receipt remains preserved.

| Final gate scope | Passed | Failed | Ignored / skipped |
| --- | ---: | ---: | ---: |
| Ordinary Rust tests | 2355 | 0 | 23 ignored |
| App subset of ordinary tests | 1047 | 0 | 2 ignored |
| Rustdoc tests | 14 | 0 | 0 |
| Gate Python driver | 130 of 131 run | 0 | 1 skipped |

All **42 combined new names** and **204 selected baseline names** actually
passed in the parent's named verification. The 42 include all **17 traceroute**
names, individually matched to `test ... ok` in the raw final-gate log during
this documentation readback. The 42 and 204 are subsets of the ordinary gate
results. The standalone r3 **14-test traceroute Python** receipt remains the
earlier exit 0 / actual wait 10.891797375 seconds / original 45-second budget;
it is not added again to the gate Python or Rust totals. Historical author r2
17-new/37-regression/4-ignored and strict Clippy receipts remain historical.

| Parent command / scope | Numeric exit | Actual elapsed through wait/reap seconds | Original budget seconds | Child and external execution reaped |
| --- | ---: | ---: | ---: | --- |
| Final r3 engineering gate | 0 | 575.972411125 | 1500 | Yes |
| Normal workspace application build | 0 | 12.185535959 | 1500 | Yes |
| macOS aarch64 package generation | 0 | 0.318052959 | 1500 | Yes |
| macOS package static inspection | 0 | 0.267179291 | 1500 | Yes |

The gate ran from 2026-11-06T04:02:13.376568Z to
2026-11-06T04:11:49.324102Z; its corrected outer receipt was recorded at
2026-11-06T04:13:50.628066Z. All four commands retained the original
1500-second deadlines and recorded unchanged 543 engineering inputs.
The normal workspace `cargo build --workspace --locked` produced the debug
application used for the macOS `aarch64-apple-darwin` package, distinct from
the app test executable. The package manifest records application SHA-256:

```text
9e83d83af253ffe3c3aba29997450ea75f4c70a6dbe9311e61469228bd0158b9
```

The package and `packaging/inspect_native.py` results establish generation and
static inspection of that artifact. The manifest records `installed: false`
and `native_acceptance: not performed by this script`.

Parent originals are under `work/next-batch-20261106/`:

- `full-gate-r3-result.json`, `full-gate-r3-summary-r2.json`,
  `full-gate-r3-exec-readback-r2.json` and `full-gate-r3.log`.
- `inventory-verification-r3.json`, with unchanged app/core inventory references
  and the fresh session inventory; the final gate's actual named passes remain
  the execution evidence.
- `workspace-build-r1-result.json` and
  `workspace-build-r1-exec-readback.json`.
- `package-macos-r1-result.json`, `package-macos-r1-exec-readback.json` and
  `package-macos-r1/package-manifest.json`.
- `inspect-macos-r1-result.json` and `inspect-macos-r1-exec-readback.json`.

Raw final-gate log SHA-256:
`8e1ee2a9321b1fec027c4a31d8e41f69aca431e28a700bbeb2a24e4cd8bee17b`.
The author matched the recorded result-file hashes to each external receipt,
the log hash to the corrected summary and the freeze-file hash to the final r3
identity. This is readback of parent executions, not a new author test run.

Archimedes's **APPROVE_FOR_SOURCE_ONLY** remains approval of the exact isolated
frozen r3 necessary delta, with zero open findings. The parent's final gate and
artifact receipts are separate engineering evidence. Scoped native observations
are now recorded in the parent's [native follow-up](2026-11-06-editor-traceroute-native.md):
the same package's real SSH UI reviewed and confirmed UDP IPv4/IPv6 loopback
separately and returned one-hop destination replies. Native-r1 independently
approved the observed subset; complete real-SSH cancellation/remote-reap,
Windows/Linux desktop, Release and
installation remain open. The original r1/r2
failures, rejected version-stream counterexample and all old freezes remain
preserved; these results do not declare the complete product accepted.
