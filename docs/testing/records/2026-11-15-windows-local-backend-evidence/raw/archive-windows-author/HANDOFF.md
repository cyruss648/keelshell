# Windows archive local backend — author delivery, 2026-11-15

Status: **SOURCE_IMPLEMENTED / PARENT_INTEGRATION_REQUIRED / WINDOWS_NATIVE_NOT_RUN**.

The author read AGENTS, HANDOFF, STRATEGY, SHARED_TARGET, and
rust-best-practices. Only the two assigned Rust files and this evidence directory
were written. No Cargo, fmt, rustc, Clippy, Git, agent, chat, or worktree operation
was performed. Main docs, existing local.rs, archive_transfer.rs, Cargo manifests,
UI, platform sources, and integration-test cfg remain parent-owned.

## Source identity

| Owned source | SHA-256 |
| --- | --- |
| `crates/keelshell-session/src/sftp/archive_transfer/local_windows.rs` | `7fbe527fcc9d110a6c5bd36bdbd319b97874ae1358867e0ef6e3900f0f62de41` |
| `crates/keelshell-session/src/sftp/archive_transfer/local_windows/tests.rs` | `e49d2ca86c6ca9c36962eb0c0b92f627f35301857aeec940b82947620a1712a3` |

These are working-file hashes. No Git revision was inspected or created.
`source-inspection.json` records the complete owned-source and read-only API
input snapshot, all 29 exact test names, source locations, and NOT_RUN states.
The inspection command actually exited 0 with 14 text checks; it executed zero
Rust tests and performed no syntax, compilation, formatting, or native check.

## Parent interface

The new module provides the existing `mod local` interface:

| Item | Contract |
| --- | --- |
| `Manifest` | `Clone + Debug + Default + PartialEq + Eq`; equality compares observations, not handles |
| `destination(&Path, &Authority) -> Result<Manifest>` | async, absent target and all retained ancestors |
| `scan(&[PathBuf], &Authority) -> Result<(Manifest, Vec<ArchiveMember>)>` | async, full contents and namespace rechecks |
| `extraction_supported() -> Result<()>` | Windows backend present; actual unsupported volumes fail closed during native observation |
| `staging_path(&Path) -> Result<PathBuf>` | uncreated sibling pathname; pure validation |
| `validate_destinations(&Path, &[ArchiveMember]) -> Result<()>` | pure, explicit and implicit Windows targets |
| `extract(&SftpSession, &ArchiveTransferPlan, &RawSftpSession, &Path, &Path, &FileMutationScope, &Authority) -> Result<()>` | async, stage / remote revalidate / publish and verify |

The parent supplies the announced Windows `cfg_attr` module path and Windows
keelshell-platform dependency. Also handle **notice-003-parent-cfg.md**: the old
parent-level AtomicBool import and Authority::io/local_mutation lose their
Windows consumers and may need `cfg(not(windows))` for warnings-denied gates.
The author did not edit these out-of-scope declarations.

The platform API is consumed through open_chain/open_directory/open_file,
metadata/verify_private, and retained PrivateEntry creation/publication.
Directory::from_file and native ABI calls are not used. This source relies on
the parent's strengthened held-volume checks and security-safe Debug output.

## Observations and mutation lifecycle

Manifest clones retain every ancestor Directory and every observed source
directory/file. Ancestors compare identity, kind, link count, attributes,
creation time, and full security observations while excluding only timestamps
that sibling writes change. Source entry stamps additionally retain modification
and change times. Directory children are sorted and exactly re-enumerated after
all file reads; every file is fully read, digested, and read again before a scan
returns. Caller review/execution still compare both Manifest and all member bytes.

Source count, explicit/implied entry count, expanded bytes, manifest bytes, full
native/member paths, UTF-16 components, and directory depth are bounded. Names
use the platform component validator, conservative Unicode case aliases, full
filesystem identities, and native hard-link/reparse checks. Slash member strings
are split and individually pushed into a Windows PathBuf.

All synchronous filesystem work is submitted with spawn_blocking. Await drop
sets an AtomicBool through a cancellation guard. Each mutation job owns a clone
of scope.ticket(); pending is set before queue submission and cleared only on
normal synchronous completion. A panic leaves pending true. Partial staging and
unobserved completed output independently retain an armed UnverifiedStage ticket.
Late known completion cannot undo an already-retired quarantine.

Extraction creates private objects with the platform creation-time protected
owner-only DACL. Ordinary private files use RW | DELETE + shareREAD creation
handles. Independent READ observations pin file identities while writing handles
close and final metadata is observed. Private directory creation uses
GENERIC_READ + SHARE_ALL without WRITE or DELETE access. The backend
conservatively replaces child directory creation capabilities with independent
READ observations before the full staged-tree snapshot. The root PrivateEntry
retains its original directory capability. Directory rename_no_replace acquires
and verifies a short-lived DELETE source lease through ReOpenFile inside the
platform API, without replacing the retained capability.

The first worker builds and reads back the complete private tree. Then extract
awaits sftp.revalidate_archive. The second worker reopens and checks all original
ancestors, the staging root identity, private DACLs, exact inventory, and all
contents immediately before publishing through the retained PrivateEntry with
no replacement. Post-publication checks cover full contents, DACLs, stable
identities, exact unique destination spelling, original parents, and staging
absence. The async owner disarms UnverifiedStage only after receiving the
verified result. Failure never automatically removes or retries staging.

The close-time timestamp source correction is described in
notice-002-close-baselines.md. It is not a claimed reproduced native failure.
No directory crash durability or exclusion of independent external writers
after readback is claimed.

## Test inventory

All names below have the prefix `sftp::archive_transfer::local::tests::`.
There are **29 authored tests, 28 default and 1 privileged opt-in; all NOT_RUN**.
The reparse-point test is ignored by default because it requires Windows
Developer Mode or symbolic-link privilege.

```text
windows_archive_slash_members_join_literal_windows_components
windows_archive_rejects_device_stream_traversal_and_direction_marker_components
windows_archive_rejects_unc_device_namespace_and_non_normalized_drive_paths
windows_archive_selected_extended_drive_and_unicode_case_aliases_overlap
windows_archive_implicit_directory_aliases_and_file_parents_are_rejected
windows_archive_implicit_directory_inventory_has_its_own_entry_bound
windows_archive_destinations_enforce_combined_absolute_path_bound
windows_archive_member_depth_is_bounded_before_any_native_io
windows_archive_cancel_guard_marks_its_worker_on_drop
windows_archive_fresh_scan_manifest_equality_ignores_handle_values
windows_archive_manifest_clone_retains_every_original_ancestor_capability
windows_archive_scan_rechecks_children_even_when_directory_stamps_match
windows_archive_scan_rechecks_digest_and_bytes_even_when_file_stamps_match
windows_archive_scan_rejects_hard_linked_sources
windows_archive_scan_rejects_source_and_ancestor_reparse_points
windows_archive_destination_observations_ignore_sibling_write_timestamps
windows_archive_destination_refuses_existing_unicode_case_alias
windows_archive_private_nested_tree_publishes_after_child_creation_handles_close
windows_archive_empty_private_tree_publishes_with_exact_root_identity
windows_archive_publication_preserves_existing_file_and_quarantines_stage
windows_archive_publication_preserves_existing_empty_directory_and_quarantines_stage
windows_archive_changed_staging_content_is_not_published_or_deleted
windows_archive_changed_staging_children_are_not_published_or_deleted
windows_archive_cancel_before_publication_preserves_complete_staging
windows_archive_unobserved_successful_publication_remains_quarantined
windows_archive_abandoned_stage_result_keeps_isolation_after_pending_clears
windows_archive_dropping_stage_await_quarantines_its_detached_worker_result
windows_archive_panicking_worker_keeps_pending_and_partial_stage_isolation
windows_archive_queued_cancel_retains_ticket_until_worker_confirms_stop
```

The directory-list and content-digest regressions deliberately equalize the
observed stamps after a real filesystem change, so timestamps alone cannot make
those regressions pass. The queued-cancel regression consumes the original
reservation before releasing its blocked worker, and confirms a new admission
only after that worker acknowledges cancellation.

## Unverified boundaries and integration ownership

Rust syntax/type checking, compiled test inventory, all 29 test results, formatter,
Clippy, dependency policy, cross compilation, native Windows filesystem behavior,
real SSH archive transfers, native desktop behavior, and release acceptance are
**NOT_RUN by this author**. Platform tests, UI/cross-platform integration cfg,
shared-target ownership, final centralized gates, and source commit binding
remain with the parent. There is no claim of independent reviewer approval.

The source-only inspection record is useful for input identity and explicit
delivery scope. It cannot substitute for the parent's compiled inventory or
actual Windows acceptance.

The parent reports a passing Zmodem full-session Windows all-target check after
SDK environment fixes. That separate check does not establish compilation or
native acceptance for this newly delivered Windows archive backend.
