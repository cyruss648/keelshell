# Fixed Windows collector assembly

The lab must assemble the checked-in production files verbatim. They are UTF-8
without BOM and use LF. No newline normalization or script minifier is involved.
The operation mapping is fixed:

| Operation | Literal prefix | Body file |
| --- | --- | --- |
| Probe | `$op='probe';$skip=$false;` + LF | `probe.ps1` |
| Resources | `$op='resources';$skip=$false;` + LF | `resources.ps1` |
| Resources, capacity skipped | `$op='resources';$skip=$true;` + LF | `resources.ps1` |
| Processes | `$op='processes';$skip=$false;` + LF | `processes.ps1` |
| Sockets | `$op='sockets';$skip=$false;` + LF | `sockets.ps1` |

1. Concatenate literal prefix, `common.ps1`, operation body file, and the literal
   suffix `}catch{F (E $_) $null 1}` followed by LF. Both files include their final
   LF. Do not insert any additional separators or interpolate remote/user input.
2. Encode that string as UTF-16LE, without a BOM.
3. Encode the resulting bytes with standard padded Base64, without line wrapping.
4. Prepend exactly `powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand `
   (one trailing ASCII space) to obtain the command.
5. Reject if that entire command is not ASCII or is 8000 bytes or longer.

These are command strings for SSH exec; do not add shell quotes, redirections,
execution-policy overrides, files, stdin or an alternate PowerShell executable.
The `common.ps1` deliberately opens the `try` closed by the fixed suffix.

The parent resolved CPU null representation with `CpuSample::Unavailable`.
The collector emits null when every processor sample is unavailable. No
zero/NaN/ticks fallback is allowed; this changes no wire field.

Current whole-command ASCII byte lengths are 3778 (probe), 7954 (resources),
7954 (resources with capacity skipped), 4394 (processes), and 4762 (sockets).
These are measured from the actual checked-in script bytes, not a shell estimate.
`verify_static.py` recomputes the lengths, hashes and fixture/test inventories.

The synthetic frames in `fixtures/` are for parser and UI integration only:

| Fixture | Required exec exit | Meaning |
| --- | --- | --- |
| `probe.json` | 0 | Complete Windows identity |
| `resources.json` | 0 | Reported CPU, Unicode interface, wide counters, quota-distinct capacity |
| `resources-unknown.json` | 0 | Unknown CPU, explicitly empty interfaces, skipped capacity |
| `processes.json` | 0 | PID 0 plus Unicode process, unknown and wide working sets |
| `processes-empty.json` | 0 | Complete explicit empty process inventory |
| `sockets.json` | 0 | TCP listener plus scoped IPv6 UDP endpoint |
| `sockets-empty.json` | 0 | Complete explicit empty socket inventory |
| `sockets-permission-denied.json` | nonzero | Typed failure with null data |

Successful fixtures require empty stderr. A failure fixture also uses empty
stderr: classification comes only from its validated typed status. Files end
with LF; exactly one LF or CRLF, or no final line ending, is accepted. Leading
whitespace, banners, extra trailing whitespace and concatenated frames fail.

## Official source facts

Only Microsoft first-party documentation was consulted. These pages returned
HTTP 200 during the 2026-11-17 source check; this is documentation research,
not execution evidence:

- [Windows PowerShell 5.1 invocation](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_powershell_exe?view=powershell-5.1):
  EncodedCommand uses Base64 over UTF-16LE.
- [Win32_Processor](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-processor):
  the collector reads LoadPercentage and NumberOfLogicalProcessors.
- [Win32_OperatingSystem](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-operatingsystem):
  memory sizes use provider kilobytes; boot/current timestamps are system dates,
  so their difference is explicitly a wall-clock uptime estimate.
- [GetIPStatistics](https://learn.microsoft.com/en-us/dotnet/api/system.net.networkinformation.networkinterface.getipstatistics?view=netframework-4.8.1)
  and [BytesReceived](https://learn.microsoft.com/en-us/dotnet/api/system.net.networkinformation.ipinterfacestatistics.bytesreceived?view=netframework-4.8.1):
  actual per-interface cumulative byte observations, with no rate inference.
- [AvailableFreeSpace](https://learn.microsoft.com/en-us/dotnet/api/system.io.driveinfo.availablefreespace?view=netframework-4.8.1):
  quota-aware availability can differ from TotalFreeSpace; used capacity is
  calculated from total minus total free.
- [WorkingSet64](https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.workingset64?view=netframework-4.8.1):
  physical working-set bytes are optional process observations, not memory or
  CPU percentages.
- [Get-NetTCPConnection](https://learn.microsoft.com/en-us/powershell/module/nettcpip/get-nettcpconnection?view=windowsserver2025-ps)
  and [Get-NetUDPEndpoint](https://learn.microsoft.com/en-us/powershell/module/nettcpip/get-netudpendpoint?view=windowsserver2025-ps):
  collect TCP Listen and UDP local endpoint/PID metadata independently; either
  command's failure rejects the entire socket facility.

No actual Windows execution has been performed by this collector task.
