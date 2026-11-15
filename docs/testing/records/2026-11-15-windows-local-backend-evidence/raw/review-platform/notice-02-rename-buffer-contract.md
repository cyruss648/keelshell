# P2 — rename passes a length below the documented native buffer minimum

Status: OPEN; source-contract finding. Windows native execution: NOT_RUN.

Input remains the unchanged r1 platform freeze:
`c10ab180854098fbe636f7c57805b7e10ad4c9498043918f06ea655d4bb34954`.

At `crates/keelshell-platform/src/windows/native.rs:320-324`, `length` is
`offset_of!(FILE_RENAME_INFORMATION, FileName) + encoded_name_bytes`.
The same value is passed as `NtSetInformationFile.Length` at lines 344-349.
The allocation is rounded up to machine words, but the supplied API length is
not enlarged with it.

Microsoft's `FILE_RENAME_INFORMATION` documentation states that the information
buffer must be at least `sizeof(FILE_RENAME_INFORMATION)` plus the filename's
byte count. The current calculation is smaller than that stated minimum.
In particular, for the Windows x64 layout, the filename offset is 20 and the
structure size is 24. A valid one-UTF-16-unit filename such as `"x"` therefore
passes 22 bytes, below even the fixed structure's 24-byte extent.

Trigger: publish a retained private file or directory under a valid one-unit
name; there is no destination-name lower bound that excludes this operation.
Impact: successful publication is not covered by the native API's documented
buffer contract. This is an API correctness/short-name coverage issue, not a
claim of out-of-bounds access: the underlying rounded allocation is larger.
No actual NTSTATUS or Windows failure is claimed without native execution.

Primary source is preserved in
`sources/file-rename-information.md`, final paragraph, with URL and hash in
`sources/fetch-r1.json`. Existing successful publication tests use longer
names such as `"published"` and do not cover a one-unit destination name
(`crates/keelshell-platform/src/windows/tests.rs:80-95,124-150`).

Use a checked buffer extent that satisfies the documented native minimum and
pass the actual initialized buffer length. Add one-unit and maximum-length
native file/directory rename regressions. Source remains strictly unchanged
by this reviewer.
