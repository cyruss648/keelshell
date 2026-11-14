# Profile sync atomic publish preflight — 2026-11-14

This slice tightens the existing reviewed profile synchronization publish path.
The shared payload is still written to a temporary file in the selected
directory, flushed with `sync_all`, atomically renamed into place, and followed
by Unix parent-directory synchronization. Before creating a staging file, the
publisher now requires the destination to be absent or a regular file. A
directory, symlink, or other non-regular entry is rejected with the existing
`Invalid` error and is left untouched.

The focused core tests cover replacement of an existing encrypted payload,
cleanup of the same-directory staging file after success, rejection of a
non-regular destination without a staging-file leak, and reviewed apply
rejection without a local StateStore write when the shared payload path becomes
a directory after review.

Validation for this slice is limited to the local filesystem and the existing
cooperating lock protocol. It does not prove power-loss recovery, atomicity or
directory durability semantics supplied by an arbitrary network filesystem, or
coordination with writers that bypass the lock file. Windows and Linux native
desktop behavior, real multi-machine mounts, and release acceptance remain
outside this focused core record.
