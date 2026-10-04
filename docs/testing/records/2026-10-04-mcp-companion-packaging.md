# MCP companion packaging and update recovery — 2026-10-04

Base: `main` commit `3137712`; branch `feature/mcp-companion-packaging`.
The desktop bridge is integrated separately. This record covers companion delivery
and isolated updater behavior; it does not claim external-agent or GUI acceptance.

## Implemented requirement

MCP-04 delivery now explicitly packages `keelshell-mcp` beside the application on
all six target triples. The Release workflow builds both crates in one invocation.
The extended schema-1 receipt requires `mcp_binary_sha256` and its fixed `files`
entry. Release validation checks both architectures, hashes and Unix modes; native
inspection checks both dependency sets/minimum versions. New update validators
reject historical incomplete packages before writing; old helpers can read the
extended schema and copy the additional file through their existing file list.

The updater checks the companion at download, helper and replacement time. Late
failure restores both images, or removes a newly added companion on older installs.
Failed restoration retains the exact old backups, blocks retries/restart and adds
a bilingual local recovery diagnostic. See [ADR0041](../../adr/0041-mcp-companion-packaging-and-update-recovery.md).

## Evidence

Evidence lives in ignored `work/mcp-companion-evidence/` and is retained for root
integration. The initial code/config hashes were frozen in `frozen-code.json`; all eight
hashes were rechecked after that snapshot's gates, build and native probes. The
12-file original review snapshot and its evidence remain intact. Its subsequent
independent review found the repeated-recovery defect recorded below; the native
artifacts in this section predate that repair.

- `baseline-recovery-probe.rs` extracts the exact `3137712` rollback/cleanup
  functions and invokes them on owned temporary files. `baseline-recovery-failure.log`
  records the expected nonzero result: a blocked restore retained the old image
  before cleanup, then the old helper cleanup deleted it. No real installed app was
  targeted; the probe demonstrates that isolated original error path only.
- Initial companion staging/release/publication tests: 57 passed. Six-target
  synthetic headers are never executed. Cases cover required input, same-directory
  placement, missing/corrupt receipts and files, companion architecture/truncation,
  external checksum failing to mask internal tampering, modes and native inspection
  contracts. Unix mode-dependent tests are explicitly skipped on Windows hosts.
- Initial updater tests: 19 passed; the final helper recovery diagnostic test was
  added afterward and is included in the final frozen gate, recorded below.

## Initial frozen gate and native evidence

- Initial frozen `python3 scripts/check.py`: 908 ordinary tests, 7 doctests and 6 Python
  script regressions passed; 8 externally configured OpenSSH tests remained ignored.
  Workspace formatting, strict Clippy and direct-dependency policy passed.
  `workspace-final-gate.log` covers the initial frozen implementation; an earlier successful
  `workspace-gate.log` is intermediate evidence from before the final helper test.
- Initial frozen updater module has 20 tests. They include both download/helper validators,
  missing/changed/non-file/symlink MCP, executable modes, isolated successful dual
  image replacement, late corruption restoring both old images or removing a new
  MCP image, failed restore blocking retry, and helper finalization retaining the
  old image plus bilingual diagnostic while refusing restart.
- Initial frozen packaging regression set: 57 passed in `packaging-final.log`. No source
  or workflow was changed after this successful packaging run.
- `fixed-recovery-probe.rs` extracts the exact frozen rollback/helper-finalization
  functions and replays the original owned-directory failure. It returns 0 and
  confirms `RecoveryRequired`, restart refused, old image and diagnostic retained.
  The first standalone harness compilation missed a harness-only `std::io` import;
  that import was corrected without changing production source.
- `MACOSX_DEPLOYMENT_TARGET=15.0 cargo build -p keelshell-app -p keelshell-mcp --locked`
  passed in one native aarch64 macOS dev-profile build. `native-build.log` retains
  the upstream `block 0.1.6` future-compatibility warning, with no strict-Clippy error.
- `package.py` staged both real build outputs; `inspect_native.py` passed plist,
  both dependency sets, and both Mach-O deployment minimums at 15.0. `release.py`
  archived and streamed validation of the six-file ZIP (including its manifest).
  These are debug-profile development artifacts, not a newly published release.
- The staged MCP foundation process completed a real stdio initialize, seven-tool
  discovery, `DISABLED` scope rejection and EOF exit: code 0, no stderr. It used
  the foundation backend at this base commit, with no desktop/SSH access. The
  initial no-input probe incorrectly expected exit 0 before initialization; it
  exited 1 with a bounded initialization diagnostic. Its receipt remains as
  `native-mcp-eof-initial.json`; the corrected protocol probe and bounded cleanup
  are saved in `native-mcp-probe.py/.json/.log`.

Exact dev-build hashes are recorded in `native-package-receipt.json`:

| Artifact | SHA-256 |
| --- | --- |
| Application | `a6b8c9f236666e6c90ba1a53d3d15d9aec5808b109c9c401d53bb2eb3194c158` |
| MCP companion | `34b678da6368b30ef2aa214edee4eb83a26ee9f4614774b6fbbaeb0af7e20ff9` |
| aarch64 macOS ZIP, 38,981,939 bytes | `649458967fdbe4676b1a46cc73e1dcab0a2aaf2b1731b0b082aade07d7a11be5` |

The local source was not committed when this development artifact was staged, so
its optional build-commit field is null. Base plus frozen per-file SHA-256 records
the exact implementation. Release CI supplies the real tag commit as provenance.

The original author's new-agent request was rejected with `agent thread limit
reached`. A fresh existing reviewer subsequently checked the exact 12-file staged
snapshot and found one P1: a second helper entry with an absent executable returned
ordinary `Install`, and finalization deleted its unique old backup. Invalid or
missing manifests had the same ordinary-error cleanup boundary. The independent
report and failing exact-source probe remain in
`work/mcp-companion-independent-review/`.

## Repeated recovery repair

The repair preserves the original staged tree
`f905d306413d306d012060751317281ea42cd8ca`, its patch and all 12 original source
files under ignored `work/mcp-companion-repeat-recovery-fix/`. The original
review-ready receipt SHA-256 is
`b3270e5e23c9432499d981eaeb83230d82c636f0f5e16b66567ce90459eac2fc`; the original
patch SHA-256 is
`5e83689d682b64281c1e4850e7c59126ee04ed26c7b42d132c236a22f36b2c7d`.
`original-preservation.json` records and verifies the copied per-file hashes.

Before changing production behavior, three new tests called the actual
`apply_update` and `finish_update_staging` on owned temporary staging plus a
separate isolated installation directory. `entry-baseline-failure.log` retains
all three expected failures: absent executable returned `Install`, invalid or
missing manifest returned `Invalid`, and each allowed restart and deleted the
old backup. The independent earlier exact-source failure is also retained as
`independent-repeat-recovery.log`; neither evidence set was overwritten.

The repaired entry checks both recovery objects before ordinary validation.
Only `NotFound` counts as absence. Backup creation uses one atomic directory
claim; failure leaves ownership ambiguous and returns `RecoveryRequired`.
Finalization separately preserves unclassified recovery state even if given an
ordinary error. Successful installation instead returns a payload-bound,
non-cloneable `CommittedUpdate` proof that finalization consumes, so successful
backups are still cleaned normally. Existing recovery instructions and marker
links are preserved without modifying an external target.

The final regression invokes both real functions twice on the same payload
before and after changing each preflight condition. The updater suite now has
29 tests, adding repeated missing-executable/invalid-manifest/missing-manifest
cases, ordinary-error finalization, a mismatched committed payload, deterministic
competing directory claims, an existing marker, a Unix metadata loop and a marker
link. The successful dual-image test also consumes the committed result, verifies
staging removal, and reads back both installed images outside staging. Unix-only
metadata/link probes do not establish Windows behavior.

The repaired `python3 scripts/check.py` passed formatting, strict workspace
Clippy for all targets, dependency policy, 917 ordinary tests, 7 doctests and
6 Python script regressions. The 8 externally configured OpenSSH tests remain
ignored. `whole-gate-after.log` and `gate-summary-after.json` record those results.
The unchanged packaging set passed all 57 tests in `packaging-after.log`. An
earlier successful 29-test updater-only run is retained in `updater-after.log`;
the complete gate additionally includes the final regression that calls both
entry and finalization twice for each repeated payload.

A macOS 15.0 deployment-target dev build of both binary crates passed in
`native-build-after.log`. Its upstream `block 0.1.6` future-compatibility warning
is preserved; strict Clippy still passed. This build uses the repaired updater
source and remains a development artifact without native GUI/update acceptance.
`native-package-after.log` confirms both fresh build outputs were staged beside
each other, both Mach-O architectures/dependency sets/deployment minima were
inspected, and the six-file ZIP passed streaming archive validation. The packaged
foundation MCP process completed initialize, seven-tool discovery, `DISABLED`
scope rejection and EOF exit with code 0 and no stderr. It uses the disconnected
foundation backend at this branch's base, without desktop/SSH access.

`native-package-receipt-after.json` binds the following development artifacts to
updater SHA-256
`d900d3f78673d32b94324a55706706ef4d48fe097a9226b71b8125865b538923`:

| Rebuilt artifact | SHA-256 |
| --- | --- |
| Application | `5203d621313a2c3b912738d3de67e781ec2b8a9547d60db172f65f148863077f` |
| MCP companion | `34b678da6368b30ef2aa214edee4eb83a26ee9f4614774b6fbbaeb0af7e20ff9` |
| aarch64 macOS ZIP, 38,984,977 bytes | `fca163379018e6992036f7b602e97950efeaf2db90c18cdb141554d4634eb5ea` |

The repaired 12-file staged snapshot is frozen in
`review-ready-source-p1-fix.json`, with its separate patch and evidence hashes in
`work/mcp-companion-repeat-recovery-fix/`. The initial receipt/patch/failure logs
remain independently identifiable.

Independent re-review of staged tree
`eeabf528f261d6486f796c0c2904d7d87b9adb2a` found the original P1 resolved and no
new P1/P2. The reviewer independently passed 57 packaging and 29 updater tests,
then re-ran all three real two-entry regressions with visible results:
`RecoveryRequired`, restart refused and the old bytes retained. Its original
exact-source probe still failed as expected against the preserved original
snapshot; the repaired-function adapter passed and retained the old image and
original recovery instructions. The report and checksummed private receipts are
in `work/mcp-companion-repeat-recovery-review/`.

All 12 source/config/document hashes matched before and after review, with no
unstaged diff. Only these two review-status documents changed afterward; code,
workflow, packaging and build hashes remain identical to the approved snapshot.
`commit-ready-source.json` binds that documentation completion to the final staged
tree and the independent report. No push, tag or merge was made by this task.

## Unverified boundaries

No application was installed or overwritten, no tag or release was created, and
no GUI was launched by this packaging task. Real installed-directory update/restart,
Windows locked companion replacement, Windows/Linux native process/runtime behavior,
signing/notarization and final six-runner CI remain unverified. macOS package
inspection establishes binary/dependency structure only. Running MCP stdio
processes require external-agent restart; GUI restart alone does not load their new
image. Cross-volume backup rename can be refused and remains outside acceptance.
