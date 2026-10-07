# Text merge and strict patch: limited macOS native evidence — 2026-10-07

The root exercised the development macOS application built for
`b54b83089bbf2166f1a8351a5535e220f4946163` against an owned loopback
SSH/SFTP fixture. Chinese and System appearance, observed as dark, were used
in a maximized 1440×933 logical window. No model, external MCP client,
customer machine, installation or update was involved.

All 746 source inputs, 15,200,672 bytes, matched before and after. The actual
application, MCP companion, fixture and four package resources were read and
hashed: seven artifacts, 229,062,439 bytes. Compared with the earlier build
map, only nine result Markdown files changed; executable and test sources
were identical. This binding belongs to b54; it does not validate later
Agent/MCP integration source or its artifacts.

## Operations and complete independent readback

The initial regular file contained `alpha\nbase-middle\nbase-tail\n`
(28 bytes). The desktop draft changed the middle line. The root independently
changed the remote tail to `remote-tail`, producing 30 bytes. The application
read this real SFTP conflict and presented a zero-overlap three-way result.
Screenshot 07 and AX both showed all three resulting lines simultaneously.

Adopting the result updated only the draft. Independent full remote reads
before and after adoption still returned the same 30 bytes, SHA-256
`953d877de111cec38ac08cebf596f282443443c23f20f63c23fb12bd648c8d81`.
After separate review and the visible confirmation action, the application
reported atomic save and complete readback. The root independently read all
31 remote bytes, exactly `alpha\ndraft-middle\nremote-tail\n`, SHA-256
`b9c0687e35a11b2f9f14018b4c6c00c7a249c19a9ee5b79280477f0048c8087e`.

A strict single-file patch then changed only `remote-tail` to `patch-tail`.
Parsing and applying it left the remote file at the preceding 31 bytes and
hash. After another separate review and confirmation, the complete remote
read was exactly `alpha\ndraft-middle\npatch-tail\n` (30 bytes), SHA-256
`f60292b0440fffb5b16dfe9513eb7aa1b18639fac007136f3fef2477abaeb749`.
The reviewed target was an existing regular file with mode 0600. These
results prove the two fixture draft/save sequences, not arbitrary conflict
or patch behavior.

## Independent evidence review and UI limits

A non-author actually read all 16 AX records and viewed all 16 original
pixel screenshots, compared the complete raw remote bytes and source maps,
and read/hashed the actual seven artifacts. It found no new data-integrity
blocker in these two scenarios. Its 63-payload, 25,118,701-byte review was
fully consumed by the root. Manifest SHA-256:
`f5f14d5b033735b04246ab6437fdc178ed31de69ea7928a27649cf9e051b7448`;
seal: `924f5a87e3c828694ec9f02eb38789d9b0abf918fabb1e6b4c262827fbfb6cbf`;
result: `51290b780c046556dc21f07fd5a665dbe6b986242907d4989faf6ce38cfa8c67`.
This was independent evidence review, not independent native re-execution.

The save review viewport was about 112 physical pixels high. AX contained
the full three-line value, while the save screenshots showed scrolled
segments. Screenshot 14's name mentions the middle, but its pixels show the
tail. This does not establish simultaneous full-save visibility or
VoiceOver usability. An initial offscreen keyboard edit focused the path
field; the attempt and subsequent restoration are preserved, rather than
classified as a proven production defect. The successful draft and patch
were entered through real AX setValue; the patch textarea was offscreen.
That is native input evidence, not proof of keyboard-only reachability.

Improving review height, keyboard navigation and patch focus remains work.
Overlapping conflicts, long horizontal content, 900×580, English, explicit
light/dark combinations, VoiceOver and Windows/Linux native checks remain
open. The later combined source needs its own checks and native evidence.

## Ownership and preserved evidence

The owned 1,500-second run ended by an explicit stop at 1,305.402 seconds,
without extending its deadline. The runner actually waited for GUI PID/PGID
76802 (return -15) and fixture PID/PGID 76794 (return 0), and reaped both.
The root tool session 80150 subsequently returned actual exit 0; that tool
receipt is distinct from the runner's child OS wait. Both known groups were
absent, port 63040 refused connections and private scratch/fixture data were
removed. The independent reviewer rechecked those final resource states.

Original failures, raw bytes, screenshot/AX captures, before/after source
maps and cleanup receipts remain in ignored `work/text-merge-native-20261007-v1/`.
The root-consumed review is in
`work/text-merge-native-root-consumption-20261007-v1/`.
See [the feature guide](../../product/TEXT_CONFLICT_MERGE.md) and
[the earlier engineering integration](2026-10-07-reviewed-text-main-integration.md).
