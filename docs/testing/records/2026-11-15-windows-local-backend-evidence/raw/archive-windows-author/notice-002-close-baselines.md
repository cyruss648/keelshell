# Windows archive timestamp baseline — 2026-11-15

Status: SOURCE_ADJUSTED / NOT_COMPILED / WINDOWS_NATIVE_NOT_RUN.

Source review identified a Windows-specific false-change risk: the first draft
captured file timestamps before closing its writing handle.

Microsoft's primary Win32 file-time documentation states that last-write time
is not fully updated until all writing handles are closed. Reference:
`https://learn.microsoft.com/en-us/windows/win32/sysinfo/file-times`.

The revised source opens an independent READ observation handle before closing
each file creation handle, then observes its final metadata and reads back bytes.
After populating the tree it conservatively replaces every child directory
creation capability with an independent READ observation capability, then
captures the full tree. Creation identity, links, attributes, creation time, and
security are compared across that boundary. Private directory creation already
uses GENERIC_READ + SHARE_ALL with no WRITE or DELETE access, so this directory
step does not close writing handles. Only the staging root's retained
PrivateEntry keeps its original directory capability through publication.
Directory rename_no_replace acquires and verifies its short-lived DELETE source
lease through ReOpenFile inside the platform API; the retained capability is not
replaced. Ordinary private files retain RW | DELETE + shareREAD creation handles,
so their writer-close timestamp boundary remains necessary.

This is an author source correction, not evidence of a reproduced native failure
or a passing Windows run. The nested private-tree publication test exercises the
resulting ordering when the parent runs the Windows inventory.
