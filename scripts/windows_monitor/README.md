# Windows monitor lab

`python scripts/windows_monitor_lab.py --mode direct --output <new-directory>`
runs the actual production PowerShell collector files on Windows. It never
compiles Rust or runs the application. Production assembly follows the exact
literal prefix + `common.ps1` + body + literal suffix rules in production
`ASSEMBLY.md`, without inserted separators, trimming or rewriting. The encoded
wire command, file hashes and command lengths are preserved. The fixed
skip-capacity resources variant is also run and saved separately.
All production captures go through actual `cmd.exe /d /s /c` with the complete
encoded command as one argument. The receipt retains shell, argv, serialized
length, original cmd exit and original PowerShell process handles observed
while live. A complete admitted-process census and the original PowerShell
object's natural zero exit must be established before cleanup. Missing a
short-lived child fails the gate; pipe EOF never supplies this proof.

The fixture owns one process, IPv4 and IPv6 TCP LISTEN sockets, UDP bindings,
and connected TCP client endpoints that must not appear as listeners. The lab
uses the existing suspended-create/Job/retained-handle lifecycle implementation
in `windows_native_crash.py`; pipes alone cannot establish terminal state.
All raw frames and command/cleanup receipts are retained, including on failure.
Production raw byte filenames are `probe.stdout`, `resources.stdout`,
`processes.stdout` and `sockets.stdout`. The workflow subsequently runs the
public authenticated monitor routing capture replay against that directory.
This is a replay of actual API frames, not an SSH acceptance claim. Missing,
failed or held API evidence produces an explicit nonzero replay HOLD.
The independent Python Win32 oracle uses IP Helper, process, memory, uptime,
disk and interface APIs. Socket and process comparisons concern the owned
fixtures. Resource comparisons describe their tolerances; CPU samples have
different observation windows, so the independent CPU difference is recorded
without asserting numerical equality.

Resource coverage uses independently collected before/after inventories.
Win32 `GetLogicalDriveStringsW` and `GetDriveTypeW` select local fixed roots;
readiness, volume GUID/serial and capacity are read independently of production
mounts. `GetAdaptersAddresses` bounds the IP-enabled adapter set, and
`GetIfTable2` supplies LUID/GUID/index, alias and counters. Production must
include every stable identity, including when it reports an empty array.
Interface/volume changes, counter resets, ambiguous identities or incomplete
oracle readings remain HOLD; a definite stable omission fails. The fixed
capacity-skipped variant exempts capacity alone and still requires full stable
network coverage. Legal explicit empty frames remain valid parser inputs.
These added Win32 declarations still require actual Windows ABI/runtime checks;
offline buffer tests do not supply that acceptance.

`--mode ssh` performs only isolated OpenSSH prerequisite/configuration checks:
private temporary directory, temporary Ed25519 host/client keys, loopback high
port and an explicit authorized-keys file. It never starts or changes an
installed service, registry entry or firewall rule, and never reads user keys.
Even a successful `sshd -t` is **HOLD**, exit 78. An authenticated session and
original remote PowerShell process termination have not been established.
Enabling actual SSH acceptance requires a reviewed admission/identity protocol
covering Windows OpenSSH token/broker descendants, not merely channel closure.
Private key material stays outside the uploaded output directory and is
removed before the final receipt is written.

`windows-monitor.yml` has separate direct-API and isolated-SSH jobs on
`windows-2025`. Both use the existing setup action. The SSH job deliberately
reports HOLD as a nonzero result. Both always upload their own evidence.
Docs-only pushes do not trigger this workflow; the existing Quality workflow
is unchanged.

Offline unit tests use synthetic frames and real owned Python children on the
host running the tests. They do not emulate or sign Windows API, OpenSSH,
native desktop, Rust or release acceptance.
