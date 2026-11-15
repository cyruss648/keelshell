# P2 — retained private target directory conflicts with the rename target open

Status: OPEN; source-contract finding. Windows native reproduction: NOT_RUN.

Frozen input: `platform-inputs-r1.json`, SHA-256
`c10ab180854098fbe636f7c57805b7e10ad4c9498043918f06ea655d4bb34954`;
all 9 platform inputs matched before independent validation.

The supported safe composition

```rust
let destination = parent.create_directory("private-target")?;
let target = destination.as_directory()?;
let source = parent.create_file("stage")?;
target.rename_no_replace(&source, "published")?;
```

retains the target directory's creation handle. That handle was opened with
`FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE`, but only `FILE_SHARE_READ`
(`crates/keelshell-platform/src/windows/native.rs:131-156`).
`PrivateEntry::as_directory()` duplicates the same file object rather than
opening a differently shared one
(`crates/keelshell-platform/src/windows/mod.rs:262-267`).
The rename then passes that directory as `RootDirectory`
(`crates/keelshell-platform/src/windows/native.rs:318-349`).

Microsoft's `FILE_RENAME_INFORMATION.RootDirectory` contract explicitly says
that `IopOpenLinkOrRenameTarget` opens the target directory, requesting
`FILE_WRITE_DATA | SYNCHRONIZE`. The retained creation handle does not share
write access. Under the documented access/share compatibility rules, this
composition cannot rely on a successful target open even when the destination
name is absent and both objects are otherwise valid. This is a functional
publication boundary issue, not evidence of replacement or an unsafe memory
access.

Primary documentation saved in this review directory:

- `sources/file-rename-information.md:90-96`, target-directory relative open.
- `sources/zwcreatefile.md:120-126,206-208`, access/share compatibility.
- `sources/fetch-r1.json`, source URLs and exact retrieved byte hashes.

The existing positive publication tests use the ordinary parent opened with
`SHARE_ALL`; `complete_private_tree_is_published_through_its_retained_root`
also publishes to that ordinary parent. No positive test exercises publication
*into* a still-retained private directory
(`crates/keelshell-platform/src/windows/tests.rs:80-95,124-150`).

Please resolve the target-directory access/share design and add a native
regression for the exact retained-private-target composition. Preserve the
intended refusal of concurrent write/delete openers on private source files;
blanket widening of all private sharing would change that contract. If this
composition is intentionally unsupported, make that restriction explicit and
enforced at the safe API boundary and align the acceptance contract.

The reviewer has not changed source, formatting, Git, parent target, or main
documentation. This notice is ready for the parallel integrating parent to
read; no cross-chat delivery is claimed.
