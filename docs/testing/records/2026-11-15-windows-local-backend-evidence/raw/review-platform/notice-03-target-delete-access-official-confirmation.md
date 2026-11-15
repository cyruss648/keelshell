# Supplement to P2 notice-01 — official Windows DELETE/share incompatibility

This supplements the same finding, not a third independent P2.
Frozen source remains r1. Windows native execution remains NOT_RUN.

The integrating parent proposed directory-only `FILE_SHARE_READ |
FILE_SHARE_WRITE`, keeping ordinary private files at read sharing only. The
write-sharing adjustment alone does not resolve the complete compatibility
problem while the target directory's original creation file object retains
DELETE access.

Additional Microsoft primary evidence was found and preserved:

- `sources/ms-fsa-rename.txt:78-93` (MS-FSA 2.1.5.15.12) describes opening the
  destination directory. The main algorithm lists all three sharing bits but
  explicitly references product-behavior note 185.
- `sources/ms-fsa-product-behavior.txt:1387-1392` (MS-FSA Appendix A, note 185)
  specifies different behavior through Windows 11 and Windows Server 2022:
  the target open shares read and write access only; a pre-existing
  DELETE-access handle on that directory causes STATUS_SHARING_VIOLATION.
- URLs and exact retrieved hashes are in `sources/fsa-fetch-r1.json` and
  `sources/fsa-appendix-fetch.json`.

The current target composition retains precisely such a DELETE-access file
object (`native.rs:131-136`, `mod.rs:262-267`). `try_clone()` preserves the
original object and does not create a separately opened target capability
with different access/share semantics.

Therefore, for Windows versions explicitly covered by note 185, the parent's
proposed READ|WRITE sharing with the existing access mask is still
source-contract incompatible. Adding FILE_SHARE_DELETE to the original
creation handle would not fix the reciprocal failure either: the *new
kernel target opener* would still decline to share the original handle's
DELETE access.

The source/target directory capability roles or opening strategy need to
account for this constraint. Keep the private-file writer/delete rejection
contract and retained identity/namespace checks explicit. A repaired design
needs the retained-private-target positive regression plus ordinary private
file read success and writer/delete rejection controls; actual Windows
execution is still a separate acceptance gate.

This is now backed by explicit official product behavior, rather than an
assumption about an undocumented internal ShareAccess value. It remains a
source-contract conclusion; no observed Windows syscall result is claimed.
