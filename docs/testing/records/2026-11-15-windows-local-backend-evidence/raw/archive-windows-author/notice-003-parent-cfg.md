# Parent integration cfg notice — 2026-11-15

Status: PARENT_INTEGRATION_REQUIRED / NOT_COMPILER_VERIFIED.

Switching `mod local` to the new Windows module removes the Windows call sites
of these existing, parent-owned items in `sftp/archive_transfer.rs`:

- `use std::sync::atomic::AtomicBool;`
- `Authority::io`
- `Authority::local_mutation`

The current source has no corresponding uses in `remote.rs` or
`local_windows.rs`. The new module imports its own atomics and queues synchronous
work explicitly so it can mark pending before enqueue and retain worker tickets.
It cannot safely reuse the existing local_mutation wrapper for that purpose.

When adding the Windows module path, the parent should cfg-gate these existing
items to the targets that still compile the original local module, for example
`#[cfg(not(windows))]`. Otherwise a Windows build with warnings denied may reject
the now-unused import or methods. This is a source-derived integration issue;
the author has not run a compiler or claimed a reproduced lint failure.

These items, the module wiring, Cargo manifests, UI, and integration-test cfg
are outside the author write set and were not edited. The new module's required
public-to-parent interface is unchanged.
