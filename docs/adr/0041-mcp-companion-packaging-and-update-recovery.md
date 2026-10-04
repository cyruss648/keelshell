# ADR 0041 — MCP companion delivery and recoverable updates

Status: implemented locally and independently reviewed; integrated release/native acceptance remain pending.

## Context

The desktop MCP configuration resolves `keelshell-mcp` (`.exe` on Windows) beside
the current application executable. A release containing only the application
cannot provide that configured entry point. The updater previously required only
the application receipt, accepted a manifest without the companion, and discarded
rollback failures before deleting the staging directory containing old backups.

## Decision

Every platform package requires explicitly supplied application and MCP binaries
from the same build. The six-target Release matrix builds both packages together.
The fixed companion paths are `KeelShell.app/Contents/MacOS/keelshell-mcp`,
`usr/bin/keelshell-mcp`, and `keelshell-mcp.exe`, each beside its application.

Keep `package-manifest.json` schema 1 and add required `mcp_binary_sha256`, bound
to the fixed path in `files`. Existing helpers ignore the added field and install
every listed file, permitting old applications to acquire the companion on their
next complete update. The historical helper implementation is not retroactively
changed. New download validators and helpers explicitly reject a missing companion
receipt with a bilingual incomplete-package error. Unsupported or corrupted
receipts remain invalid; complete receipts still undergo version/target checks.

Stage and archive validation require both binaries' format, target architecture,
SHA-256 and Unix execute bits. Native inspection checks both runtime dependency
sets and macOS deployment minimums. Windows icon and DPI resources are required
only on the GUI executable. Staging, synthetic binary headers, compilation and
native dependency inspection do not establish GUI or external-agent acceptance.

The updater includes both images in its manifest-bound plan and rechecks source
type, symlink ancestors, digest and executable mode before each replacement. It
preserves Unix modes when copying; a mode-copy failure is an installation failure.
Ordinary late failures remove newly installed files and restore every moved old
file. Old installations lacking MCP remove the newly added companion on rollback.
Unlisted installation files are preserved.

Rollback failures return `RecoveryRequired`. The entry point checks `.backup` and
`recovery-required.txt` before executable or manifest validation, since a failed
restore may have left the current executable absent. Only a metadata `NotFound`
establishes absence; all other metadata errors, existing recovery objects and any
failed atomic backup-directory claim stop the attempt conservatively. A retry
never deletes an unclassified backup to start again.

Finalization independently checks recovery state on every failure. Successful
installation returns a non-cloneable `CommittedUpdate` proof bound to the exact
payload; consuming that proof permits cleanup of its ordinary success backups.
An ordinary preflight error cannot classify old backups as disposable. Only
fully restored failures or a committed update permit normal cleanup/restart.
The detached helper retains unresolved staging, adds a bilingual diagnostic only
when `recovery-required.txt` does not already exist, reports the local path on
stderr, and stops restart. It does not overwrite an existing marker or follow a
marker symlink. The diagnostic contains no SSH configuration, credentials, remote
output or logs.

Independent review of the initial frozen implementation found a second-entry
failure that bypassed the backup guard and removed the unique old image. The
original source and failed probes remain preserved. Regressions now run the real
`apply_update` and finalization twice against one owned payload, including an
absent executable and invalid/missing manifest, as well as ambiguous metadata,
competing backup claims and normal committed cleanup.

## Consequences and boundaries

Running external-agent stdio processes retain their already loaded image on Unix;
the external agent must restart them to use the updated companion. A running Windows
image may lock replacement; the existing 30-second/200-ms retry budget remains and
retries only after successful rollback. GUI restart does not restart external agents.

The existing backup directory is inside download staging. Moving old images from
another filesystem volume may be refused; this layout is not claimed to support
cross-volume installation. Restoration is best effort with an explicit retained
recovery state, not an atomic transaction against arbitrary external filesystem
changes or helper termination. No actual existing application installation was
overwritten during this work.

The requirement covered here is the delivery portion of MCP-04. Desktop scopes,
real SSH tools, native approval, external Claude Code/Codex interoperability and
Windows/Linux desktop acceptance remain governed by their own evidence. See the
[companion test record](../testing/records/2026-10-04-mcp-companion-packaging.md).
