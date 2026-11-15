# Windows archive author contract — 2026-11-15

Status: SOURCE_IMPLEMENTED / PARENT_INTEGRATION_REQUIRED / WINDOWS_NATIVE_NOT_RUN.

The author owns only `local_windows.rs`, `local_windows/tests.rs`, and this
evidence directory. No Cargo, formatter, compiler, Clippy, Git, agent, chat,
worktree, UI, integration-test cfg, or main documentation changes are authorized.

The platform API is consumed through `Directory::open_chain`,
`Directory::open_directory`, `Directory::open_file`, and
`PrivateEntry::as_directory`. `Directory::from_file` is private. Platform metadata
owns native local-volume, identity, link, reparse-point, and security checks.

The implementation must retain every ancestor capability, compare manifest
observations instead of handles, and recheck exact directory children as well as
all file contents. Private directory creation handles use GENERIC_READ +
SHARE_ALL without WRITE or DELETE access. The backend conservatively replaces
child creation capabilities with independent READ observations before the full
snapshot; the retained root PrivateEntry keeps its original capability.
Directory rename_no_replace obtains and verifies a short-lived DELETE source
lease through ReOpenFile inside the platform API without replacing that retained
capability. Private ordinary files retain RW | DELETE + shareREAD creation
handles, which close before the final file timestamp baseline is captured.

Staging and publication are separate blocking jobs with remote revalidation
between them. Mutation pending is set before enqueue, worker tickets survive
abandoned awaits, and an armed UnverifiedStage survives every incomplete tree
or unobserved publication. Failed staging is never automatically deleted.

No blocking integration issue has been established at this checkpoint. Source
delivery, parent integration checks, and actual Windows native execution remain
separate states.
